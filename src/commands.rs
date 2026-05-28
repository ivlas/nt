use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::Path;
use std::process::Command as ProcessCommand;

use crate::cli::{Cli, Command};
use crate::completion::print_completion;
use crate::error::{NtError, Result};
use crate::fs::{absolute_path, atomic_write, nt_home, relative_to_cwd};
use crate::index::{Index, NoteMeta, NotebookMeta};
use crate::note::{
    generate_unique_id, note_path, tags_from_body, timestamp_from_id, timestamp_from_system_time,
    title_from_body, validate_id,
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
    index.rebuild_derived();
    index.save()?;

    println!("initialized {}", relative_to_cwd(&notes_dir).display());
    Ok(())
}

fn add() -> Result<()> {
    let mut index = Index::load()?;
    let notes_dir = active_notes_dir(&index)?.to_path_buf();
    let body = read_note_body_for_add(&notes_dir)?;
    let timestamp = generate_unique_id(&notes_dir, &index)?;
    let path = note_path(&notes_dir, &timestamp.id)?;

    atomic_write(&path, body.as_bytes())?;

    index.upsert_note(NoteMeta {
        id: timestamp.id.clone(),
        path,
        created: timestamp.iso.clone(),
        updated: timestamp.iso,
        title: title_from_body(&body),
        tags: tags_from_body(&body),
    });
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

    let status = ProcessCommand::new(&editor).arg(&note.path).status()?;
    if !status.success() {
        return Err(NtError::EditorFailed(editor));
    }

    let body = fs::read_to_string(&note.path)?;
    if body.trim().is_empty() {
        return Err(NtError::EmptyNote);
    }

    let timestamp = crate::note::timestamp_now();
    let mut updated = note;
    updated.updated = timestamp.iso;
    updated.title = title_from_body(&body);
    updated.tags = tags_from_body(&body);

    index.upsert_note(updated);
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
    let mut rebuilt = Index {
        active_notes_dir: Some(notes_dir.clone()),
        notebooks: index.notebooks.clone(),
        ..Index::default()
    };

    for entry in fs::read_dir(&notes_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }

        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        if validate_id(id).is_err() {
            continue;
        }

        let body = fs::read_to_string(&path)?;
        let from_id = timestamp_from_id(id)?;
        let updated = file_updated_iso(&path)?.unwrap_or_else(|| from_id.iso.clone());
        let previous_tags = index
            .notes
            .get(id)
            .map(|note| note.tags.clone())
            .unwrap_or_default();
        let tags = if previous_tags.is_empty() {
            tags_from_body(&body)
        } else {
            previous_tags
        };

        rebuilt.upsert_note(NoteMeta {
            id: id.to_string(),
            path,
            created: from_id.iso,
            updated,
            title: title_from_body(&body),
            tags,
        });
    }

    rebuilt.save()?;
    println!("indexed {}", rebuilt.notes.len());
    Ok(())
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
    index.save()?;

    println!("removed {id}");
    Ok(())
}

fn read_note_body_for_add(notes_dir: &Path) -> Result<String> {
    let mut body = String::new();

    if !io::stdin().is_terminal() {
        io::stdin().read_to_string(&mut body)?;
    } else {
        body = read_from_editor(notes_dir)?;
    }

    if body.trim().is_empty() {
        return Err(NtError::EmptyNote);
    }

    if !body.ends_with('\n') {
        body.push('\n');
    }

    Ok(body)
}

fn read_from_editor(notes_dir: &Path) -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let path = notes_dir.join(format!(".nt-add-{}.md", std::process::id()));
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

fn active_notes_dir(index: &Index) -> Result<&Path> {
    index.active_notes_dir().ok_or(NtError::MissingNotebook)
}

fn summary_line(note: &NoteMeta) -> String {
    let day = note.created.get(0..10).unwrap_or("unknown");
    let tags = if note.tags.is_empty() { "-" } else { "" };
    let tags = if tags == "-" {
        tags.to_string()
    } else {
        note.tags.join(",")
    };

    format!("{:<17}  {}  {:<12}  {}", note.id, day, tags, note.title)
}

fn matches_metadata(note: &NoteMeta, needle: &str) -> bool {
    note.id.to_ascii_lowercase().contains(needle)
        || note.title.to_ascii_lowercase().contains(needle)
        || note
            .tags
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(needle))
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
    use std::path::PathBuf;

    use crate::index::NoteMeta;

    use super::summary_line;

    #[test]
    fn summary_line_is_stable() {
        let note = NoteMeta {
            id: "NT20260528T143012".to_string(),
            path: PathBuf::from("notes/NT20260528T143012.md"),
            created: "2026-05-28T14:30:12Z".to_string(),
            updated: "2026-05-28T14:30:12Z".to_string(),
            title: "Storage shape".to_string(),
            tags: vec!["design".to_string()],
        };

        assert_eq!(
            summary_line(&note),
            "NT20260528T143012  2026-05-28  design        Storage shape"
        );
    }
}
