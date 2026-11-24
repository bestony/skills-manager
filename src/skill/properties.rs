//! Reading validated skill properties from a skill directory.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde_json::Value;

use super::frontmatter::{SkillParseError, find_skill_md, parse_frontmatter};

/// Error produced by [`read_properties`].
#[derive(Debug)]
#[non_exhaustive]
pub enum SkillError {
    /// `SKILL.md` could not be located or parsed.
    Parse(SkillParseError),
    /// A required frontmatter field is missing or has an invalid type.
    Validation(String),
}

impl fmt::Display for SkillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => e.fmt(f),
            Self::Validation(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for SkillError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(e) => Some(e),
            Self::Validation(_) => None,
        }
    }
}

impl From<SkillParseError> for SkillError {
    fn from(e: SkillParseError) -> Self {
        Self::Parse(e)
    }
}

/// Frontmatter properties of a skill, per the Agent Skills spec.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SkillProperties {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub allowed_tools: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

/// Read the frontmatter properties of a skill directory.
///
/// This performs only the structural checks needed to construct
/// [`SkillProperties`]; use [`super::validate_skill`] for full validation.
pub fn read_properties(skill_dir: &Path) -> Result<SkillProperties, SkillError> {
    let skill_md = find_skill_md(skill_dir).ok_or(SkillParseError::MissingSkillMd)?;
    let content = std::fs::read_to_string(&skill_md).map_err(SkillParseError::Io)?;
    let (fields, _body) = parse_frontmatter(&content)?;

    let required_str = |field: &'static str| -> Result<String, SkillError> {
        match fields.get(field) {
            None => Err(SkillError::Validation(format!(
                "Missing required field in frontmatter: {field}"
            ))),
            Some(value) => match value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Ok(s.to_string()),
                None => Err(SkillError::Validation(format!(
                    "Field '{field}' must be a non-empty string"
                ))),
            },
        }
    };
    let optional_str =
        |field: &str| -> Option<String> { fields.get(field)?.as_str().map(ToString::to_string) };

    Ok(SkillProperties {
        name: required_str("name")?,
        description: required_str("description")?,
        license: optional_str("license"),
        compatibility: optional_str("compatibility"),
        allowed_tools: optional_str("allowed-tools"),
        metadata: read_metadata(&fields),
    })
}

/// Coerce `metadata` into a string map, matching `skills-ref`'s `str()` coercion
/// of scalar values. Returns an empty map when the field is absent or not a map.
fn read_metadata(fields: &serde_json::Map<String, Value>) -> BTreeMap<String, String> {
    fields
        .get("metadata")
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .map(|(key, value)| {
                    let text = match value {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    (key.clone(), text)
                })
                .collect()
        })
        .unwrap_or_default()
}
