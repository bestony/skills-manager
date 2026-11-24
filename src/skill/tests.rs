//! Pure tests for the skill spec validator and property reader.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::{Severity, read_properties, validate_skill};

/// Write `content` to `<root>/<dir_name>/SKILL.md` and return the skill dir.
fn write_skill(root: &Path, dir_name: &str, content: &str) -> PathBuf {
    let dir = root.join(dir_name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), content).unwrap();
    dir
}

fn skill_md(name: &str, description: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n\nBody.\n")
}

fn messages(report: &super::SkillValidation) -> Vec<&str> {
    report.issues.iter().map(|i| i.message.as_str()).collect()
}

#[test]
fn valid_minimal_skill() {
    let tmp = TempDir::new().unwrap();
    let dir = write_skill(
        tmp.path(),
        "pdf-processing",
        &skill_md("pdf-processing", "Extracts text and tables from PDF files."),
    );
    let report = validate_skill(&dir);
    assert!(report.is_valid(), "unexpected issues: {:?}", report.issues);
}

#[test]
fn valid_skill_with_optional_fields() {
    let tmp = TempDir::new().unwrap();
    let content = "\
---
name: pdf-processing
description: Extracts text and tables from PDF files.
license: Apache-2.0
compatibility: Requires git and docker
allowed-tools: Bash(git:*) Read
metadata:
  author: example-org
  version: \"1.0\"
---

Body.
";
    let dir = write_skill(tmp.path(), "pdf-processing", content);
    let report = validate_skill(&dir);
    assert!(report.is_valid(), "unexpected issues: {:?}", report.issues);
}

#[test]
fn path_does_not_exist() {
    let report = validate_skill(Path::new("definitely/not/here"));
    assert!(!report.is_valid());
    assert!(messages(&report)[0].starts_with("Path does not exist:"));
}

#[test]
fn path_is_not_a_directory() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("file.txt");
    fs::write(&file, "x").unwrap();
    let report = validate_skill(&file);
    assert!(!report.is_valid());
    assert!(messages(&report)[0].starts_with("Not a directory:"));
}

#[test]
fn missing_skill_md() {
    let tmp = TempDir::new().unwrap();
    let report = validate_skill(tmp.path());
    assert!(!report.is_valid());
    assert_eq!(messages(&report), ["Missing required file: SKILL.md"]);
}

#[test]
fn missing_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let dir = write_skill(tmp.path(), "s", "# No frontmatter\n");
    let report = validate_skill(&dir);
    assert!(!report.is_valid());
    assert_eq!(
        messages(&report),
        ["SKILL.md must start with YAML frontmatter (---)"]
    );
}

#[test]
fn unclosed_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let dir = write_skill(tmp.path(), "s", "---\nname: s\n");
    let report = validate_skill(&dir);
    assert!(!report.is_valid());
    assert_eq!(
        messages(&report),
        ["SKILL.md frontmatter not properly closed with ---"]
    );
}

#[test]
fn frontmatter_not_a_mapping() {
    let tmp = TempDir::new().unwrap();
    let dir = write_skill(tmp.path(), "s", "---\n- just\n- a\n- list\n---\n");
    let report = validate_skill(&dir);
    assert!(!report.is_valid());
    assert_eq!(
        messages(&report),
        ["SKILL.md frontmatter must be a YAML mapping"]
    );
}

#[test]
fn missing_required_fields() {
    let tmp = TempDir::new().unwrap();
    let dir = write_skill(tmp.path(), "s", "---\nlicense: MIT\n---\n");
    let report = validate_skill(&dir);
    let msgs = messages(&report);
    assert!(msgs.contains(&"Missing required field in frontmatter: name"));
    assert!(msgs.contains(&"Missing required field in frontmatter: description"));
}

#[test]
fn unexpected_fields_rejected() {
    let tmp = TempDir::new().unwrap();
    let content = "---\nname: s\ndescription: d\nfoo: bar\n---\n";
    let dir = write_skill(tmp.path(), "s", content);
    let report = validate_skill(&dir);
    assert!(!report.is_valid());
    assert!(
        messages(&report)[0].starts_with("Unexpected fields in frontmatter: foo"),
        "got: {:?}",
        report.issues
    );
}

#[test]
fn name_rules() {
    let cases: [(&str, &str); 6] = [
        ("Upper-Case", "must be lowercase"),
        ("-leading", "cannot start or end with a hyphen"),
        ("trailing-", "cannot start or end with a hyphen"),
        ("double--hyphen", "cannot contain consecutive hyphens"),
        ("has_underscore", "contains invalid characters"),
        (&"a".repeat(65), "exceeds 64 character limit"),
    ];
    for (name, expected) in cases {
        let tmp = TempDir::new().unwrap();
        let dir = write_skill(tmp.path(), "valid-name", &skill_md(name, "d"));
        let report = validate_skill(&dir);
        assert!(
            messages(&report).iter().any(|m| m.contains(expected)),
            "name {name:?}: expected {expected:?} in {:?}",
            report.issues
        );
    }
}

#[test]
fn name_must_match_directory() {
    let tmp = TempDir::new().unwrap();
    let dir = write_skill(tmp.path(), "actual-dir", &skill_md("other-name", "d"));
    let report = validate_skill(&dir);
    assert!(
        messages(&report)
            .iter()
            .any(|m| m.contains("Directory name 'actual-dir' must match skill name 'other-name'"))
    );
}

