use std::collections::BTreeMap;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use crate::cli::{Cli, Command, ConfigCommand, SkillCommand};
use crate::completion::print_completion;
use crate::config::Config;
use crate::error::{NtError, Result};
use crate::fs::{absolute_path, atomic_write, nt_home, relative_to_cwd};
use crate::index::{Index, NoteMeta, NotebookMeta, terms_from_body};
use crate::note::{
    generate_unique_id, note_path, timestamp_from_id, timestamp_from_system_time, title_from_body,
    validate_id,
};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init { notes_dir } => init(&notes_dir),
        Command::Add => add(),
        Command::List => list(),
        Command::Show { id } => show(&id),
        Command::Edit { id } => edit(&id),
        Command::Find { query } => find(&query),
        Command::Ids => ids(),
        Command::Tags => tags(),
        Command::Rebuild => rebuild(),
        Command::Rm { id } => rm(&id),
        Command::Completion { shell } => {
            print_completion(shell);
            Ok(())
        }
        Command::Skill { command } => skill(command),
        Command::Config { command } => config(command),
        Command::Agent { prompt } => crate::agent::run(&prompt),
    }
}

fn init(notes_dir: &Path) -> Result<()> {
    let notes_dir = absolute_path(notes_dir)?;
    fs::create_dir_all(&notes_dir)?;
    fs::create_dir_all(nt_home()?)?;

    let mut index = Index::load()?;
    let timestamp = crate::note::timestamp_now();
    index.active_notes_dir = Some(notes_dir.clone());
    index
        .notebooks
        .entry(notes_dir.to_string_lossy().to_string())
        .or_insert(NotebookMeta {
            created: timestamp.iso,
        });
    refresh_derived_from_note_files(&mut index)?;
    index.save()?;

    println!("initialized {}", relative_to_cwd(&notes_dir).display());
    Ok(())
}

fn add() -> Result<()> {
    let mut index = Index::load()?;
    let notes_dir = active_notes_dir(&index)?.to_path_buf();
    let body = read_note_body_for_add()?;
    let timestamp = generate_unique_id(&notes_dir, &index)?;
    let path = note_path(&notes_dir, &timestamp.id)?;

    atomic_write(&path, body.as_bytes())?;

    index.upsert_note(NoteMeta::new_note(
        timestamp.id.clone(),
        path,
        timestamp.iso.clone(),
        timestamp.iso,
        title_from_body(&body),
    ));
    refresh_derived_from_note_files(&mut index)?;
    index.save()?;

    println!("saved {}", timestamp.id);
    Ok(())
}

fn list() -> Result<()> {
    let index = Index::load()?;

    for id in &index.recent {
        let Some(note) = index.notes.get(id) else {
            continue;
        };
        println!("{}", summary_line(note));
    }

    Ok(())
}

fn show(id: &str) -> Result<()> {
    validate_id(id)?;
    let index = Index::load()?;
    let note = index
        .notes
        .get(id)
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
    let body = fs::read_to_string(&note.path)?;

    println!("{}  {}", note.id, note.title);
    println!("path {}", relative_to_cwd(&note.path).display());
    println!("created {}", note.created);
    println!("updated {}", note.updated);
    println!("kind {}", note.kind);
    println!("status {}", note.status.as_deref().unwrap_or("-"));
    println!("tags {}", joined_or_dash(&note.tags));
    println!("collections {}", joined_or_dash(&note.collections));
    println!("links {}", joined_or_dash(&note.links));
    println!("refs {}", joined_or_dash(&note.refs));
    println!();
    print!("{body}");
    if !body.ends_with('\n') {
        println!();
    }

    Ok(())
}

fn edit(id: &str) -> Result<()> {
    validate_id(id)?;
    let mut index = Index::load()?;
    let note = index
        .notes
        .get(id)
        .cloned()
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let body = fs::read_to_string(&note.path)?;
    let edit_path = edit_temp_path(id)?;
    atomic_write(&edit_path, body.as_bytes())?;

    let status = ProcessCommand::new(&editor).arg(&edit_path).status()?;
    if !status.success() {
        let _ = fs::remove_file(&edit_path);
        return Err(NtError::EditorFailed(editor));
    }

    let body = fs::read_to_string(&edit_path)?;
    if body.trim().is_empty() {
        let _ = fs::remove_file(&edit_path);
        return Err(NtError::EmptyNote);
    }
    atomic_write(&note.path, body.as_bytes())?;
    let _ = fs::remove_file(&edit_path);

    let timestamp = crate::note::timestamp_now();
    let mut updated = note;
    updated.updated = timestamp.iso;
    updated.title = title_from_body(&body);

    index.upsert_note(updated);
    refresh_derived_from_note_files(&mut index)?;
    index.save()?;

    println!("saved {id}");
    Ok(())
}

