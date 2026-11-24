//! Validation of a skill directory against the Agent Skills spec.
//!
//! Error semantics follow `skills-ref::validator`; spec recommendations
//! (e.g. `SKILL.md` length) are reported as warnings.

use std::path::Path;

use serde_json::{Map, Value};
use unicode_normalization::UnicodeNormalization;

use super::frontmatter::{SKILL_MD, SkillParseError, find_skill_md, parse_frontmatter};

const MAX_SKILL_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 1024;
const MAX_COMPATIBILITY_LENGTH: usize = 500;
/// Spec recommendation: keep `SKILL.md` under 500 lines.
const RECOMMENDED_SKILL_MD_LINES: usize = 500;

/// Frontmatter fields allowed by the spec.
const ALLOWED_FIELDS: [&str; 6] = [
    "allowed-tools",
    "compatibility",
    "description",
    "license",
    "metadata",
    "name",
];

/// Severity of a [`ValidationIssue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Violates a spec requirement; the skill is invalid.
    Error,
    /// Violates a spec recommendation; the skill still works.
    Warning,
}

/// A single validation finding.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ValidationIssue {
    pub severity: Severity,
    pub message: String,
}

impl ValidationIssue {
    fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
        }
    }
}

/// Result of validating a skill directory or frontmatter map.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct SkillValidation {
    pub issues: Vec<ValidationIssue>,
}

impl SkillValidation {
    /// True when no [`Severity::Error`] issues were found.
    pub fn is_valid(&self) -> bool {
        !self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    fn push(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    fn error(&mut self, message: impl Into<String>) {
        self.push(ValidationIssue::error(message));
    }

    fn warning(&mut self, message: impl Into<String>) {
        self.push(ValidationIssue::warning(message));
    }
}

/// Validate a skill directory.
///
/// Checks that the directory contains `SKILL.md`, that its YAML frontmatter
/// parses, and that all fields satisfy the spec. The returned
/// [`SkillValidation`] is valid when it contains no errors.
pub fn validate_skill(skill_dir: &Path) -> SkillValidation {
    let mut report = SkillValidation::default();

    if !skill_dir.exists() {
        report.error(format!("Path does not exist: {}", skill_dir.display()));
        return report;
    }
    if !skill_dir.is_dir() {
        report.error(format!("Not a directory: {}", skill_dir.display()));
        return report;
    }

    let Some(skill_md) = find_skill_md(skill_dir) else {
        report.error(format!("Missing required file: {SKILL_MD}"));
        return report;
    };
    if skill_md.file_name().is_some_and(|n| n != SKILL_MD) {
        report.warning(format!(
            "Manifest filename '{}' should be {SKILL_MD}",
            skill_md.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    let content = match std::fs::read_to_string(&skill_md) {
        Ok(content) => content,
        Err(e) => {
            report.error(SkillParseError::Io(e).to_string());
            return report;
        }
    };
    if content.lines().count() > RECOMMENDED_SKILL_MD_LINES {
        report.warning(format!(
            "{SKILL_MD} has {} lines; the spec recommends keeping it under {RECOMMENDED_SKILL_MD_LINES}",
            content.lines().count()
        ));
    }

    let (fields, _body) = match parse_frontmatter(&content) {
        Ok(parsed) => parsed,
        Err(e) => {
            report.error(e.to_string());
            return report;
        }
    };

    let dir_name = skill_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    validate_frontmatter(&fields, dir_name.as_deref(), &mut report);
    report
}

/// Validate an already-parsed frontmatter map.
///
/// `dir_name` is the skill directory name, used for the required
/// name-matches-directory check; pass `None` to skip it.
pub fn validate_frontmatter(
    fields: &Map<String, Value>,
    dir_name: Option<&str>,
    report: &mut SkillValidation,
) {
    let mut extra: Vec<&str> = fields
        .keys()
        .map(String::as_str)
        .filter(|key| !ALLOWED_FIELDS.contains(key))
        .collect();
    extra.sort_unstable();
    if !extra.is_empty() {
        report.error(format!(
            "Unexpected fields in frontmatter: {}. Only {ALLOWED_FIELDS:?} are allowed.",
            extra.join(", ")
        ));
    }

    match fields.get("name") {
        None => report.error("Missing required field in frontmatter: name"),
        Some(value) => validate_name(value, dir_name, report),
    }

    match fields.get("description") {
        None => report.error("Missing required field in frontmatter: description"),
        Some(value) => validate_description(value, report),
    }

    if let Some(value) = fields.get("compatibility") {
        validate_compatibility(value, report);
    }
    if let Some(value) = fields.get("metadata") {
        validate_metadata_field(value, report);
    }
    for field in ["license", "allowed-tools"] {
        if fields.get(field).is_some_and(|v| !v.is_string()) {
            report.error(format!("Field '{field}' must be a string"));
        }
    }
}

fn validate_name(value: &Value, dir_name: Option<&str>, report: &mut SkillValidation) {
    let Some(raw) = value.as_str() else {
        report.error("Field 'name' must be a non-empty string");
        return;
    };
    let name: String = raw.trim().nfkc().collect();
    if name.is_empty() {
        report.error("Field 'name' must be a non-empty string");
        return;
    }

    if name.chars().count() > MAX_SKILL_NAME_LENGTH {
        report.error(format!(
            "Skill name '{name}' exceeds {MAX_SKILL_NAME_LENGTH} character limit ({} chars)",
            name.chars().count()
        ));
    }
    if name != name.to_lowercase() {
        report.error(format!("Skill name '{name}' must be lowercase"));
    }
    if name.starts_with('-') || name.ends_with('-') {
        report.error("Skill name cannot start or end with a hyphen");
    }
    if name.contains("--") {
        report.error("Skill name cannot contain consecutive hyphens");
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-') {
        report.error(format!(
            "Skill name '{name}' contains invalid characters. \
             Only letters, digits, and hyphens are allowed."
        ));
    }
    if let Some(dir_name) = dir_name {
        let normalized: String = dir_name.nfkc().collect();
        if normalized != name {
            report.error(format!(
                "Directory name '{dir_name}' must match skill name '{name}'"
            ));
        }
    }
}

fn validate_description(value: &Value, report: &mut SkillValidation) {
    let Some(description) = value.as_str() else {
        report.error("Field 'description' must be a non-empty string");
        return;
    };
    if description.trim().is_empty() {
        report.error("Field 'description' must be a non-empty string");
        return;
    }
    if description.chars().count() > MAX_DESCRIPTION_LENGTH {
        report.error(format!(
            "Description exceeds {MAX_DESCRIPTION_LENGTH} character limit ({} chars)",
            description.chars().count()
        ));
    }
}

fn validate_compatibility(value: &Value, report: &mut SkillValidation) {
    let Some(compatibility) = value.as_str() else {
        report.error("Field 'compatibility' must be a string");
        return;
    };
    if compatibility.chars().count() > MAX_COMPATIBILITY_LENGTH {
        report.error(format!(
            "Compatibility exceeds {MAX_COMPATIBILITY_LENGTH} character limit ({} chars)",
            compatibility.chars().count()
        ));
    }
}

fn validate_metadata_field(value: &Value, report: &mut SkillValidation) {
    let Some(entries) = value.as_object() else {
        report.error("Field 'metadata' must be a mapping of string keys to string values");
        return;
    };
    if entries
        .values()
        .any(|v| matches!(v, Value::Object(_) | Value::Array(_)))
    {
        report.error("Field 'metadata' values must be strings");
    }
}
