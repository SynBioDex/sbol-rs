use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sbol_rulegen::generate_profile;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[test]
fn generates_typed_profile_metadata() {
    let directory = TestDirectory::new();
    let rules = directory.path().join("rules.toml");
    fs::write(
        &rules,
        r#"profile = "https://example.org/profile/1"
version = "1"
status = "draft"

[[rules]]
id = "example-10001"
section = "Scope"
classes = ["Reader", "Validator"]
strength = "required"
machine_checkable = true
shacl_core = false
statement = "Readers and validators MUST preserve this rule."
"#,
    )
    .unwrap();

    generate_profile(&rules, directory.path());

    let generated = fs::read_to_string(directory.path().join("profile_rule_catalog.rs")).unwrap();
    assert!(generated.contains("PROFILE_RULE_CATALOG_IRI"));
    assert!(generated.contains("ConformanceClass::Reader"));
    assert!(generated.contains("ConformanceClass::Validator"));
    assert!(generated.contains("RuleStrength::Required"));
    assert!(generated.contains("example-10001"));
}

#[test]
fn rejects_duplicate_rule_identifiers() {
    let directory = TestDirectory::new();
    let rules = directory.path().join("rules.toml");
    fs::write(
        &rules,
        r#"profile = "https://example.org/profile/1"
version = "1"
status = "draft"

[[rules]]
id = "example-10001"
section = "Scope"
classes = ["Validator"]
strength = "required"
machine_checkable = true
shacl_core = false
statement = "First."

[[rules]]
id = "example-10001"
section = "Scope"
classes = ["Validator"]
strength = "required"
machine_checkable = true
shacl_core = false
statement = "Duplicate."
"#,
    )
    .unwrap();

    let outcome = std::panic::catch_unwind(|| generate_profile(&rules, directory.path()));
    assert!(outcome.is_err());
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "sbol-rulegen-profile-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
