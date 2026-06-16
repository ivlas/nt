use std::fs;
use std::io::Write;
use std::path::PathBuf;
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
    let first = run_nt_with_stdin(&home, &["add"], "# First vault\n\nbody one.\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap().to_string();
    run_nt(&home, &["status", &first_id, "open"]);

    run_nt(&home, &["init", research.to_str().unwrap()]);
    let second = run_nt_with_stdin(&home, &["add"], "# Second vault\n\nbody two.\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap().to_string();
    run_nt(&home, &["status", &second_id, "open"]);

    let vaults = run_nt(&home, &["config", "vault"]);
    assert!(vaults.contains(&format!("- notes {}", notes.display())));
    assert!(vaults.contains(&format!("* research {}", research.display())));

    let listed = run_nt(&home, &["list"]);
    assert!(listed.contains(&second_id));
    assert!(!listed.contains(&first_id));
    let status = run_nt(&home, &["status"]);
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
    let status = run_nt(&home, &["status"]);
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

    run_nt(&home, &["tag", first_id, "storage"]);
    run_nt(&home, &["collect", first_id, "projects/nt"]);
    run_nt(&home, &["kind", first_id, "decision"]);
    run_nt(&home, &["status", first_id, "open"]);
    run_nt(&home, &["link", first_id, deleted_id]);

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
    assert!(bash.contains("init add rebuild list find show edit"));
    assert!(bash.contains("_nt_note_ids"));
    assert!(bash.contains("nt ids 2>/dev/null"));

    let zsh = run_nt(&home, &["completion", "zsh"]);
    assert!(zsh.contains("'show:'"));
    assert!(zsh.contains(":id:_nt_note_ids"));
    assert!(zsh.contains("nt ids 2>/dev/null"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn help_is_a_flagless_command_with_examples() {
    let root = temp_dir("help-command");
    let home = root.join("home");

    let root_help = run_nt(&home, &["help"]);
    assert!(root_help.contains("nt <command> [positional...]"));
    assert!(root_help.contains("nt help <command>"));
    assert!(root_help.contains("Examples:"));

    let find_help = run_nt(&home, &["help", "find"]);
    assert!(find_help.contains("nt find <expr...>"));
    assert!(find_help.contains("nt find tag:decision collection:projects/nt"));
    assert!(
        find_help.contains(
            "Quoted multiword body: values match all\nindexed terms, not an exact phrase"
        )
    );
    assert!(!find_help.contains("exact phrase search"));

    let vault_help = run_nt(&home, &["help", "config", "vault"]);
    assert!(vault_help.contains("nt config vault [vault-name]"));
    assert!(vault_help.contains("nt config vault notes"));
    let rebuild_help = run_nt(&home, &["help", "rebuild"]);
    assert!(
        rebuild_help.contains(
            "preserves existing sources and merges\nURLs currently found in Markdown body"
        )
    );

    assert_failed(&home, &["--help"], "unexpected argument '--help'");
    assert_failed(&home, &["list", "--help"], "unexpected argument '--help'");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebuild_docs_document_persistent_source_semantics() {
    let expected = "preserves existing sources and merges URLs currently found in";
    for path in ["docs/cli-syntax-spec.md", "docs/usage.md", "README.md"] {
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
fn find_docs_document_body_terms_not_phrase_search() {
    let expected = "Quoted multiword `body:` values match all indexed terms, not an exact phrase.";
    for path in [
        "docs/cli-syntax-spec.md",
        "docs/usage.md",
        "docs/design.md",
        "README.md",
    ] {
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

    let syntax = fs::read_to_string("docs/cli-syntax-spec.md").unwrap();
    let syntax = syntax.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(syntax.contains("The visible `heading_terms` index is for future/internal use"));
    assert!(syntax.contains("there is no `heading:<term>` query field yet."));
}

#[cfg(unix)]
#[test]
fn edit_uses_editor_and_updates_visible_note() {
    let root = temp_dir("edit-editor");
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

    let edited = run_nt_with_env(
        &home,
        &["edit", &id],
        &[("EDITOR", editor.to_str().unwrap())],
    );
    assert_eq!(edited.trim(), format!("saved {id}"));

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

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("tags plain"));
    assert!(!shown.contains("\x1b["));

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

    run_nt(&home, &["collect", first_id, "projects/nt"]);
    run_nt(&home, &["tag", first_id, "storage"]);
    run_nt(&home, &["kind", first_id, "decision"]);
    run_nt(&home, &["status", first_id, "open"]);
    run_nt(&home, &["link", first_id, second_id]);

    let tags = run_nt(&home, &["tags"]);
    assert!(tags.contains("storage\t1"));

    let collections = run_nt(&home, &["collections"]);
    assert_eq!(collections.trim(), "projects/nt");

    let collection = run_nt(&home, &["collection", "projects/nt"]);
    assert!(collection.contains(first_id));

    let status = run_nt(&home, &["status"]);
    assert!(status.contains(first_id));

    let links = run_nt(&home, &["links", first_id, "out"]);
    assert_eq!(links.trim(), second_id);

    let backlinks = run_nt(&home, &["links", second_id, "in"]);
    assert_eq!(backlinks.trim(), first_id);

    let self_links = run_nt(&home, &["links", second_id, "self"]);
    assert_eq!(self_links.trim(), format!("in {first_id}"));

    let all_links = run_nt(&home, &["links", first_id, "all"]);
    assert_eq!(all_links.trim(), format!("1 out {second_id}"));

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

    run_nt(&home, &["unlink", first_id, second_id]);
    run_nt(&home, &["untag", first_id, "storage"]);
    run_nt(&home, &["uncollect", first_id, "projects/nt"]);

    let links = run_nt(&home, &["links", first_id, "out"]);
    assert!(links.trim().is_empty());

    let collection = run_nt(&home, &["collection", "projects/nt"]);
    assert!(collection.trim().is_empty());

    let tags = run_nt(&home, &["tags"]);
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
        &["collect", "bad-id", "projects/nt"],
        "invalid note id",
    );
    assert_failed(
        &home,
        &["collect", first_id, "Projects/nt"],
        "invalid collection",
    );
    assert_failed(
        &home,
        &["collect", first_id, "projects,nt"],
        "without spaces or commas",
    );
    assert_failed(&home, &["tag", first_id, "Storage"], "invalid tag");
    assert_failed(&home, &["kind", first_id, "unknown"], "invalid kind");
    assert_failed(&home, &["status", first_id, "blocked"], "invalid status");

    let collected = run_nt(&home, &["collect", first_id, "projects/nt"]);
    assert_eq!(
        collected.trim(),
        format!("collected {first_id} projects/nt")
    );
    let collected_again = run_nt(&home, &["collect", first_id, "projects/nt"]);
    assert_eq!(
        collected_again.trim(),
        format!("collected {first_id} projects/nt")
    );

    run_nt(&home, &["kind", first_id, "todo"]);
    run_nt(&home, &["status", first_id, "open"]);
    run_nt(&home, &["status", second_id, "waiting"]);

    let first_body = fs::read_to_string(notes.join(format!("{first_id}.md"))).unwrap();
    assert_eq!(first_body, "# First\n\nbody one.\n");

    let collections = run_nt(&home, &["collections"]);
    assert_eq!(collections.trim(), "projects/nt");

    let collection = run_nt(&home, &["collection", "projects/nt"]);
    assert_eq!(summary_ids(&collection), vec![first_id]);

    let status = run_nt(&home, &["status"]);
    assert_eq!(summary_ids(&status), vec![second_id, first_id]);

    let cleared = run_nt(&home, &["status", first_id, "-"]);
    assert_eq!(cleared.trim(), format!("status {first_id} -"));
    let status = run_nt(&home, &["status"]);
    assert_eq!(summary_ids(&status), vec![second_id]);

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
        &vec![serde_json::Value::String(first_id.to_string())]
    );
    assert!(index["statuses"].get("open").is_none());

    let uncollected = run_nt(&home, &["uncollect", first_id, "projects/nt"]);
    assert_eq!(
        uncollected.trim(),
        format!("uncollected {first_id} projects/nt")
    );
    let uncollected_again = run_nt(&home, &["uncollect", first_id, "projects/nt"]);
    assert_eq!(
        uncollected_again.trim(),
        format!("uncollected {first_id} projects/nt")
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

    let tags = run_nt(&home, &["tags"]);
    assert!(tags.contains("firecracker\t1"));
    assert!(tags.contains("qemu\t1"));
    assert!(tags.contains("research\t1"));

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

    let backlinks = run_nt(&home, &["links", first_id, "in"]);
    assert_eq!(backlinks.trim(), id);

    let self_links = run_nt(&home, &["links", id, "self"]);
    assert_eq!(
        self_links.trim(),
        format!("out {first_id}\nout {second_id}")
    );

    let all_links = run_nt(&home, &["links", id, "all"]);
    assert_eq!(
        all_links.trim(),
        format!("1 out {first_id}\n1 out {second_id}")
    );

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

fn read_index(home: &PathBuf) -> serde_json::Value {
    let index = fs::read_to_string(home.join(".nt/index.json")).unwrap();
    serde_json::from_str(&index).unwrap()
}

fn write_index(home: &PathBuf, index: &serde_json::Value) {
    let bytes = serde_json::to_vec_pretty(index).unwrap();
    fs::write(home.join(".nt/index.json"), bytes).unwrap();
}

fn summary_ids(output: &str) -> Vec<&str> {
    output
        .lines()
        .map(|line| line.split_whitespace().next().unwrap())
        .collect()
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
