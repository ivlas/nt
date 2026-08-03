mod common;

use std::fs;

const DOCS: &[&str] = &[
    "README.md",
    "docs/usage.md",
    "docs/cli-reference.md",
    "docs/design.md",
];

#[test]
fn docs_describe_the_sqlite_storage_contract() {
    for path in DOCS {
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("SQLite"), "{path} should describe SQLite");
    }

    let reference = fs::read_to_string("docs/cli-reference.md").unwrap();
    for term in [
        "$HOME/.nt/nt.sqlite3",
        "UUIDv7",
        "<vault>/<collection>",
        "active-vault state",
        "home collection",
    ] {
        assert!(
            reference.contains(term),
            "reference should contain {term:?}"
        );
    }
}

#[test]
fn docs_cover_cross_vault_home_membership_and_bounded_context() {
    let design = fs::read_to_string("docs/design.md").unwrap();
    assert!(design.contains("multiple vaults"));
    assert!(design.contains("home"));
    assert!(design.contains("bounded context"));

    let usage = fs::read_to_string("docs/usage.md").unwrap();
    assert!(usage.contains("home:personal/rust"));
    assert!(usage.contains("collection:work/project_a"));
    assert!(usage.contains("nt list id,title,home"));
}
