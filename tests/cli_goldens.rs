use std::path::Path;
use std::process::{Command, Output, Stdio};

#[test]
fn help_output_matches_goldens() {
    let home = tempfile::tempdir().unwrap();

    assert_stdout(
        run(home.path(), &["help"]),
        include_bytes!("fixtures/root-help.txt"),
    );
    assert_stdout(
        run(home.path(), &["help", "memory"]),
        include_bytes!("fixtures/memory-help.txt"),
    );
}

fn run(home: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nt"))
        .env("HOME", home)
        .env("USERPROFILE", home)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn assert_stdout(output: Output, expected: &[u8]) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
}
