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