fn find(query: &str) -> Result<()> {
    let index = Index::load()?;
    let needle = query.to_ascii_lowercase();

    for id in &index.recent {
        let Some(note) = index.notes.get(id) else {
            continue;
        };

        if matches_metadata(note, &needle) || matches_body(note, &needle) {
            println!("{}", summary_line(note));
        }
    }

    Ok(())
}

fn ids() -> Result<()> {
    let index = Index::load()?;
    for id in &index.recent {
        println!("{id}");
    }
    Ok(())
}

fn tags() -> Result<()> {
    let index = Index::load()?;
    for (tag, ids) in &index.tags {
        println!("{tag}\t{}", ids.len());
    }
    Ok(())
}

fn rebuild() -> Result<()> {
    let index = Index::load()?;
    let notes_dir = active_notes_dir(&index)?.to_path_buf();
    let rebuilt = rebuild_index_from_dir(&notes_dir, &index)?;

    rebuilt.save()?;
    println!("indexed {}", rebuilt.notes.len());
    Ok(())
}

fn rebuild_index_from_dir(notes_dir: &Path, previous_index: &Index) -> Result<Index> {
    let mut rebuilt = Index {
        active_notes_dir: Some(notes_dir.to_path_buf()),
        notebooks: previous_index.notebooks.clone(),
        ..Index::default()
    };

    for entry in fs::read_dir(&notes_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }

        let Some(id) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_string)
        else {
            continue;
        };

        if validate_id(&id).is_err() {
            continue;
        }

        let body = fs::read_to_string(&path)?;
        let from_id = timestamp_from_id(&id)?;
        let updated = file_updated_iso(&path)?.unwrap_or_else(|| from_id.iso.clone());
        let previous = previous_index.notes.get(&id);
        let mut note = NoteMeta::new_note(
            id.clone(),
            path,
            from_id.iso,
            updated,
            title_from_body(&body),
        );

        if let Some(previous) = previous {
            note.kind = previous.kind.clone();
            note.status = previous.status.clone();
            note.tags = previous.tags.clone();
            note.collections = previous.collections.clone();
            note.links = previous.links.clone();
            note.refs = previous.refs.clone();
        }

        rebuilt.notes.insert(id, note);
    }

    refresh_derived_from_note_files(&mut rebuilt)?;
    Ok(rebuilt)
}

fn rm(id: &str) -> Result<()> {
    validate_id(id)?;
    let mut index = Index::load()?;
    let note = index
        .notes
        .get(id)
        .cloned()
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;

    fs::remove_file(&note.path)?;
    index.remove_note(id);
    refresh_derived_from_note_files(&mut index)?;
    index.save()?;

    println!("removed {id}");
    Ok(())
}

fn skill(command: SkillCommand) -> Result<()> {
    match command {
        SkillCommand::Install => crate::skills::install(),
        SkillCommand::List => {
            crate::skills::list();
            Ok(())
        }
        SkillCommand::Show { name } => crate::skills::show(&name),
    }
}

fn config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show => Config::load()?.print(),
        ConfigCommand::AgentOutput { mode } => {
            let mut config = Config::load()?;
            config.agent.output = mode;
            config.save()?;
            println!("configured agent-output {}", agent_output_name(mode));
            Ok(())
        }
    }
}

fn agent_output_name(mode: crate::config::AgentOutputMode) -> &'static str {
    match mode {
        crate::config::AgentOutputMode::Hidden => "hidden",
        crate::config::AgentOutputMode::Format => "format",
        crate::config::AgentOutputMode::Full => "full",
    }
}

fn read_note_body_for_add() -> Result<String> {
    let mut body = String::new();

    if !io::stdin().is_terminal() {
        io::stdin().read_to_string(&mut body)?;
    } else {
        body = read_from_editor()?;
    }

    if body.trim().is_empty() {
        return Err(NtError::EmptyNote);
    }

    if !body.ends_with('\n') {
        body.push('\n');
    }

    Ok(body)
}

fn read_from_editor() -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let path = add_temp_path()?;
    atomic_write(&path, b"")?;

    let status = ProcessCommand::new(&editor).arg(&path).status()?;
    if !status.success() {
        let _ = fs::remove_file(&path);
        return Err(NtError::EditorFailed(editor));
    }

    let body = fs::read_to_string(&path)?;
    fs::remove_file(&path)?;
    Ok(body)
}

fn add_temp_path() -> Result<PathBuf> {
    editor_temp_path("add", None)
}

fn edit_temp_path(id: &str) -> Result<PathBuf> {
    editor_temp_path("edit", Some(id))
}

fn editor_temp_path(action: &str, id: Option<&str>) -> Result<PathBuf> {
    let dir = nt_home()?;
    fs::create_dir_all(&dir)?;
    let file_name = match id {
        Some(id) => format!(".nt-{action}-{id}-{}.tmp", std::process::id()),
        None => format!(".nt-{action}-{}.tmp", std::process::id()),
    };
    Ok(dir.join(file_name))
}

