use std::collections::BTreeMap;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use crate::cli::{Cli, Command, ConfigCommand};
use crate::completion::print_completion;
use crate::config::Config;
use crate::error::{NtError, Result};
use crate::fs::{absolute_path, atomic_write, nt_home, relative_to_cwd};
use crate::index::{Index, NoteMeta, NotebookMeta, terms_from_body};
use crate::note::{
    generate_unique_id, note_path, timestamp_from_id, timestamp_from_system_time, title_from_body,
    validate_id,
};
use crate::query::Query;

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init { notes_dir } => init(&notes_dir),
        Command::Add { metadata } => add(&metadata),
        Command::List => list(),
        Command::Find { expr } => find(&expr),
        Command::Show { id } => show(&id),
        Command::Edit { id } => edit(&id),
        Command::Discuss { id, prompt } => discuss(&id, &prompt),
        Command::Rm { id } => rm(&id),
        Command::Rebuild => rebuild(),
        Command::Ids => ids(),
        Command::Tags => tags(),
        Command::Tag { id, tag } => tag_note(&id, &tag),
        Command::Untag { id, tag } => untag_note(&id, &tag),
        Command::Collections => collections(),
        Command::Collection { name } => collection(&name),
        Command::Collect { id, collection } => collect(&id, &collection),
        Command::Uncollect { id, collection } => uncollect(&id, &collection),
        Command::Kind { id, kind } => set_kind(&id, &kind),
        Command::Status { args } => route_status(&args),
        Command::Link { from_id, to_id } => link(&from_id, &to_id),
        Command::Unlink { from_id, to_id } => unlink(&from_id, &to_id),
        Command::Links { id } => links(&id),
        Command::Backlinks { id } => backlinks(&id),
        Command::Agent { prompt } => crate::agent::run(&prompt),
        Command::Config { command } => config(command),
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
    refresh_derived_from_note_files(&mut index)?;
    index.save()?;
    crate::skills::ensure_defaults()?;

    println!("initialized {}", relative_to_cwd(&notes_dir).display());
    Ok(())
}

