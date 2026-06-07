use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
fn add_show_rebuild_show_uses_visible_storage() {
    let root = temp_dir("storage-smoke");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);
    assert!(home.join(".nt/AGENTS.md").exists());
    assert!(home.join(".nt/skills/nt-skill-builder/SKILL.md").exists());

    let saved = run_nt_with_stdin(
        &home,
        &["add"],
        "# Smoke Title\n\nbodyonlyterm from first paragraph.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("kind note"));
    assert!(shown.contains("status -"));
    assert!(shown.contains("bodyonlyterm"));

    run_nt(&home, &["rebuild"]);

    let shown_after_rebuild = run_nt(&home, &["show", id]);
    assert!(shown_after_rebuild.contains("Smoke Title"));
    assert!(shown_after_rebuild.contains("bodyonlyterm"));

    let index = fs::read_to_string(home.join(".nt/index.json")).unwrap();
    let index: serde_json::Value = serde_json::from_str(&index).unwrap();
    let term_ids = index["terms"]["bodyonlyterm"].as_array().unwrap();
    assert!(term_ids.iter().any(|value| value.as_str() == Some(id)));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn config_show_prints_agent_workspace_files() {
    let root = temp_dir("config-agent-workspace");
    let home = root.join("home");
    let notes = root.join("notes");

    run_nt(&home, &["init", notes.to_str().unwrap()]);

    let shown = run_nt(&home, &["config", "show"]);

    assert!(shown.contains("[agent]"));
    assert!(shown.contains("agent_workspace"));
    assert!(shown.contains("agents_md"));
    assert!(shown.contains("AGENTS.md"));
    assert!(shown.contains("skill nt-skill-builder"));

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

    let index = read_index(&home);
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
    assert!(
        index["statuses"]["open"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str() == Some(first_id))
    );

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
            &links,
        ],
        "# VM decision\n\nPrefer visible metadata at creation time.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("kind decision"));
    assert!(shown.contains("status open"));
    assert!(shown.contains("tags firecracker,qemu,research"));
    assert!(shown.contains("collections projects/nt"));
    assert!(shown.contains(&format!("links {first_id},{second_id}")));

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

fn read_index(home: &PathBuf) -> serde_json::Value {
    let index = fs::read_to_string(home.join(".nt/index.json")).unwrap();
    serde_json::from_str(&index).unwrap()
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
