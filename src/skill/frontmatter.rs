//! `SKILL.md` location and YAML frontmatter extraction.

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The canonical manifest filename required by the spec.
pub const SKILL_MD: &str = "SKILL.md";

/// Filenames accepted when locating the manifest, in preference order.
/// `skill.md` is accepted for parity with `skills-ref`.
const SKILL_MD_CANDIDATES: [&str; 2] = [SKILL_MD, "skill.md"];

/// Error produced while locating or parsing `SKILL.md`.
#[derive(Debug)]
#[non_exhaustive]
pub enum SkillParseError {
    /// No `SKILL.md`/`skill.md` file exists in the directory.
    MissingSkillMd,
    /// The manifest could not be read.
    Io(std::io::Error),
    /// The file does not start with a `---` frontmatter delimiter.
    MissingFrontmatter,
    /// The opening `---` has no closing delimiter.
    UnclosedFrontmatter,
    /// The frontmatter block is not valid YAML.
    InvalidYaml(String),
    /// The frontmatter parsed, but is not a YAML mapping.
    NotAMapping,
}

impl fmt::Display for SkillParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSkillMd => write!(f, "Missing required file: {SKILL_MD}"),
            Self::Io(e) => write!(f, "Failed to read {SKILL_MD}: {e}"),
            Self::MissingFrontmatter => {
                write!(f, "{SKILL_MD} must start with YAML frontmatter (---)")
            }
            Self::UnclosedFrontmatter => {
                write!(f, "{SKILL_MD} frontmatter not properly closed with ---")
            }
            Self::InvalidYaml(e) => write!(f, "Invalid YAML in frontmatter: {e}"),
            Self::NotAMapping => write!(f, "{SKILL_MD} frontmatter must be a YAML mapping"),
        }
    }
}

impl std::error::Error for SkillParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// Locate the manifest inside a skill directory.
///
/// Prefers `SKILL.md` but accepts `skill.md`, matching `skills-ref`.
/// Entries are enumerated so the returned path carries the real on-disk
/// filename (relevant on case-insensitive filesystems).
pub fn find_skill_md(skill_dir: &Path) -> Option<PathBuf> {
    let mut lowercase = None;
    for entry in std::fs::read_dir(skill_dir).ok()?.flatten() {
        let name = entry.file_name();
        if name == SKILL_MD_CANDIDATES[0] && entry.path().is_file() {
            return Some(entry.path());
        }
        if name == SKILL_MD_CANDIDATES[1] && entry.path().is_file() {
            lowercase = Some(entry.path());
        }
    }
    lowercase
}

/// Split `SKILL.md` content into `(frontmatter_source, body)`.
///
/// Both the opening and closing `---` must appear on their own line.
pub fn split_frontmatter(content: &str) -> Result<(&str, &str), SkillParseError> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);

    let mut lines = content.split('\n');
    let first = lines.next().unwrap_or_default();
    if first.trim_end() != "---" {
        return Err(SkillParseError::MissingFrontmatter);
    }

    let mut offset = first.len() + 1;
    let frontmatter_start = offset;
    for line in lines {
        if line.trim_end() == "---" {
            let body_start = (offset + line.len() + 1).min(content.len());
            return Ok((&content[frontmatter_start..offset], &content[body_start..]));
        }
        offset += line.len() + 1;
    }
    Err(SkillParseError::UnclosedFrontmatter)
}

/// Parse `SKILL.md` content into `(frontmatter_fields, body)`.
///
/// The returned body is trimmed, matching `skills-ref`.
pub fn parse_frontmatter(content: &str) -> Result<(Map<String, Value>, String), SkillParseError> {
    let (frontmatter, body) = split_frontmatter(content)?;
    let value: Value = serde_saphyr::from_str(frontmatter)
        .map_err(|e| SkillParseError::InvalidYaml(e.to_string()))?;
    let fields = value
        .as_object()
        .cloned()
        .ok_or(SkillParseError::NotAMapping)?;
    Ok((fields, body.trim().to_string()))
}