fn add(metadata: &[String]) -> Result<()> {
    let mut index = Index::load()?;
    let notes_dir = active_notes_dir(&index)?.to_path_buf();
    let metadata = CreationMetadata::parse(metadata, &index)?;
    let body = read_note_body_for_add()?;
    let timestamp = generate_unique_id(&notes_dir, &index)?;
    let path = note_path(&notes_dir, &timestamp.id)?;
    let mut note = NoteMeta::new_note(
        timestamp.id.clone(),
        path.clone(),
        timestamp.iso.clone(),
        timestamp.iso,
        title_from_body(&body),
    );
    metadata.apply(&mut note);

    atomic_write(&path, body.as_bytes())?;

    index.upsert_note(note);
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
    println!("sources {}", joined_or_dash(&note.sources));
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

fn discuss(id: &str, _prompt: &[String]) -> Result<()> {
    validate_id(id)?;
    let index = Index::load()?;
    if !index.notes.contains_key(id) {
        return Err(NtError::NoteNotFound(id.to_string()));
    }

    Err(NtError::Message(
        "discuss is not implemented yet".to_string(),
    ))
}

fn find(exprs: &[String]) -> Result<()> {
    let index = Index::load()?;
    let query = Query::parse(exprs)?;

    for id in &index.recent {
        let Some(note) = index.notes.get(id) else {
            continue;
        };

        if query.matches(&index, note) {
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

fn tag_note(id: &str, tag: &str) -> Result<()> {
    mutate_note(id, |note| {
        push_unique_sorted(&mut note.tags, tag.to_string());
        Ok(())
    })?;

    println!("tagged {id} {tag}");
    Ok(())
}

fn untag_note(id: &str, tag: &str) -> Result<()> {
    mutate_note(id, |note| {
        note.tags.retain(|value| value != tag);
        Ok(())
    })?;

    println!("untagged {id} {tag}");
    Ok(())
}

fn collections() -> Result<()> {
    let index = Index::load()?;
    for (collection, ids) in &index.collections {
        println!("{collection}\t{}", ids.len());
    }
    Ok(())
}

fn collection(name: &str) -> Result<()> {
    let index = Index::load()?;
    let Some(ids) = index.collections.get(name) else {
        return Ok(());
    };

    for id in ids {
        if let Some(note) = index.notes.get(id) {
            println!("{}", summary_line(note));
        }
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
            note.sources = previous.sources.clone();
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

fn collect(id: &str, collection: &str) -> Result<()> {
    mutate_note(id, |note| {
        push_unique_sorted(&mut note.collections, collection.to_string());
        Ok(())
    })?;

    println!("collected {id} {collection}");
    Ok(())
}

fn uncollect(id: &str, collection: &str) -> Result<()> {
    mutate_note(id, |note| {
        note.collections.retain(|value| value != collection);
        Ok(())
    })?;

    println!("uncollected {id} {collection}");
    Ok(())
}

fn set_kind(id: &str, kind: &str) -> Result<()> {
    mutate_note(id, |note| {
        note.kind = kind.to_string();
        Ok(())
    })?;

    println!("kind {id} {kind}");
    Ok(())
}

fn route_status(args: &[String]) -> Result<()> {
    match args {
        [] => print_status(),
        [id, status] => set_status(id, status),
        _ => Err(NtError::Message(
            "usage: nt status [<id> <status>]".to_string(),
        )),
    }
}

fn print_status() -> Result<()> {
    let index = Index::load()?;

    for wanted in ["open", "waiting"] {
        let Some(ids) = index.statuses.get(wanted) else {
            continue;
        };

        for id in ids {
            if let Some(note) = index.notes.get(id) {
                println!("{}", summary_line(note));
            }
        }
    }

    Ok(())
}

fn set_status(id: &str, status: &str) -> Result<()> {
    mutate_note(id, |note| {
        note.status = Some(status.to_string());
        Ok(())
    })?;

    println!("status {id} {status}");
    Ok(())
}

fn link(from_id: &str, to_id: &str) -> Result<()> {
    validate_id(from_id)?;
    validate_id(to_id)?;
    mutate_index(|index| {
        ensure_note_exists(index, to_id)?;
        let from = note_mut(index, from_id)?;
        push_unique_sorted(&mut from.links, to_id.to_string());
        Ok(())
    })?;

    println!("linked {from_id} {to_id}");
    Ok(())
}

fn unlink(from_id: &str, to_id: &str) -> Result<()> {
    validate_id(from_id)?;
    validate_id(to_id)?;
    mutate_index(|index| {
        let from = note_mut(index, from_id)?;
        from.links.retain(|value| value != to_id);
        Ok(())
    })?;

    println!("unlinked {from_id} {to_id}");
    Ok(())
}

fn links(id: &str) -> Result<()> {
    validate_id(id)?;
    let index = Index::load()?;
    let note = index
        .notes
        .get(id)
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;

    for link in &note.links {
        println!("{link}");
    }

    Ok(())
}

fn backlinks(id: &str) -> Result<()> {
    validate_id(id)?;
    let index = Index::load()?;
    ensure_note_exists(&index, id)?;

    if let Some(ids) = index.backlinks.get(id) {
        for backlink in ids {
            println!("{backlink}");
        }
    }

    Ok(())
}

fn config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show => config_show(),
        ConfigCommand::AgentOutput { mode } => {
            let mut config = Config::load()?;
            config.agent.output = mode;
            config.save()?;
            println!("configured agent-output {}", agent_output_name(mode));
            Ok(())
        }
    }
}

fn config_show() -> Result<()> {
    Config::load()?.print()?;

    let index = Index::load()?;
    let notes_dir = index
        .active_notes_dir()
        .map(relative_to_cwd)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    let skills = crate::skills::available_skill_paths()?;

    println!("notes_dir {notes_dir}");
    println!("agent_workspace {}", relative_to_cwd(&nt_home()?).display());
    println!(
        "agents_md {}",
        relative_to_cwd(&crate::skills::agents_md_path()?).display()
    );
    for (name, path) in skills {
        println!("skill {name} {}", relative_to_cwd(&path).display());
    }

    Ok(())
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

fn mutate_index<F>(mutate: F) -> Result<()>
where
    F: FnOnce(&mut Index) -> Result<()>,
{
    let mut index = Index::load()?;
    mutate(&mut index)?;
    refresh_derived_from_note_files(&mut index)?;
    index.save()
}

fn mutate_note<F>(id: &str, mutate: F) -> Result<()>
where
    F: FnOnce(&mut NoteMeta) -> Result<()>,
{
    validate_id(id)?;
    mutate_index(|index| mutate(note_mut(index, id)?))
}

fn note_mut<'a>(index: &'a mut Index, id: &str) -> Result<&'a mut NoteMeta> {
    index
        .notes
        .get_mut(id)
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))
}

fn ensure_note_exists(index: &Index, id: &str) -> Result<()> {
    if index.notes.contains_key(id) {
        Ok(())
    } else {
        Err(NtError::NoteNotFound(id.to_string()))
    }
}

fn push_unique_sorted(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
        values.sort();
    }
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

fn file_updated_iso(path: &Path) -> Result<Option<String>> {
    let modified = fs::metadata(path)?.modified();
    match modified {
        Ok(time) => Ok(Some(timestamp_from_system_time(time).iso)),
        Err(_) => Ok(None),
    }
}

#[derive(Debug, Default)]
struct CreationMetadata {
    kind: Option<String>,
    status: Option<String>,
    tags: Vec<String>,
    collections: Vec<String>,
    links: Vec<String>,
    sources: Vec<String>,
}

impl CreationMetadata {
    fn parse(exprs: &[String], index: &Index) -> Result<Self> {
        let mut metadata = Self::default();

        for expr in exprs {
            metadata.parse_expr(expr, index)?;
        }

        Ok(metadata)
    }

    fn parse_expr(&mut self, expr: &str, index: &Index) -> Result<()> {
        let Some((field, value)) = expr.split_once(':') else {
            return Err(NtError::Message(format!(
                "unknown add metadata `{expr}`; use tag:<tag>, kind:<kind>, status:<status>, collection:<name>, link:<id>, or source:<term>"
            )));
        };

        match field {
            "tag" => push_value_list(&mut self.tags, field, value),
            "collection" => push_value_list(&mut self.collections, field, value),
            "source" => push_single_value(&mut self.sources, field, value),
            "link" => {
                for link in split_metadata_values(field, value)? {
                    validate_id(&link)?;
                    ensure_note_exists(index, &link)?;
                    push_unique_sorted(&mut self.links, link);
                }
                Ok(())
            }
            "kind" => set_single_metadata(&mut self.kind, field, value),
            "status" => set_single_metadata(&mut self.status, field, value),
            _ => Err(NtError::Message(format!(
                "unknown add metadata field `{field}`"
            ))),
        }
    }

    fn apply(self, note: &mut NoteMeta) {
        if let Some(kind) = self.kind {
            note.kind = kind;
        }
        note.status = self.status;
        note.tags = self.tags;
        note.collections = self.collections;
        note.links = self.links;
        note.sources = self.sources;
    }
}

fn push_value_list(values: &mut Vec<String>, field: &str, raw: &str) -> Result<()> {
    for value in split_metadata_values(field, raw)? {
        push_unique_sorted(values, value);
    }
    Ok(())
}

fn push_single_value(values: &mut Vec<String>, field: &str, raw: &str) -> Result<()> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(NtError::Message(format!(
            "empty add metadata value for `{field}`"
        )));
    }

    push_unique_sorted(values, value.to_string());
    Ok(())
}