#[test]
fn name_unicode_is_accepted() {
    let tmp = TempDir::new().unwrap();
    let dir = write_skill(tmp.path(), "数据分析", &skill_md("数据分析", "d"));
    let report = validate_skill(&dir);
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.message.contains("invalid characters")),
        "got: {:?}",
        report.issues
    );
}

#[test]
fn empty_and_nonstring_name() {
    for name_yaml in ["\"  \"", "123", ""] {
        let tmp = TempDir::new().unwrap();
        let content = format!("---\nname: {name_yaml}\ndescription: d\n---\n");
        let dir = write_skill(tmp.path(), "s", &content);
        let report = validate_skill(&dir);
        assert!(
            messages(&report)
                .iter()
                .any(|m| m.contains("Field 'name' must be a non-empty string")),
            "name {name_yaml:?}: {:?}",
            report.issues
        );
    }
}

#[test]
fn description_too_long() {
    let tmp = TempDir::new().unwrap();
    let long = "d".repeat(1025);
    let dir = write_skill(tmp.path(), "s", &skill_md("s", &long));
    let report = validate_skill(&dir);
    assert!(
        messages(&report)
            .iter()
            .any(|m| m.contains("Description exceeds 1024 character limit"))
    );
}

#[test]
fn compatibility_limits() {
    let tmp = TempDir::new().unwrap();
    let long = "c".repeat(501);
    let content = format!("---\nname: s\ndescription: d\ncompatibility: {long}\n---\n");
    let dir = write_skill(tmp.path(), "s", &content);
    let report = validate_skill(&dir);
    assert!(
        messages(&report)
            .iter()
            .any(|m| m.contains("Compatibility exceeds 500 character limit"))
    );

    let tmp = TempDir::new().unwrap();
    let content = "---\nname: s\ndescription: d\ncompatibility: [a, b]\n---\n";
    let dir = write_skill(tmp.path(), "s", content);
    let report = validate_skill(&dir);
    assert!(
        messages(&report)
            .iter()
            .any(|m| m.contains("Field 'compatibility' must be a string"))
    );
}

#[test]
fn metadata_must_be_string_map() {
    let tmp = TempDir::new().unwrap();
    let content = "---\nname: s\ndescription: d\nmetadata: just-a-string\n---\n";
    let dir = write_skill(tmp.path(), "s", content);
    let report = validate_skill(&dir);
    assert!(
        messages(&report)
            .iter()
            .any(|m| m.contains("Field 'metadata' must be a mapping")),
        "got: {:?}",
        report.issues
    );
}

#[test]
fn lowercase_skill_md_warns_but_valid() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("s");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("skill.md"), skill_md("s", "d")).unwrap();
    let report = validate_skill(&dir);
    assert!(report.is_valid(), "got: {:?}", report.issues);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].severity, Severity::Warning);
}

#[test]
fn long_skill_md_warns_but_valid() {
    let tmp = TempDir::new().unwrap();
    let padding = "line\n".repeat(500);
    let content = format!("{}\n{}", skill_md("s", "d"), padding);
    let dir = write_skill(tmp.path(), "s", &content);
    let report = validate_skill(&dir);
    assert!(report.is_valid(), "got: {:?}", report.issues);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.severity == Severity::Warning && i.message.contains("500"))
    );
}

#[test]
fn read_properties_full() {
    let tmp = TempDir::new().unwrap();
    let content = "\
---
name: pdf-processing
description: Extracts text and tables from PDF files.
license: Apache-2.0
compatibility: Requires git
allowed-tools: Bash(git:*)
metadata:
  author: example-org
  version: \"1.0\"
---

Instructions here.
";
    let dir = write_skill(tmp.path(), "pdf-processing", content);
    let props = read_properties(&dir).unwrap();
    assert_eq!(props.name, "pdf-processing");
    assert_eq!(
        props.description,
        "Extracts text and tables from PDF files."
    );
    assert_eq!(props.license.as_deref(), Some("Apache-2.0"));
    assert_eq!(props.compatibility.as_deref(), Some("Requires git"));
    assert_eq!(props.allowed_tools.as_deref(), Some("Bash(git:*)"));
    assert_eq!(
        props.metadata.get("author").map(String::as_str),
        Some("example-org")
    );
    assert_eq!(
        props.metadata.get("version").map(String::as_str),
        Some("1.0")
    );
}

#[test]
fn read_properties_coerces_scalar_metadata() {
    let tmp = TempDir::new().unwrap();
    let content = "---\nname: s\ndescription: d\nmetadata:\n  version: 1.0\n  pinned: true\n---\n";
    let dir = write_skill(tmp.path(), "s", content);
    let props = read_properties(&dir).unwrap();
    assert_eq!(
        props.metadata.get("version").map(String::as_str),
        Some("1.0")
    );
    assert_eq!(
        props.metadata.get("pinned").map(String::as_str),
        Some("true")
    );
}

#[test]
fn read_properties_missing_name() {
    let tmp = TempDir::new().unwrap();
    let dir = write_skill(tmp.path(), "s", "---\ndescription: d\n---\n");
    let err = read_properties(&dir).unwrap_err();
    assert!(err.to_string().contains("name"), "got: {err}");
}
