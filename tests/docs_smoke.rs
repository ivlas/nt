use std::collections::BTreeSet;
use std::fs;

#[test]
fn readme_quickstart_uses_supported_commands_and_explains_placeholder_id() {
    let readme = fs::read_to_string("README.md").unwrap();
    let quickstart = markdown_section(&readme, "## Quick Start");

    assert!(quickstart.contains("`nt note` prints a note id like `NT20260616T101500`."));
    assert!(quickstart.contains("nt show <id>"));
    assert!(quickstart.contains("nt open <id>"));

    for command in nt_commands_in_shell_blocks(&quickstart) {
        assert!(
            ROOT_COMMANDS.contains(&command.as_str()),
            "README quickstart uses unsupported nt command `{command}`"
        );
    }

    let quickstart_commands = quickstart
        .split("`nt note` prints a note id")
        .next()
        .unwrap();
    let commands = nt_commands_in_shell_blocks(quickstart_commands);
    assert_eq!(commands, vec!["init", "note", "find"]);
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
fn docs_describe_user_directed_agent_use_and_single_writer_mutations() {
    for path in ["docs/usage.md", "docs/cli-reference.md", "docs/design.md"] {
        let text = fs::read_to_string(path).unwrap();
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized.contains("user") && normalized.contains("direct"),
            "{path} should describe user-directed use"
        );
        assert!(
            normalized.contains("one user-directed writer at a time")
                || normalized.contains("one user-directed writer")
                || normalized.contains("single-writer CLI"),
            "{path} should document the single-writer mutation model"
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
    let usage = fs::read_to_string("docs/usage.md").unwrap();
    assert!(usage.contains("cargo install --path ."));

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
    for path in ["docs/cli-reference.md", "docs/usage.md"] {
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
    for path in ["docs/usage.md", "docs/cli-reference.md", "docs/design.md"] {
        let text = fs::read_to_string(path).unwrap();
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            normalized.contains("Indexed body entries are trusted until `nt rebuild`")
                || normalized.contains("indexed body entries are trusted until `nt rebuild`"),
            "{path} should document indexed body entries are trusted until rebuild"
        );
    }

    for path in ["docs/usage.md", "docs/design.md"] {
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

const ROOT_COMMANDS: &[&str] = &[
    "init",
    "note",
    "todo",
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
