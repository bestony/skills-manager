//! Skill domain feature: parsing and validation per the
//! [Agent Skills specification](https://agentskills.io/specification).
//!
//! Semantics follow the `skills-ref` reference implementation.

mod frontmatter;
mod properties;
#[cfg(test)]
mod tests;
mod validate;

pub use frontmatter::{SkillParseError, find_skill_md, parse_frontmatter, split_frontmatter};
pub use properties::{SkillError, SkillProperties, read_properties};
pub use validate::{Severity, SkillValidation, ValidationIssue, validate_skill};