fn refresh_derived_from_note_files(index: &mut Index) -> Result<()> {
    let mut body_terms = BTreeMap::new();

    for note in index.notes.values() {
        let Ok(body) = fs::read_to_string(&note.path) else {
            continue;
        };
        body_terms.insert(note.id.clone(), terms_from_body(&body));
    }

    index.rebuild_derived_with_body_terms(&body_terms);
    Ok(())
}

fn active_notes_dir(index: &Index) -> Result<&Path> {
    index.active_notes_dir().ok_or(NtError::MissingNotebook)
}

fn summary_line(note: &NoteMeta) -> String {
    let day = note.created.get(0..10).unwrap_or("unknown");
    let tags = joined_or_dash(&note.tags);

    format!("{:<17}  {}  {:<12}  {}", note.id, day, tags, note.title)
}

fn joined_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
    }
}

fn matches_metadata(note: &NoteMeta, needle: &str) -> bool {
    note.id.to_ascii_lowercase().contains(needle)
        || note.title.to_ascii_lowercase().contains(needle)
        || note.kind.to_ascii_lowercase().contains(needle)
        || note
            .status
            .as_deref()
            .is_some_and(|status| status.to_ascii_lowercase().contains(needle))
        || note
            .tags
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(needle))
        || note
            .collections
            .iter()
            .any(|collection| collection.to_ascii_lowercase().contains(needle))
        || note
            .links
            .iter()
            .any(|link| link.to_ascii_lowercase().contains(needle))
        || note
            .refs
            .iter()
            .any(|reference| reference.to_ascii_lowercase().contains(needle))
}

fn matches_body(note: &NoteMeta, needle: &str) -> bool {
    let Ok(body) = fs::read_to_string(&note.path) else {
        return false;
    };

    body.to_ascii_lowercase().contains(needle)
}

fn file_updated_iso(path: &Path) -> Result<Option<String>> {
    let modified = fs::metadata(path)?.modified();
    match modified {
        Ok(time) => Ok(Some(timestamp_from_system_time(time).iso)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::fs::atomic_write;
    use crate::index::{Index, NoteMeta};

    use super::{rebuild_index_from_dir, summary_line};

    fn note(id: &str) -> NoteMeta {
        NoteMeta::new_note(
            id.to_string(),
            PathBuf::from(format!("notes/{id}.md")),
            "2026-05-28T14:30:12Z".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "Storage shape".to_string(),
        )
    }

    #[test]
    fn summary_line_is_stable() {
        let mut note = note("NT20260528T143012");
        note.tags = vec!["design".to_string()];

        assert_eq!(
            summary_line(&note),
            "NT20260528T143012  2026-05-28  design        Storage shape"
        );
    }

    #[test]
    fn summary_line_uses_dash_for_empty_tags() {
        let note = note("NT20260528T143012");

        assert_eq!(
            summary_line(&note),
            "NT20260528T143012  2026-05-28  -             Storage shape"
        );
    }

    #[test]
    fn rebuild_preserves_non_derivable_metadata_for_known_notes() {
        let dir = temp_dir("rebuild-preserves");
        let id = "NT20260528T143012";
        let path = dir.join(format!("{id}.md"));
        atomic_write(&path, b"# Recovered title\n\nbody\n").unwrap();

        let mut previous = Index::default();
        let mut old_note = note(id);
        old_note.path = path.clone();
        old_note.title = "Old title".to_string();
        old_note.kind = "decision".to_string();
        old_note.status = Some("open".to_string());
        old_note.tags = vec!["design".to_string()];
        old_note.collections = vec!["projects/nt".to_string()];
        old_note.links = vec!["NT20260527T120000".to_string()];
        old_note.refs = vec!["https://example.com/spec".to_string()];
        previous.upsert_note(old_note);

        let rebuilt = rebuild_index_from_dir(&dir, &previous).unwrap();
        let rebuilt_note = rebuilt.notes.get(id).unwrap();

        assert_eq!(rebuilt_note.title, "Recovered title");
        assert_eq!(rebuilt_note.kind, "decision");
        assert_eq!(rebuilt_note.status.as_deref(), Some("open"));
        assert_eq!(rebuilt_note.tags, vec!["design".to_string()]);
        assert_eq!(rebuilt_note.collections, vec!["projects/nt".to_string()]);
        assert_eq!(rebuilt_note.links, vec!["NT20260527T120000".to_string()]);
        assert_eq!(
            rebuilt_note.refs,
            vec!["https://example.com/spec".to_string()]
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rebuild_indexes_body_terms() {
        let dir = temp_dir("rebuild-body-terms");
        let id = "NT20260528T143012";
        let path = dir.join(format!("{id}.md"));
        atomic_write(&path, b"# Recovered title\n\nbodyonlyterm\n").unwrap();

        let rebuilt = rebuild_index_from_dir(&dir, &Index::default()).unwrap();

        assert_eq!(
            rebuilt.terms.get("bodyonlyterm").unwrap(),
            &vec![id.to_string()]
        );

        let _ = fs::remove_dir_all(dir);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nt-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
