use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn nt_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_nt") {
        return PathBuf::from(path);
    }

    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("nt");
    path
}

#[test]
fn config_show_prints_active_vault() {
    let root = temp_dir("config-active-vault");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let shown = run_nt(&home, &["config", "show"]);

    assert!(shown.contains("vault notes"));
    assert!(shown.contains(&notes.display().to_string()));
    assert!(!shown.contains("agent_workspace"));
    assert!(!shown.contains("skills_dir"));
    assert!(!shown.contains("agent_output"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn config_vault_lists_and_switches_active_vault() {
    let root = temp_dir("config-vault-switch");
    let home = root.join("home");
    let notes = root.join("notes");
    let research = root.join("research");

    run_nt(&home, &["init", notes.to_str().unwrap()]);
    let first = run_nt_with_stdin(
        &home,
        &["add", "tag:first-vault"],
        "# First vault\n\nbody one.\n",
    );
    let first_id = first.trim().strip_prefix("saved ").unwrap().to_string();
    run_nt(&home, &["update", &first_id, "status", "open"]);

    run_nt(&home, &["init", research.to_str().unwrap()]);
    let second = run_nt_with_stdin(
        &home,
        &["add", "tag:second-vault"],
        "# Second vault\n\nbody two.\n",
    );
    let second_id = second.trim().strip_prefix("saved ").unwrap().to_string();
    run_nt(&home, &["update", &second_id, "status", "open"]);

    let vaults = run_nt(&home, &["config", "vault"]);
    assert!(vaults.contains(&format!("- notes {}", notes.display())));
    assert!(vaults.contains(&format!("* research {}", research.display())));

    let listed = run_nt(&home, &["list"]);
    assert!(listed.contains(&second_id));
    assert!(!listed.contains(&first_id));
    assert_eq!(run_nt(&home, &["list", "tags"]).trim(), "second-vault");
    let status = run_nt(&home, &["find", "status:open"]);
    assert!(status.contains(&second_id));
    assert!(!status.contains(&first_id));

    let switched = run_nt(&home, &["config", "vault", "notes"]);
    assert_eq!(
        switched.trim(),
        format!("configured vault notes {}", notes.display())
    );
    assert_failed(
        &home,
        &["config", "vault", "missing"],
        "unknown vault `missing`; run `nt config vault`",
    );

    let listed = run_nt(&home, &["list"]);
    assert!(listed.contains(&first_id));
    assert!(!listed.contains(&second_id));
    assert_eq!(run_nt(&home, &["list", "tags"]).trim(), "first-vault");
    let status = run_nt(&home, &["find", "status:open"]);
    assert!(status.contains(&first_id));
    assert!(!status.contains(&second_id));

    let index = read_index(&home);
    assert_eq!(index["active_vault"].as_str(), Some("notes"));
    assert_eq!(
        index["vaults"]["notes"]["path"].as_str(),
        Some(notes.to_str().unwrap())
    );
    assert!(index.get("notebooks").is_none());
    assert!(index.get("active_notes_dir").is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_rejects_duplicate_vault_names() {
    let root = temp_dir("init-duplicate-vault");
    let home = root.join("home");
    let notes = root.join("notes");
    let research = root.join("research");
    let duplicate_notes = root.join("other").join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);
    run_nt(&home, &["init", research.to_str().unwrap()]);

    assert_failed(
        &home,
        &["init", duplicate_notes.to_str().unwrap()],
        "vault `notes` already exists; choose another notes directory name",
    );

    let vaults = run_nt(&home, &["config", "vault"]);
    assert!(vaults.contains(&format!("- notes {}", notes.display())));
    assert!(vaults.contains(&format!("* research {}", research.display())));
    assert!(!vaults.contains(&duplicate_notes.display().to_string()));
    assert!(!duplicate_notes.exists());

    let index = read_index(&home);
    assert_eq!(index["active_vault"].as_str(), Some("research"));
    assert!(index["vaults"]["notes"].is_object());
    assert!(index["vaults"]["research"].is_object());
    assert!(
        index["vaults"]
            .as_object()
            .unwrap()
            .get("notes-2")
            .is_none()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_rejects_non_flat_or_non_note_entries() {
    let root = temp_dir("init-invalid-notes-dir");
    let home = root.join("home");
    let stray_notes = root.join("stray-notes");
    let nested_notes = root.join("nested-notes");

    fs::create_dir_all(&stray_notes).unwrap();
    fs::write(stray_notes.join("draft.md"), "# Draft\n").unwrap();

    assert_failed(
        &home,
        &["init", stray_notes.to_str().unwrap()],
        "notes directory must contain only NTYYYYMMDDTHHmmss.md files",
    );

    fs::create_dir_all(nested_notes.join("nested")).unwrap();
    assert_failed(
        &home,
        &["init", nested_notes.to_str().unwrap()],
        "notes directory must contain only NTYYYYMMDDTHHmmss.md files",
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_imports_existing_flat_notes() {
    let root = temp_dir("init-import-existing-notes");
    let home = root.join("home");
    let notes = root.join("notes");
    let id = "NT20260528T143012";

    fs::create_dir_all(&notes).unwrap();
    fs::write(
        notes.join(format!("{id}.md")),
        "# Imported\n\nExisting body with [spec](https://example.com/import).\n",
    )
    .unwrap();

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let listed = run_nt(&home, &["list"]);
    assert!(listed.contains(id));
    assert!(listed.contains("Imported"));

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains(&format!("{id}  Imported")));
    assert!(shown.contains("created 2026-05-28T14:30:12Z"));
    assert!(shown.contains("sources https://example.com/import"));
    assert!(shown.contains("# Imported\n\nExisting body with [spec](https://example.com/import)."));

    let index = read_index(&home);
    assert_eq!(index["notes"][id]["title"].as_str(), Some("Imported"));
    assert_eq!(
        index["notes"][id]["path"].as_str(),
        Some(notes.join(format!("{id}.md")).to_str().unwrap())
    );

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn new_user_release_readiness_smoke_flow() {
    let root = temp_dir("release-readiness-flow");
    let home = root.join("home");
    let vault = root.join("notes");
    let editor = root.join("editor.sh");

    let initialized = run_nt(&home, &["init", vault.to_str().unwrap()]);
    assert_eq!(
        initialized.trim(),
        format!("initialized notes {}", vault.display())
    );
    let index = read_index(&home);
    assert_eq!(index["active_vault"].as_str(), Some("notes"));
    assert_eq!(
        index["vaults"]["notes"]["path"].as_str(),
        Some(vault.to_str().unwrap())
    );

    let saved = run_nt_with_stdin(
        &home,
        &["add", "tag:rust", "kind:note", "status:open"],
        "# Rust Ownership\n\nBorrow checker notes.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap().to_string();
    assert!(is_valid_note_id(&id), "invalid generated id: {id}");
    assert_eq!(
        fs::read_to_string(vault.join(format!("{id}.md"))).unwrap(),
        "# Rust Ownership\n\nBorrow checker notes.\n"
    );

    let listed = run_nt(&home, &["list"]);
    assert_eq!(summary_ids(&listed), vec![id.as_str()]);
    assert!(listed.contains("Rust Ownership"));

    let found = run_nt(&home, &["find", "rust", "ownership"]);
    assert_eq!(summary_ids(&found), vec![id.as_str()]);

    let shown = run_nt(&home, &["show", &id]);
    assert!(shown.contains(&format!("{id}  Rust Ownership")));
    assert!(shown.contains("kind note"));
    assert!(shown.contains("status open"));
    assert!(shown.contains("tags rust"));
    assert!(shown.contains("# Rust Ownership\n\nBorrow checker notes."));

    fs::write(
        &editor,
        "#!/bin/sh\ncat > \"$1\" <<'EOF'\n# Rust Ownership Updated\n\nBorrow checker field guide.\nEOF\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&editor).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&editor, permissions).unwrap();

    let edited = run_nt_with_env(
        &home,
        &["open", &id],
        &[("EDITOR", editor.to_str().unwrap())],
    );
    assert_eq!(edited.trim(), format!("saved {id}"));

    let shown = run_nt(&home, &["show", &id]);
    assert!(shown.contains(&format!("{id}  Rust Ownership Updated")));
    assert!(shown.contains("# Rust Ownership Updated\n\nBorrow checker field guide."));
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "body:field"])),
        vec![id.as_str()]
    );

    let rebuilt = run_nt(&home, &["rebuild"]);
    assert_eq!(rebuilt.trim(), "rebuilt 1");
    let index = read_index(&home);
    assert_eq!(index["notes"][id.as_str()]["status"].as_str(), Some("open"));
    assert_eq!(
        index["notes"][id.as_str()]["tags"].as_array().unwrap(),
        &vec![serde_json::Value::String("rust".to_string())]
    );

    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "body:borrow"])),
        vec![id.as_str()]
    );
    assert_eq!(run_nt(&home, &["list", "ids"]).trim(), id);
    assert_eq!(run_nt(&home, &["list", "tags"]).trim(), "rust");
    let status = run_nt(&home, &["find", "status:open"]);
    assert_eq!(summary_ids(&status), vec![id.as_str()]);
    let config = run_nt(&home, &["config", "show"]);
    assert!(config.contains("vault notes"));
    assert!(config.contains(&vault.display().to_string()));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebuild_reconstructs_active_vault_index_from_markdown() {
    let root = temp_dir("rebuild-active-vault");
    let home = root.join("home");
    let notes = root.join("notes");
    let first_id = "NT20000101T000000";
    let deleted_id = "NT20000102T000000";

    run_nt(&home, &["init", notes.to_str().unwrap()]);
    fs::write(
        notes.join(format!("{first_id}.md")),
        "# Imported\n\nBody with https://example.com/body-one.\n",
    )
    .unwrap();
    fs::write(
        notes.join(format!("{deleted_id}.md")),
        "# Deleted\n\nThis file will be removed.\n",
    )
    .unwrap();

    let rebuilt = run_nt(&home, &["rebuild"]);
    assert_eq!(rebuilt.trim(), "rebuilt 2");

    run_nt(&home, &["update", first_id, "tag", "+storage"]);
    run_nt(&home, &["update", first_id, "collection", "+projects/nt"]);
    run_nt(&home, &["update", first_id, "kind", "decision"]);
    run_nt(&home, &["update", first_id, "status", "open"]);
    run_nt(
        &home,
        &["update", first_id, "link", &format!("+{}", deleted_id)],
    );

    let mut index = read_index(&home);
    assert_eq!(
        index["notes"][first_id]["sources"].as_array().unwrap(),
        &vec![serde_json::Value::String(
            "https://example.com/body-one".to_string()
        )]
    );
    index["notes"][first_id]["sources"] = serde_json::json!([
        "https://example.com/body-one",
        "https://example.com/explicit"
    ]);
    write_index(&home, &index);

    fs::write(
        notes.join(format!("{first_id}.md")),
        "# Refreshed\n\nBody with https://example.com/body-two.\n",
    )
    .unwrap();
    fs::remove_file(notes.join(format!("{deleted_id}.md"))).unwrap();

    let rebuilt = run_nt(&home, &["rebuild"]);
    assert_eq!(rebuilt, "rebuilt 1\n");
    let rebuilt_again = run_nt(&home, &["rebuild"]);
    assert_eq!(rebuilt_again, "rebuilt 1\n");

    let shown = run_nt(&home, &["show", first_id]);
    assert!(shown.contains(&format!("{first_id}  Refreshed")));
    assert!(shown.contains("kind decision"));
    assert!(shown.contains("status open"));
    assert!(shown.contains("tags storage"));
    assert!(shown.contains("collections projects/nt"));
    assert!(shown.contains("links -"));
    assert!(shown.contains(
        "sources https://example.com/body-one,https://example.com/body-two,https://example.com/explicit"
    ));

    let index = read_index(&home);
    assert!(index["notes"].get(first_id).is_some());
    assert!(index["notes"].get(deleted_id).is_none());
    assert_eq!(
        index["notes"][first_id]["title"].as_str(),
        Some("Refreshed")
    );
    assert_eq!(
        index["body_terms"]["body"].as_array().unwrap(),
        &vec![serde_json::Value::String(first_id.to_string())]
    );
    assert_eq!(
        index["body_terms"]["two"].as_array().unwrap(),
        &vec![serde_json::Value::String(first_id.to_string())]
    );
    assert_eq!(
        index["heading_terms"]["refreshed"].as_array().unwrap(),
        &vec![serde_json::Value::String(first_id.to_string())]
    );
    assert!(index["body_terms"].get("deleted").is_none());
    assert!(
        !serde_json::to_string(&index)
            .unwrap()
            .contains("# Refreshed\\n\\nBody with")
    );
    assert_eq!(
        index["notes"][first_id]["created"].as_str(),
        Some("2000-01-01T00:00:00Z")
    );
    assert_ne!(
        index["notes"][first_id]["updated"].as_str(),
        Some("2000-01-01T00:00:00Z")
    );
    assert!(index["backlinks"].as_object().unwrap().is_empty());
    assert_eq!(
        index["notes"][first_id]["sources"].as_array().unwrap(),
        &vec![
            serde_json::Value::String("https://example.com/body-one".to_string()),
            serde_json::Value::String("https://example.com/body-two".to_string()),
            serde_json::Value::String("https://example.com/explicit".to_string()),
        ]
    );
    assert_eq!(
        index["tags"]["storage"].as_array().unwrap(),
        &vec![serde_json::Value::String(first_id.to_string())]
    );

    let listed = run_nt(&home, &["list"]);
    assert!(listed.contains(first_id));
    assert!(!listed.contains(deleted_id));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_does_not_install_agent_workspace_files() {
    let root = temp_dir("init-no-agent-workspace");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    assert!(home.join(".nt/index.json").exists());
    assert!(!home.join(".nt/AGENTS.md").exists());
    assert!(!home.join(".nt/skills").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn completion_outputs_dynamic_note_id_hooks() {
    let root = temp_dir("completion-hooks");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let bash = run_nt(&home, &["completion", "bash"]);
    assert!(bash.contains("init add rebuild list find show open"));
    assert!(bash.contains("_nt_note_ids"));
    assert!(bash.contains("_nt_titled_notes"));
    assert!(bash.contains("nt list id,title 2>/dev/null"));
    assert!(bash.contains("nt list id 2>/dev/null"));
    assert!(bash.contains("_nt_complete_list_arg"));

    let zsh = run_nt(&home, &["completion", "zsh"]);
    assert!(zsh.contains("'show:'"));
    assert!(zsh.contains("'open:'"));
    assert!(zsh.contains(":id:_nt_titled_notes"));
    assert!(zsh.contains("command nt list id,title 2>/dev/null"));
    assert!(zsh.contains("nt list id 2>/dev/null"));
    assert!(zsh.contains("_nt_list_arg"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn help_is_a_flagless_command_with_examples() {
    let root = temp_dir("help-command");
    let home = root.join("home");

    let root_help = run_nt(&home, &["help"]);
    assert!(root_help.contains("nt <command> [args...]"));
    assert!(root_help.contains("Getting started:"));
    assert!(root_help.contains("Read and edit:"));
    assert!(root_help.contains("Plan and organize:"));
    assert!(root_help.contains("Maintenance:"));
    assert!(root_help.contains("Examples:"));

    let find_help = run_nt(&home, &["help", "find"]);
    assert!(find_help.contains("nt find <expr...>"));
    assert!(find_help.contains("nt find kind:todo due:2026-06-30"));

    let vault_help = run_nt(&home, &["help", "config", "vault"]);
    assert!(vault_help.contains("nt config vault [vault-name]"));
    assert!(vault_help.contains("nt config vault notes"));
    let rebuild_help = run_nt(&home, &["help", "rebuild"]);
    assert!(rebuild_help.contains("preserving primary JSON metadata"));
    let reference = run_nt(&home, &["help", "reference"]);
    assert!(reference.contains("nt CLI reference"));
    assert!(reference.contains("Add metadata:"));
    assert!(reference.contains("nt add kind:research tag:qemu collection:research/vm"));
    assert!(reference.contains("body:<term>"));
    assert!(reference.contains("NTYYYYMMDDTHHmmss"));

    assert_failed(&home, &["--help"], "unexpected argument '--help'");
    assert_failed(&home, &["list", "--help"], "unexpected argument '--help'");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn help_lists_current_top_level_command_surface() {
    let root = temp_dir("help-command-surface");
    let home = root.join("home");

    let root_help = run_nt(&home, &["help"]);
    for command in ROOT_COMMANDS {
        let expected = format!("  {command}");
        assert!(
            root_help.lines().any(|line| line.starts_with(&expected)),
            "nt help should list top-level command `{command}`"
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn readme_quickstart_uses_supported_commands_and_explains_placeholder_id() {
    let readme = fs::read_to_string("README.md").unwrap();
    let quickstart = markdown_section(&readme, "## Quick Start");

    assert!(quickstart.contains("`nt add` prints a note id like `NT20260616T101500`."));
    assert!(quickstart.contains("nt show <id>"));
    assert!(quickstart.contains("nt open <id>"));

    for command in nt_commands_in_shell_blocks(&quickstart) {
        assert!(
            ROOT_COMMANDS.contains(&command.as_str()),
            "README quickstart uses unsupported nt command `{command}`"
        );
    }

    let quickstart_commands = quickstart
        .split("`nt add` prints a note id")
        .next()
        .unwrap();
    let commands = nt_commands_in_shell_blocks(quickstart_commands);
    assert_eq!(commands, vec!["init", "add", "find"]);
}

#[test]
fn usage_shell_workflows_use_only_core_commands() {
    let usage = fs::read_to_string("docs/usage.md").unwrap();
    let workflows = markdown_section(&usage, "## Find And Read");
    let commands = nt_commands_in_shell_blocks(&workflows);

    assert!(!commands.is_empty());
    for command in commands {
        assert!(
            ["find", "show", "open", "list"].contains(&command.as_str()),
            "docs/usage.md uses unsupported shell workflow command `{command}`"
        );
    }
}

#[test]
fn readme_links_to_core_docs() {
    let readme = fs::read_to_string("README.md").unwrap();

    for link in [
        "[docs/usage.md](docs/usage.md)",
        "[docs/cli-reference.md](docs/cli-reference.md)",
        "[docs/design.md](docs/design.md)",
        "[docs/examples/agent-skills.md](docs/examples/agent-skills.md)",
        "[CHANGELOG.md](CHANGELOG.md)",
    ] {
        assert!(readme.contains(link), "README should link to {link}");
    }

    let docs: BTreeSet<String> = fs::read_dir("docs")
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.unwrap();
            entry
                .file_type()
                .unwrap()
                .is_file()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();
    assert_eq!(
        docs,
        BTreeSet::from([
            "cli-reference.md".to_string(),
            "design.md".to_string(),
            "usage.md".to_string(),
        ])
    );
}

#[test]
fn design_tracks_stable_core_and_non_goals() {
    let readme = fs::read_to_string("README.md").unwrap();
    assert!(readme.contains("[docs/design.md](docs/design.md)"));

    let design = fs::read_to_string("docs/design.md").unwrap();
    assert!(design.contains("The 0.1.0 stable\ncore"));

    for area in [
        "## Implemented Architecture",
        "## Storage Decisions",
        "## Retrieval Decisions",
        "## Metadata Decisions",
        "## Interface Decisions",
        "## Decision Status",
        "## Development And Release",
    ] {
        assert!(
            design.contains(area),
            "design doc should include area {area}"
        );
    }

    for non_goal in [
        "RAG system",
        "vector database",
        "daemon",
        "hidden retrieval",
        "embeddings",
        "There is no scoring",
        "A TUI is intentionally deferred",
    ] {
        assert!(
            design.contains(non_goal),
            "design doc should include non-goal {non_goal}"
        );
    }

    let normalized = design.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalized.contains(
            "Future changes are constrained to preserve canonical CommonMark, visible JSON, explicit commands, deterministic output, atomic writes, and no hidden runtime."
        )
    );

    for command in [
        "nt pick",
        "nt tui",
        "nt search",
        "nt grep",
        "nt graph",
        "nt agent",
        "nt run",
        "nt version",
    ] {
        assert!(
            !design.contains(command),
            "design doc should not imply unsupported command `{command}` exists"
        );
    }
}

#[test]
fn release_docs_cover_source_install_and_manual_checks() {
    let readme = fs::read_to_string("README.md").unwrap();
    assert!(readme.contains("cargo install --path ."));

    let design = fs::read_to_string("docs/design.md").unwrap();
    for check in [
        "cargo fmt --check",
        "cargo test",
        "cargo clippy --all-targets",
        "cargo run -- help",
    ] {
        assert!(
            design.contains(check),
            "design release section should include {check}"
        );
    }
}

#[test]
fn docs_do_not_document_version_command_or_flag() {
    assert!(
        !ROOT_COMMANDS.contains(&"version"),
        "update this test if nt gains a supported version command"
    );

    for path in DOC_PATHS {
        let text = fs::read_to_string(path).unwrap();
        assert!(
            !text.contains("nt version"),
            "{path} should not document an unsupported nt version command"
        );
        assert!(
            !text.contains("--version"),
            "{path} should not document an unsupported version flag"
        );
    }
}

#[test]
fn rebuild_docs_document_persistent_source_semantics() {
    let expected = "preserves existing sources and merges URLs currently found in";
    for path in ["docs/cli-reference.md", "docs/usage.md", "README.md"] {
        let text = fs::read_to_string(path).unwrap();
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            normalized.contains(expected),
            "{path} should document rebuild source semantics"
        );
        assert!(
            !text.contains("refreshes current body URL sources"),
            "{path} should not claim rebuild refreshes body URL sources"
        );
    }
}

#[test]
fn docs_document_index_trust_boundary_and_deferred_tui() {
    for path in [
        "README.md",
        "docs/usage.md",
        "docs/cli-reference.md",
        "docs/design.md",
    ] {
        let text = fs::read_to_string(path).unwrap();
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            normalized.contains("Indexed body entries are trusted until `nt rebuild`")
                || normalized.contains("indexed body entries are trusted until `nt rebuild`"),
            "{path} should document indexed body entries are trusted until rebuild"
        );
    }

    for path in ["README.md", "docs/usage.md", "docs/design.md"] {
        let text = fs::read_to_string(path).unwrap();
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            normalized.contains("A TUI is intentionally deferred")
                || normalized.contains("a TUI is intentionally deferred"),
            "{path} should document that TUI is deferred"
        );
    }
}

#[test]
fn find_docs_document_body_terms_not_phrase_search() {
    let expected = "Quoted multiword `body:` values match all indexed terms, not an exact phrase.";
    for path in ["docs/cli-reference.md", "docs/usage.md", "docs/design.md"] {
        let text = fs::read_to_string(path).unwrap();
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            normalized.contains(expected),
            "{path} should document multiword body term semantics"
        );
        assert!(
            !normalized.contains("exact phrase search"),
            "{path} should not imply phrase search"
        );
    }

    let reference = fs::read_to_string("docs/cli-reference.md").unwrap();
    let reference = reference.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(reference.contains("There is no public `heading:<term>` field"));
    assert!(reference.contains("possible future use"));
}

#[test]
fn docs_do_not_make_unqualified_search_claims_or_future_command_examples() {
    for path in DOC_PATHS {
        let text = fs::read_to_string(path).unwrap();
        let lower = text.to_lowercase();
        assert!(!lower.contains("exact phrase search"), "{path}");

        for command in UNSUPPORTED_ROOT_COMMAND_EXAMPLES {
            for paragraph in lower.split("\n\n") {
                if !paragraph_mentions_nt_command(paragraph, command) {
                    continue;
                }
                let normalized = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
                assert!(
                    normalized.contains("there is no")
                        || normalized.contains("no built-in")
                        || normalized.contains("avoid adding")
                        || normalized.contains("deferred")
                        || normalized.contains("not part of the current core")
                        || normalized.contains("outside `nt`"),
                    "{path} contains unqualified unsupported command `{command}`: {normalized}"
                );
            }
        }

        for term in ["semantic search", "ranking", "vector", "embedding"] {
            for paragraph in lower.split("\n\n") {
                if !paragraph.contains(term) {
                    continue;
                }
                let normalized = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
                assert!(
                    normalized.contains(" no ")
                        || normalized.contains(" not ")
                        || normalized.contains("without ")
                        || normalized.contains("avoid ")
                        || normalized.contains("do not ")
                        || normalized.contains("absence "),
                    "{path} contains unqualified `{term}`: {normalized}"
                );
            }
        }
    }
}

#[test]
fn usage_documents_external_shell_interaction() {
    let readme = fs::read_to_string("README.md").unwrap();
    assert!(readme.contains("[docs/usage.md](docs/usage.md)"));

    let usage = fs::read_to_string("docs/usage.md").unwrap();
    assert!(usage.contains("fzf --preview"));

    let combined = [readme, usage, fs::read_to_string("docs/design.md").unwrap()]
        .join("\n")
        .to_lowercase();
    assert!(
        combined.contains("tui is intentionally deferred")
            || combined.contains("tui is not part of the current core")
    );
}

#[cfg(unix)]
#[test]
fn open_uses_editor_and_updates_visible_note() {
    let root = temp_dir("open-editor");
    let home = root.join("home");
    let notes = root.join("notes");
    let editor = root.join("editor.sh");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let saved = run_nt_with_stdin(&home, &["add"], "# Original\n\nbody one.\n");
    let id = saved.trim().strip_prefix("saved ").unwrap().to_string();

    fs::write(
        &editor,
        "#!/bin/sh\ncat > \"$1\" <<'EOF'\n# Edited\n\nbody two with https://example.com/edited.\nEOF\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&editor).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&editor, permissions).unwrap();

    let opened = run_nt_with_env(
        &home,
        &["open", &id],
        &[("EDITOR", editor.to_str().unwrap())],
    );
    assert_eq!(opened.trim(), format!("saved {id}"));

    let shown = run_nt(&home, &["show", &id]);
    assert!(shown.contains(&format!("{id}  Edited")));
    assert!(shown.contains("sources https://example.com/edited"));
    assert!(shown.contains("# Edited\n\nbody two with https://example.com/edited."));
    assert!(!shown.contains("\x1b["));

    let index = read_index(&home);
    assert!(index["body_terms"].get("one").is_none());
    assert_eq!(
        index["body_terms"]["two"].as_array().unwrap(),
        &vec![serde_json::Value::String(id.to_string())]
    );
    assert!(index["heading_terms"].get("original").is_none());
    assert_eq!(
        index["heading_terms"]["edited"].as_array().unwrap(),
        &vec![serde_json::Value::String(id.to_string())]
    );

    let body = fs::read_to_string(notes.join(format!("{id}.md"))).unwrap();
    assert_eq!(
        body,
        "# Edited\n\nbody two with https://example.com/edited.\n"
    );

    fs::write(
        &editor,
        "#!/bin/sh\ncat > \"$1\" <<'EOF'\n## Invalid section\n\nbody three.\nEOF\n",
    )
    .unwrap();
    assert_failed_with_env(
        &home,
        &["open", &id],
        &[("EDITOR", editor.to_str().unwrap())],
        "note must start with a non-empty `# Title` heading",
    );
    assert_eq!(
        fs::read_to_string(notes.join(format!("{id}.md"))).unwrap(),
        "# Edited\n\nbody two with https://example.com/edited.\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn piped_list_and_show_output_stay_plain() {
    let root = temp_dir("plain-piped-output");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let saved = run_nt_with_stdin(&home, &["add", "tag:plain"], "# Plain\n\nbody.\n");
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let listed = run_nt(&home, &["list"]);
    assert!(listed.contains(id));
    assert!(!listed.contains("\x1b["));

    let titled = run_nt(&home, &["list", "titles"]);
    assert_eq!(titled.trim(), format!("{id}\tPlain"));

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("tags plain"));
    assert!(!shown.contains("\x1b["));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn list_projects_fields_and_applies_structured_filters() {
    let root = temp_dir("list-projections");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);
    let first = run_nt_with_stdin(
        &home,
        &["add", "tag:design", "status:open", "kind:decision"],
        "# First decision\n\nbody.\n",
    );
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let second = run_nt_with_stdin(&home, &["add", "tag:draft"], "# Second note\n\nbody.\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();

    let ids = run_nt(&home, &["list", "id"]);
    assert_eq!(ids.lines().collect::<Vec<_>>(), vec![second_id, first_id]);

    let selected = run_nt(&home, &["list", "id,title,status", "status:open"]);
    assert_eq!(selected.trim(), format!("{first_id}\tFirst decision\topen"));

    let filtered_default = run_nt(&home, &["list", "tag:design"]);
    let columns = filtered_default.trim().split('\t').collect::<Vec<_>>();
    assert_eq!(columns.len(), 6);
    assert_eq!(columns[0], first_id);
    assert_eq!(columns[1], "First decision");
    assert_eq!(columns[3], "open");
    assert_eq!(columns[5], "design");

    let all = run_nt(&home, &["list", "all", "tag:design"]);
    let columns = all.trim().split('\t').collect::<Vec<_>>();
    assert_eq!(columns.len(), 15);
    assert_eq!(columns[0], first_id);
    assert_eq!(columns[4], "First decision");

    let optional = run_nt(
        &home,
        &["list", "id,status,tag", &format!("id:{second_id}")],
    );
    assert_eq!(optional.trim(), format!("{second_id}\t-\tdraft"));

    assert_failed(&home, &["list", "id,title", "body:body"], "use `nt find`");
    assert_failed(&home, &["list", "id,titel"], "unknown list field `titel`");
    assert_failed(&home, &["list", "id,id"], "duplicate list field `id`");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn metadata_commands_route_through_visible_index() {
    let root = temp_dir("metadata-routes");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let first = run_nt_with_stdin(&home, &["add"], "# First\n\nbody one.\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let second = run_nt_with_stdin(&home, &["add"], "# Second\n\nbody two.\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();

    run_nt(&home, &["update", first_id, "collection", "+projects/nt"]);
    run_nt(&home, &["update", first_id, "tag", "+storage"]);
    run_nt(&home, &["update", second_id, "collection", "+projects/nt"]);
    run_nt(&home, &["update", second_id, "tag", "+storage"]);
    run_nt(&home, &["update", first_id, "kind", "decision"]);
    run_nt(&home, &["update", first_id, "status", "open"]);
    run_nt(
        &home,
        &["update", first_id, "link", &format!("+{}", second_id)],
    );

    let tags = run_nt(&home, &["list", "tags"]);
    assert_eq!(tags.trim(), "storage");
    let tagged = run_nt(&home, &["list", "tags", "storage"]);
    assert_eq!(summary_ids(&tagged), vec![second_id, first_id]);

    let collections = run_nt(&home, &["list", "collections"]);
    assert_eq!(collections.trim(), "projects/nt");
    let collected = run_nt(&home, &["list", "collections", "projects/nt"]);
    assert_eq!(summary_ids(&collected), vec![second_id, first_id]);

    let collection = run_nt(&home, &["find", "collection:projects/nt"]);
    assert!(collection.contains(first_id));

    let status = run_nt(&home, &["find", "status:open"]);
    assert!(status.contains(first_id));

    assert_failed(
        &home,
        &["list", "links", first_id, "from"],
        "positional link directions are not supported",
    );
    assert_failed(
        &home,
        &["list", "links", second_id, "to"],
        "positional link directions are not supported",
    );

    assert_failed(
        &home,
        &["list", "links", second_id],
        "directionless link lookup",
    );

    let link_metadata = run_nt(&home, &["list", "links"]);
    assert_eq!(
        link_metadata.lines().collect::<Vec<_>>(),
        vec![format!("{first_id}\tFirst\t{second_id}\tSecond")]
    );

    let filtered_links = run_nt(&home, &["list", "links", &format!("id:{first_id}")]);
    assert_eq!(filtered_links, link_metadata);
    let from_links = run_nt(&home, &["list", "links", &format!("from:{first_id}")]);
    assert_eq!(from_links, link_metadata);
    let to_links = run_nt(&home, &["list", "links", &format!("to:{second_id}")]);
    assert_eq!(to_links, link_metadata);
    let exact_link = run_nt(
        &home,
        &[
            "list",
            "links",
            &format!("from:{first_id}"),
            &format!("to:{second_id}"),
        ],
    );
    assert_eq!(exact_link, link_metadata);
    let composed_links = run_nt(
        &home,
        &["list", "links", &format!("to:{second_id}"), "kind:decision"],
    );
    assert_eq!(composed_links, link_metadata);
    let no_outbound_links = run_nt(&home, &["list", "links", &format!("id:{second_id}")]);
    assert!(no_outbound_links.is_empty());

    let found = run_nt(
        &home,
        &[
            "find",
            "tag:storage",
            "kind:decision",
            "status:open",
            "collection:projects/nt",
            &format!("link:{second_id}"),
        ],
    );
    assert!(found.contains(first_id));

    run_nt(
        &home,
        &["update", first_id, "link", &format!("-{}", second_id)],
    );
    run_nt(&home, &["update", first_id, "tag", "-storage"]);
    run_nt(&home, &["update", first_id, "collection", "-projects/nt"]);
    run_nt(&home, &["update", second_id, "tag", "-storage"]);
    run_nt(&home, &["update", second_id, "collection", "-projects/nt"]);

    let links = run_nt(&home, &["list", "links", &format!("from:{first_id}")]);
    assert!(links.trim().is_empty());

    let collection = run_nt(&home, &["find", "collection:projects/nt"]);
    assert!(collection.trim().is_empty());

    let tags = run_nt(&home, &["list", "tags"]);
    assert!(!tags.contains("storage"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn export_writes_front_matter_copies_from_index() {
    let root = temp_dir("export-front-matter");
    let home = root.join("home");
    let notes = root.join("notes");
    let archive = root.join("archive");
    let full_archive = root.join("full-archive");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let first = run_nt_with_stdin(&home, &["add"], "# First\n\nbody one.\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let link = format!("link:{first_id}");
    let second = run_nt_with_stdin(
        &home,
        &[
            "add",
            "tag:storage",
            "kind:decision",
            "status:open",
            "collection:projects/nt",
            "source:https://example.com/spec",
            &link,
        ],
        "# Second\n\nbody two.\n",
    );
    let second_id = second.trim().strip_prefix("saved ").unwrap();

    let exported = run_nt(&home, &["export", archive.to_str().unwrap(), second_id]);
    assert!(
        exported
            .trim()
            .starts_with(&format!("exported {second_id} "))
    );
    assert!(exported.trim().ends_with(&format!("{second_id}.md")));

    let exported_body = fs::read_to_string(archive.join(format!("{second_id}.md"))).unwrap();
    assert!(exported_body.starts_with("---\n"));
    assert!(exported_body.contains(&format!("id: \"{second_id}\"\n")));
    assert!(exported_body.contains("kind: \"decision\"\n"));
    assert!(exported_body.contains("status: \"open\"\n"));
    assert!(exported_body.contains("tags: [\"storage\"]\n"));
    assert!(exported_body.contains("collections: [\"projects/nt\"]\n"));
    assert!(exported_body.contains(&format!("links: [\"{first_id}\"]\n")));
    assert!(exported_body.contains("sources: [\"https://example.com/spec\"]\n"));
    assert!(exported_body.ends_with("# Second\n\nbody two.\n"));

    let active_body = fs::read_to_string(notes.join(format!("{second_id}.md"))).unwrap();
    assert_eq!(active_body, "# Second\n\nbody two.\n");

    let exported = run_nt(&home, &["export", full_archive.to_str().unwrap()]);
    assert_eq!(summary_ids(&exported), vec!["exported", "exported"]);
    assert!(full_archive.join(format!("{first_id}.md")).exists());
    assert!(full_archive.join(format!("{second_id}.md")).exists());

    assert_failed(
        &home,
        &["export", notes.to_str().unwrap()],
        "export path must be outside the active notes directory",
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn collection_and_status_commands_validate_and_update_index_only() {
    let root = temp_dir("metadata-validation");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let first = run_nt_with_stdin(&home, &["add"], "# First\n\nbody one.\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let second = run_nt_with_stdin(&home, &["add"], "# Second\n\nbody two.\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();

    assert_failed(
        &home,
        &["update", "bad-id", "collection", "+projects/nt"],
        "invalid note id",
    );
    assert_failed(
        &home,
        &["update", first_id, "collection", "+Projects/nt"],
        "invalid collection",
    );
    assert_failed(
        &home,
        &["update", first_id, "collection", "+projects,nt"],
        "without spaces or commas",
    );
    assert_failed(
        &home,
        &["update", first_id, "tag", "+Storage"],
        "invalid tag",
    );
    assert_failed(
        &home,
        &["update", first_id, "kind", "unknown"],
        "invalid kind",
    );
    assert_failed(
        &home,
        &["update", first_id, "status", "blocked"],
        "invalid status",
    );

    let collected = run_nt(&home, &["update", first_id, "collection", "+projects/nt"]);
    assert_eq!(
        collected.trim(),
        format!("updated {first_id} collection +projects/nt")
    );
    let collected_again = run_nt(&home, &["update", first_id, "collection", "+projects/nt"]);
    assert_eq!(
        collected_again.trim(),
        format!("updated {first_id} collection +projects/nt")
    );

    run_nt(&home, &["update", first_id, "kind", "todo"]);
    run_nt(&home, &["update", first_id, "status", "open"]);
    run_nt(&home, &["update", second_id, "kind", "todo"]);
    run_nt(&home, &["update", second_id, "status", "waiting"]);

    let first_body = fs::read_to_string(notes.join(format!("{first_id}.md"))).unwrap();
    assert_eq!(first_body, "# First\n\nbody one.\n");

    let collections = run_nt(&home, &["list", "collections"]);
    assert_eq!(collections.trim(), "projects/nt");

    let collection = run_nt(&home, &["find", "collection:projects/nt"]);
    assert_eq!(summary_ids(&collection), vec![first_id]);

    let status = run_nt(&home, &["agenda"]);
    assert_eq!(note_ids(&status), vec![second_id, first_id]);

    let cleared = run_nt(&home, &["update", first_id, "status", "-"]);
    assert_eq!(cleared.trim(), format!("updated {first_id} status -"));
    let status = run_nt(&home, &["agenda"]);
    assert_eq!(note_ids(&status), vec![second_id]);

    let index = read_index(&home);
    assert_eq!(index["notes"][first_id]["status"], serde_json::Value::Null);
    assert_eq!(
        index["notes"][first_id]["collections"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        index["collections"]["projects/nt"].as_array().unwrap(),
        &vec![serde_json::Value::String(first_id.to_string())]
    );
    assert_eq!(
        index["kinds"]["todo"].as_array().unwrap(),
        &vec![
            serde_json::Value::String(second_id.to_string()),
            serde_json::Value::String(first_id.to_string()),
        ]
    );
    assert!(index["statuses"].get("open").is_none());

    let uncollected = run_nt(&home, &["update", first_id, "collection", "-projects/nt"]);
    assert_eq!(
        uncollected.trim(),
        format!("updated {first_id} collection -projects/nt")
    );
    let uncollected_again = run_nt(&home, &["update", first_id, "collection", "-projects/nt"]);
    assert_eq!(
        uncollected_again.trim(),
        format!("updated {first_id} collection -projects/nt")
    );

    let index = read_index(&home);
    assert!(
        index["notes"][first_id]["collections"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(index["collections"].as_object().unwrap().is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn failed_updates_leave_index_bytes_unchanged() {
    let root = temp_dir("update-atomic-validation");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);
    let saved = run_nt_with_stdin(&home, &["add"], "# Stable\n\nbody.\n");
    let id = saved.trim().strip_prefix("saved ").unwrap();
    let index_path = home.join(".nt/index.json");
    let original = fs::read(&index_path).unwrap();

    for (field, value, expected) in [
        ("kind", "unknown", "invalid kind"),
        ("status", "blocked", "invalid status"),
        ("priority", "X", "invalid priority"),
        ("scheduled", "2026-02-29", "invalid date"),
        ("due", "2026-13-01", "invalid date"),
        ("tag", "storage", "requires +value or -value"),
        ("collection", "+Projects/nt", "invalid collection"),
        (
            "link",
            "+NT20990101T000000",
            "note not found: NT20990101T000000",
        ),
        (
            "source",
            "https://example.com/spec",
            "requires +value or -value",
        ),
    ] {
        assert_failed(&home, &["update", id, field, value], expected);
        assert_eq!(fs::read(&index_path).unwrap(), original);
    }

    assert_failed(
        &home,
        &["update", id, "topic", "value"],
        "invalid value 'topic'",
    );
    assert_eq!(fs::read(&index_path).unwrap(), original);
    assert_failed(
        &home,
        &["update", "bad-id", "status", "open"],
        "invalid note id",
    );
    assert_eq!(fs::read(&index_path).unwrap(), original);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn agenda_metadata_round_trips_through_commands_export_and_rebuild() {
    let root = temp_dir("agenda-metadata-round-trip");
    let home = root.join("home");
    let notes = root.join("notes");
    let archive = root.join("archive");

    run_nt(&home, &["init", notes.to_str().unwrap()]);
    let saved = run_nt_with_stdin(
        &home,
        &[
            "add",
            "kind:todo",
            "status:open",
            "priority:A",
            "scheduled:2099-06-25",
            "due:2099-06-30",
        ],
        "# Future task\n\nbody.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("priority A\n"));
    assert!(shown.contains("scheduled 2099-06-25\n"));
    assert!(shown.contains("due 2099-06-30\n"));
    assert!(shown.contains("closed -\n"));
    let found = run_nt(
        &home,
        &[
            "find",
            "priority:A",
            "scheduled:2099-06-25",
            "due:2099-06-30",
        ],
    );
    assert_eq!(summary_ids(&found), vec![id]);

    run_nt(&home, &["update", id, "status", "done"]);
    let index = read_index(&home);
    let closed = index["notes"][id]["closed"].as_str().unwrap().to_string();
    let closed_day = &closed[..10];
    let found = run_nt(
        &home,
        &["find", "status:done", &format!("closed:{closed_day}")],
    );
    assert_eq!(summary_ids(&found), vec![id]);

    run_nt(&home, &["export", archive.to_str().unwrap(), id]);
    let exported = fs::read_to_string(archive.join(format!("{id}.md"))).unwrap();
    assert!(exported.contains("priority: \"A\"\n"));
    assert!(exported.contains("scheduled: \"2099-06-25\"\n"));
    assert!(exported.contains("due: \"2099-06-30\"\n"));
    assert!(exported.contains(&format!("closed: \"{closed}\"\n")));

    assert_eq!(run_nt(&home, &["rebuild"]).trim(), "rebuilt 1");
    let rebuilt = read_index(&home);
    assert_eq!(rebuilt["notes"][id]["priority"].as_str(), Some("A"));
    assert_eq!(
        rebuilt["notes"][id]["scheduled"].as_str(),
        Some("2099-06-25")
    );
    assert_eq!(rebuilt["notes"][id]["due"].as_str(), Some("2099-06-30"));
    assert_eq!(
        rebuilt["notes"][id]["closed"].as_str(),
        Some(closed.as_str())
    );

    run_nt(&home, &["update", id, "status", "open"]);
    assert!(read_index(&home)["notes"][id]["closed"].is_null());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn add_accepts_creation_metadata() {
    let root = temp_dir("add-creation-metadata");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let first = run_nt_with_stdin(&home, &["add"], "# First source\n\nbody one.\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let second = run_nt_with_stdin(&home, &["add"], "# Second source\n\nbody two.\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();
    let links = format!("link:{first_id},{second_id}");

    let saved = run_nt_with_stdin(
        &home,
        &[
            "add",
            "tag:qemu,firecracker",
            "tag:research",
            "kind:decision",
            "status:open",
            "collection:projects/nt",
            "source:https://manual.example/spec",
            &links,
        ],
        "# VM decision\n\nPrefer visible metadata at creation time: https://example.com/vm.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("kind decision"));
    assert!(shown.contains("status open"));
    assert!(shown.contains("tags firecracker,qemu,research"));
    assert!(shown.contains("collections projects/nt"));
    assert!(shown.contains(&format!("links {first_id},{second_id}")));
    assert!(shown.contains("sources https://example.com/vm,https://manual.example/spec"));

    let tags = run_nt(&home, &["list", "tags"]);
    assert_eq!(tags, "firecracker\nqemu\nresearch\n");

    let found = run_nt(
        &home,
        &[
            "find",
            "tag:qemu",
            "kind:decision",
            "status:open",
            "collection:projects/nt",
            "source:example.com/vm",
        ],
    );
    assert!(found.contains(id));

    let backlinks = run_nt(&home, &["list", "links", &format!("to:{first_id}")]);
    assert!(backlinks.contains(&format!("{id}\tVM decision\t{first_id}\tFirst source")));

    let related = run_nt(&home, &["list", "links", &format!("from:{id}")]);
    assert!(related.contains(&format!("{id}\tVM decision\t{first_id}\tFirst source")));
    assert!(related.contains(&format!("{id}\tVM decision\t{second_id}\tSecond source")));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn add_rejects_invalid_creation_metadata_tokens() {
    let root = temp_dir("add-invalid-metadata");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    assert_failed_with_stdin(
        &home,
        &["add", "tag:Storage"],
        "# Invalid\n\nbody.\n",
        "invalid tag",
    );
    assert_failed_with_stdin(
        &home,
        &["add", "collection:Projects/nt"],
        "# Invalid\n\nbody.\n",
        "invalid collection",
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn find_supports_documented_query_forms() {
    let root = temp_dir("find-query-syntax");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let saved = run_nt_with_stdin(
        &home,
        &[
            "add",
            "tag:qemu",
            "kind:decision",
            "status:open",
            "collection:projects/nt",
            "source:https://firecracker.example/spec",
        ],
        "# QEMU Decision\n\nMicroVM jailer details.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();
    let day = format!("{}-{}-{}", &id[2..6], &id[6..8], &id[8..10]);
    let prefix = &id[..10];

    let draft = run_nt_with_stdin(
        &home,
        &["add", "tag:qemu", "tag:draft"],
        "# Draft\n\nQEMU draft note.\n",
    );
    let draft_id = draft.trim().strip_prefix("saved ").unwrap();

    let found = run_nt(&home, &["find", "qemu", "firecracker"]);
    assert!(found.contains(id));

    let found = run_nt(&home, &["find", "#qemu", &format!("id:{prefix}")]);
    assert!(found.contains(id));

    let found = run_nt(
        &home,
        &[
            "find",
            "title:decision",
            "kind:decision",
            "status:open",
            "collection:projects/nt",
            "source:firecracker",
            &format!("day:{day}"),
            "since:1970-01-01",
            "before:9999-01-01",
        ],
    );
    assert!(found.contains(id));

    let found = run_nt(&home, &["find", "body:microvm jailer"]);
    assert!(found.contains(id));

    let found = run_nt(&home, &["find", "not:tag:draft", "qemu"]);
    assert!(found.contains(id));
    assert!(!found.contains(draft_id));

    let found = run_nt(&home, &["find", "qemu", "before:1970-01-01"]);
    assert!(found.trim().is_empty());

    assert_failed(
        &home,
        &["find", "collectiom:projects/nt"],
        "did you mean `collection`",
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn find_uses_visible_body_term_indexes() {
    let root = temp_dir("find-body-index");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let saved = run_nt_with_stdin(
        &home,
        &["add"],
        "# Runtime Heading\n\nAlpha-only body term with beta details.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let index = read_index(&home);
    assert_eq!(
        index["body_terms"]["alpha"].as_array().unwrap(),
        &vec![serde_json::Value::String(id.to_string())]
    );
    assert_eq!(
        index["heading_terms"]["runtime"].as_array().unwrap(),
        &vec![serde_json::Value::String(id.to_string())]
    );
    assert!(
        !serde_json::to_string(&index)
            .unwrap()
            .contains("Alpha-only body term")
    );

    let body_found = run_nt(&home, &["find", "body:alpha"]);
    assert_eq!(summary_ids(&body_found), vec![id]);

    let bare_found = run_nt(&home, &["find", "beta"]);
    assert_eq!(summary_ids(&bare_found), vec![id]);

    let heading_found = run_nt(&home, &["find", "body:runtime"]);
    assert_eq!(summary_ids(&heading_found), vec![id]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn find_skips_body_file_reads_when_indexed_candidates_are_empty() {
    let root = temp_dir("find-empty-index-candidates");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let saved = run_nt_with_stdin(&home, &["add"], "# Indexed\n\noldterm only.\n");
    let id = saved.trim().strip_prefix("saved ").unwrap();
    fs::remove_file(notes.join(format!("{id}.md"))).unwrap();

    let found = run_nt(&home, &["find", "body:missingterm"]);

    assert!(found.trim().is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn find_reads_body_files_for_missing_body_index_entries() {
    let root = temp_dir("find-missing-body-index");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let saved = run_nt_with_stdin(
        &home,
        &["add"],
        "# Unindexed\n\nfallbackonlyterm lives only in the Markdown body.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let mut index = read_index(&home);
    index["body_terms"]
        .as_object_mut()
        .unwrap()
        .remove("fallbackonlyterm");
    index["body_indexed"]
        .as_array_mut()
        .unwrap()
        .retain(|value| value.as_str() != Some(id));
    write_index(&home, &index);

    let found = run_nt(&home, &["find", "body:fallbackonlyterm"]);

    assert_eq!(summary_ids(&found), vec![id]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn find_trusts_stale_indexed_body_terms_until_rebuild() {
    let root = temp_dir("find-stale-body-index");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let saved = run_nt_with_stdin(&home, &["add"], "# Indexed\n\nneedle appears here.\n");
    let id = saved.trim().strip_prefix("saved ").unwrap();
    fs::write(
        notes.join(format!("{id}.md")),
        "# Indexed\n\nchanged body.\n",
    )
    .unwrap();

    let found = run_nt(&home, &["find", "body:needle"]);

    assert_eq!(summary_ids(&found), vec![id]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn find_preserves_active_recent_order_with_index_candidates() {
    let root = temp_dir("find-candidate-order");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let first = run_nt_with_stdin(&home, &["add"], "# First\n\nsharedorder term.\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let second = run_nt_with_stdin(&home, &["add"], "# Second\n\nsharedorder term.\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();
    let third = run_nt_with_stdin(&home, &["add"], "# Third\n\nsharedorder term.\n");
    let third_id = third.trim().strip_prefix("saved ").unwrap();

    let found = run_nt(&home, &["find", "body:sharedorder"]);

    assert_eq!(summary_ids(&found), vec![third_id, second_id, first_id]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn body_multiword_queries_match_all_terms_not_exact_phrase() {
    let root = temp_dir("find-body-all-terms");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let separated = run_nt_with_stdin(
        &home,
        &["add"],
        "# Separated\n\nThe jailer notes mention another microvm detail later.\n",
    );
    let separated_id = separated.trim().strip_prefix("saved ").unwrap();
    let missing = run_nt_with_stdin(
        &home,
        &["add"],
        "# Missing\n\nThis microvm note omits the other term.\n",
    );
    let missing_id = missing.trim().strip_prefix("saved ").unwrap();

    let found = run_nt(&home, &["find", "body:microvm jailer"]);

    assert_eq!(summary_ids(&found), vec![separated_id]);
    assert!(found.contains(separated_id));
    assert!(!found.contains(missing_id));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rm_removes_deleted_ids_from_text_indexes() {
    let root = temp_dir("rm-text-index");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let removed = run_nt_with_stdin(&home, &["add"], "# Removed\n\nshared uniquegone.\n");
    let removed_id = removed.trim().strip_prefix("saved ").unwrap();
    let kept = run_nt_with_stdin(&home, &["add"], "# Kept\n\nshared uniquekept.\n");
    let kept_id = kept.trim().strip_prefix("saved ").unwrap();

    run_nt(&home, &["rm", removed_id]);

    let index = read_index(&home);
    assert_eq!(
        index["body_terms"]["shared"].as_array().unwrap(),
        &vec![serde_json::Value::String(kept_id.to_string())]
    );
    assert!(index["body_terms"].get("uniquegone").is_none());
    assert!(index["heading_terms"].get("removed").is_none());

    let found = run_nt(&home, &["find", "shared"]);
    assert_eq!(summary_ids(&found), vec![kept_id]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rm_removes_multiple_notes_and_cleans_links() {
    let root = temp_dir("rm-multiple");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let first = run_nt_with_stdin(&home, &["add"], "# First\n\nshared firstonly.\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let second = run_nt_with_stdin(&home, &["add"], "# Second\n\nshared secondonly.\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();
    let kept = run_nt_with_stdin(
        &home,
        &[
            "add",
            &format!("link:{first_id}"),
            &format!("link:{second_id}"),
        ],
        "# Kept\n\nshared keptonly.\n",
    );
    let kept_id = kept.trim().strip_prefix("saved ").unwrap();

    let removed = run_nt(&home, &["rm", first_id, second_id]);

    assert_eq!(
        removed,
        format!("removed {first_id}\nremoved {second_id}\n")
    );
    assert!(!notes.join(format!("{first_id}.md")).exists());
    assert!(!notes.join(format!("{second_id}.md")).exists());

    let index = read_index(&home);
    assert!(index["notes"].get(first_id).is_none());
    assert!(index["notes"].get(second_id).is_none());
    assert_eq!(index["notes"][kept_id]["links"], serde_json::json!([]));
    assert_eq!(index["body_terms"]["shared"], serde_json::json!([kept_id]));
    assert!(index["body_terms"].get("firstonly").is_none());
    assert!(index["body_terms"].get("secondonly").is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rm_validates_every_id_before_removing_notes() {
    let root = temp_dir("rm-prevalidate");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let saved = run_nt_with_stdin(&home, &["add"], "# Kept\n");
    let id = saved.trim().strip_prefix("saved ").unwrap();
    let missing = "NT20260101T000000";

    assert_failed(
        &home,
        &["rm", id, missing],
        &format!("note not found: {missing}"),
    );
    assert!(notes.join(format!("{id}.md")).exists());
    assert!(read_index(&home)["notes"].get(id).is_some());

    assert_failed(&home, &["rm", id, "invalid"], "invalid note id: invalid");
    assert!(notes.join(format!("{id}.md")).exists());
    assert!(read_index(&home)["notes"].get(id).is_some());

    assert_failed(&home, &["rm", id, id], &format!("duplicate note id: {id}"));
    assert!(notes.join(format!("{id}.md")).exists());
    assert!(read_index(&home)["notes"].get(id).is_some());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn find_reports_missing_note_bodies() {
    let root = temp_dir("find-missing-body");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let saved = run_nt_with_stdin(&home, &["add"], "# Missing\n\nbodyonlyterm.\n");
    let id = saved.trim().strip_prefix("saved ").unwrap();
    fs::remove_file(notes.join(format!("{id}.md"))).unwrap();

    assert_failed(
        &home,
        &["find", "body:bodyonlyterm"],
        &format!("note body not readable for {id}"),
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn common_mistakes_fail_cleanly() {
    let root = temp_dir("common-mistakes");
    let home = root.join("home");
    let notes = root.join("notes");
    let uninitialized_home = root.join("uninitialized-home");

    assert_failed(&home, &["find"], "Usage:");
    assert_failed(
        &home,
        &["help", "unknown"],
        "unknown help topic `unknown`; run `nt help`",
    );
    assert_failed(
        &uninitialized_home,
        &["rebuild"],
        "run `nt init <notes-dir>` first",
    );

    run_nt(&home, &["init", notes.to_str().unwrap()]);
    assert_failed_with_stdin(
        &home,
        &["add"],
        "## Section is not a title\n",
        "note must start with a non-empty `# Title` heading",
    );
    assert!(run_nt(&home, &["list", "ids"]).trim().is_empty());
    assert_failed(
        &home,
        &["find", "collectiom:projects/nt"],
        "unknown query field `collectiom`; did you mean `collection`?",
    );
    assert_failed(&home, &["show", "bad-id"], "invalid note id");

    let _ = fs::remove_dir_all(root);
}

fn assert_failed(home: &PathBuf, args: &[&str], expected: &str) {
    let output = Command::new(nt_bin())
        .env("HOME", home)
        .args(args)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "nt {:?} unexpectedly succeeded:\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "nt {:?} stderr did not contain {:?}:\n{}",
        args,
        expected,
        stderr
    );
}

fn assert_failed_with_stdin(home: &PathBuf, args: &[&str], stdin: &str, expected: &str) {
    let mut child = Command::new(nt_bin())
        .env("HOME", home)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(
        !output.status.success(),
        "nt {:?} unexpectedly succeeded:\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "nt {:?} stderr did not contain {:?}:\n{}",
        args,
        expected,
        stderr
    );
}

fn assert_failed_with_env(home: &PathBuf, args: &[&str], env: &[(&str, &str)], expected: &str) {
    let mut command = Command::new(nt_bin());
    command.env("HOME", home).args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command.output().unwrap();

    assert!(
        !output.status.success(),
        "nt {args:?} unexpectedly succeeded"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "nt {args:?} stderr did not contain {expected:?}:\n{stderr}"
    );
}

fn read_index(home: &Path) -> serde_json::Value {
    let index = fs::read_to_string(home.join(".nt/index.json")).unwrap();
    serde_json::from_str(&index).unwrap()
}

fn write_index(home: &Path, index: &serde_json::Value) {
    let bytes = serde_json::to_vec_pretty(index).unwrap();
    fs::write(home.join(".nt/index.json"), bytes).unwrap();
}

fn summary_ids(output: &str) -> Vec<&str> {
    output
        .lines()
        .map(|line| line.split_whitespace().next().unwrap())
        .collect()
}

fn note_ids(output: &str) -> Vec<&str> {
    summary_ids(output)
        .into_iter()
        .filter(|value| is_valid_note_id(value))
        .collect()
}

fn is_valid_note_id(id: &str) -> bool {
    id.len() == 17
        && id.starts_with("NT")
        && id.as_bytes()[10] == b'T'
        && id[2..10].chars().all(|ch| ch.is_ascii_digit())
        && id[11..17].chars().all(|ch| ch.is_ascii_digit())
}

fn markdown_section(markdown: &str, heading: &str) -> String {
    let mut section = String::new();
    let mut in_section = false;
    let heading_level = heading.chars().take_while(|ch| *ch == '#').count();

    for line in markdown.lines() {
        if line == heading {
            in_section = true;
            section.push_str(line);
            section.push('\n');
            continue;
        }

        if in_section {
            let line_level = line.chars().take_while(|ch| *ch == '#').count();
            if line_level > 0
                && line_level <= heading_level
                && line.as_bytes().get(line_level) == Some(&b' ')
            {
                break;
            }
            section.push_str(line);
            section.push('\n');
        }
    }

    section
}

fn nt_commands_in_shell_blocks(markdown: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut in_shell_block = false;

    for line in markdown.lines() {
        if let Some(language) = line.trim_start().strip_prefix("```") {
            let language = language.trim();
            in_shell_block = !in_shell_block && matches!(language, "sh" | "bash");
            if language.is_empty() {
                in_shell_block = false;
            }
            continue;
        }

        if in_shell_block {
            commands.extend(nt_commands_in_line(line));
        }
    }

    commands
}

fn nt_commands_in_line(line: &str) -> Vec<String> {
    let tokens: Vec<String> = line.split_whitespace().map(clean_shell_token).collect();
    let mut commands = Vec::new();

    for pair in tokens.windows(2) {
        if pair[0] == "nt" && !pair[1].starts_with('<') {
            commands.push(pair[1].clone());
        }
    }

    commands
}

fn paragraph_mentions_nt_command(paragraph: &str, command: &str) -> bool {
    let Some(command) = command.strip_prefix("nt ") else {
        return false;
    };

    paragraph
        .lines()
        .flat_map(nt_commands_in_line)
        .any(|found| found == command)
}

fn clean_shell_token(token: &str) -> String {
    token
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '<' && ch != '>' && ch != '-')
        .to_string()
}

fn run_nt(home: &PathBuf, args: &[&str]) -> String {
    let output = Command::new(nt_bin())
        .env("HOME", home)
        .args(args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "nt {:?} failed:\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap()
}

fn run_nt_with_env(home: &PathBuf, args: &[&str], env: &[(&str, &str)]) -> String {
    let mut command = Command::new(nt_bin());
    command.env("HOME", home).args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "nt {:?} failed:\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap()
}

fn run_nt_with_stdin(home: &PathBuf, args: &[&str], stdin: &str) -> String {
    let mut child = Command::new(nt_bin())
        .env("HOME", home)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "nt {:?} failed:\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nt-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

const ROOT_COMMANDS: &[&str] = &[
    "init",
    "add",
    "rebuild",
    "list",
    "find",
    "show",
    "open",
    "rm",
    "update",
    "agenda",
    "export",
    "config",
    "completion",
    "help",
];

const UNSUPPORTED_ROOT_COMMAND_EXAMPLES: &[&str] = &[
    "nt pick",
    "nt tui",
    "nt search",
    "nt grep",
    "nt graph",
    "nt edit",
    "nt browse",
    "nt agent",
    "nt discuss",
    "nt run",
    "nt version",
];

const DOC_PATHS: &[&str] = &[
    "README.md",
    "docs/usage.md",
    "docs/cli-reference.md",
    "docs/design.md",
];