fn set_single_metadata(target: &mut Option<String>, field: &str, raw: &str) -> Result<()> {
    let values = split_metadata_values(field, raw)?;
    if values.len() != 1 {
        return Err(NtError::Message(format!(
            "`{field}` metadata accepts one value"
        )));
    }
    if target.replace(values[0].clone()).is_some() {
        return Err(NtError::Message(format!(
            "`{field}` metadata can be set only once"
        )));
    }
    Ok(())
}

fn split_metadata_values(field: &str, raw: &str) -> Result<Vec<String>> {
    let values: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();

    if values.is_empty() {
        return Err(NtError::Message(format!(
            "empty add metadata value for `{field}`"
        )));
    }

    Ok(values)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::fs::atomic_write;
    use crate::index::{Index, NoteMeta};

    use super::{CreationMetadata, rebuild_index_from_dir, summary_line};

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
        old_note.sources = vec!["https://example.com/spec".to_string()];
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
            rebuilt_note.sources,
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

    #[test]
    fn creation_metadata_accepts_repeated_and_comma_separated_values() {
        let metadata = CreationMetadata::parse(
            &[
                "tag:design,cli".to_string(),
                "tag:rust".to_string(),
                "collection:projects/nt".to_string(),
                "source:https://example.com/a,b".to_string(),
                "kind:decision".to_string(),
                "status:open".to_string(),
            ],
            &Index::default(),
        )
        .unwrap();
        let mut note = note("NT20260528T143012");

        metadata.apply(&mut note);

        assert_eq!(note.tags, vec!["cli", "design", "rust"]);
        assert_eq!(note.collections, vec!["projects/nt"]);
        assert_eq!(note.sources, vec!["https://example.com/a,b"]);
        assert_eq!(note.kind, "decision");
        assert_eq!(note.status.as_deref(), Some("open"));
    }

    #[test]
    fn creation_metadata_rejects_unknown_fields() {
        let err =
            CreationMetadata::parse(&["topic:storage".to_string()], &Index::default()).unwrap_err();

        assert_eq!(err.to_string(), "unknown add metadata field `topic`");
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nt-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
