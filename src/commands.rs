use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use crate::cli::{Cli, Command, ConfigCommand, LinkMode};
use crate::completion::print_completion;
use crate::display::{joined_or_dash, summary_line, summary_line_for_display};
use crate::error::{NtError, Result};
use crate::export::export_markdown;
use crate::fs::{absolute_path, atomic_write, nt_home, relative_to_cwd};
use crate::index::{Index, NoteMeta};
use crate::note::{generate_unique_id, note_path, title_from_body, validate_id};
use crate::query::Query;
use crate::terminal::{Style, paint};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init { notes_dir } => init(&notes_dir),
        Command::Add { metadata } => add(&metadata),
        Command::List => list(),
        Command::Find { expr } => find(&expr),
        Command::Show { id } => show(&id),
        Command::Edit { id } => edit(&id),
        Command::Rm { id } => rm(&id),
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
        Command::Links { id, mode } => links(&id, mode),
        Command::Export { path, ids } => export(&path, &ids),
        Command::Config { command } => config(command),
        Command::Completion { shell } => {
            print_completion(shell);
            Ok(())
        }
        Command::Help { topic } => crate::help::print(&topic),
    }
}

fn init(notes_dir: &Path) -> Result<()> {
    let notes_dir = absolute_path(notes_dir)?;
    ensure_notes_dir_is_flat(&notes_dir)?;

    let mut index = Index::load()?;
    let timestamp = crate::note::timestamp_now();
    let vault = index.create_vault_for_path(notes_dir.clone(), timestamp.iso)?;

    fs::create_dir_all(&notes_dir)?;
    fs::create_dir_all(nt_home()?)?;

    index.active_vault = Some(vault.clone());
    import_existing_notes(&mut index, &notes_dir)?;
    index.save()?;

    println!(
        "initialized {vault} {}",
        relative_to_cwd(&notes_dir).display()
    );
    Ok(())
}

fn import_existing_notes(index: &mut Index, notes_dir: &Path) -> Result<()> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(notes_dir)? {
        paths.push(entry?.path());
    }
    paths.sort();

    for path in paths {
        let id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .ok_or_else(|| {
                NtError::Message(format!("invalid note filename: {}", path.display()))
            })?;
        let created = crate::note::iso_from_id(&id)?;
        let updated = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .map(crate::note::timestamp_from_system_time)
            .map(|timestamp| timestamp.iso)
            .unwrap_or_else(|_| created.clone());
        let body = fs::read_to_string(&path)?;

        if let Some(existing) = index.notes.get(&id) {
            if existing.path != path {
                return Err(NtError::Message(format!(
                    "note id `{id}` already exists in index at {}",
                    existing.path.display()
                )));
            }
        }

        let mut note = NoteMeta::new_note(id, path, created, updated, title_from_body(&body));
        add_body_sources(&mut note, &body);
        index.upsert_note(note);
    }

    Ok(())
}

fn ensure_notes_dir_is_flat(notes_dir: &Path) -> Result<()> {
    if !notes_dir.exists() {
        return Ok(());
    }

    if !notes_dir.is_dir() {
        return Err(NtError::Message(format!(
            "notes path is not a directory: {}",
            notes_dir.display()
        )));
    }

    for entry in fs::read_dir(notes_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let stem = path.file_stem().and_then(|value| value.to_str());
        let extension = path.extension().and_then(|value| value.to_str());

        if !file_type.is_file()
            || extension != Some("md")
            || !stem.is_some_and(|value| validate_id(value).is_ok())
        {
            return Err(NtError::Message(format!(
                "notes directory must contain only NTYYYYMMDDTHHmmss.md files; invalid entry: {}",
                path.display()
            )));
        }
    }

    Ok(())
}

fn add(metadata: &[String]) -> Result<()> {
    let mut index = Index::load()?;
    let notes_dir = active_vault_path(&index)?.to_path_buf();
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
    add_body_sources(&mut note, &body);

    atomic_write(&path, body.as_bytes())?;

    index.upsert_note(note);
    if let Err(err) = index.save() {
        let _ = fs::remove_file(&path);
        return Err(err);
    }

    println!("saved {}", timestamp.id);
    Ok(())
}

fn list() -> Result<()> {
    let index = Index::load()?;
    let color = crate::terminal::stdout_color_enabled();

    for note in index.active_recent_notes() {
        println!("{}", summary_line_for_display(note, color));
    }

    Ok(())
}

fn show(id: &str) -> Result<()> {
    let text = show_text_for_display(id, crate::terminal::stdout_color_enabled())?;

    print!("{text}");
    if !text.ends_with('\n') {
        println!();
    }

    Ok(())
}

fn show_text_for_display(id: &str, color: bool) -> Result<String> {
    validate_id(id)?;
    let index = Index::load()?;
    let note = note_ref(&index, id)?;
    let body = fs::read_to_string(&note.path)?;

    let mut text = String::new();
    text.push_str(&format!(
        "{}  {}\n",
        paint(&note.id, Style::BrightCyan, color),
        note.title
    ));
    text.push_str(&format!(
        "path {}\n",
        paint(
            &relative_to_cwd(&note.path).display().to_string(),
            Style::Dim,
            color
        )
    ));
    text.push_str(&format!(
        "created {}\n",
        paint(&note.created, Style::Dim, color)
    ));
    text.push_str(&format!(
        "updated {}\n",
        paint(&note.updated, Style::Dim, color)
    ));
    text.push_str(&format!("kind {}\n", note.kind));
    text.push_str(&format!(
        "status {}\n",
        note.status.as_deref().unwrap_or("-")
    ));
    text.push_str(&format!(
        "tags {}\n",
        paint(&joined_or_dash(&note.tags), Style::Green, color)
    ));
    text.push_str(&format!(
        "collections {}\n",
        joined_or_dash(&note.collections)
    ));
    text.push_str(&format!("links {}\n", joined_or_dash(&note.links)));
    text.push_str(&format!("sources {}\n\n", joined_or_dash(&note.sources)));
    text.push_str(&body);
    if !text.ends_with('\n') {
        text.push('\n');
    }

    Ok(text)
}

fn edit(id: &str) -> Result<()> {
    validate_id(id)?;
    let mut index = Index::load()?;
    let note = note_ref(&index, id)?.clone();
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let body = fs::read_to_string(&note.path)?;
    let original_body = body.as_bytes().to_vec();
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
    let note_path = note.path.clone();
    let mut updated = note;
    updated.updated = timestamp.iso;
    updated.title = title_from_body(&body);
    add_body_sources(&mut updated, &body);

    index.upsert_note(updated);
    if let Err(err) = index.save() {
        let _ = atomic_write(&note_path, &original_body);
        return Err(err);
    }

    println!("saved {id}");
    Ok(())
}

fn find(exprs: &[String]) -> Result<()> {
    let index = Index::load()?;
    let query = Query::parse(exprs)?;

    for note in index.active_recent_notes() {
        if query.matches(note)? {
            println!("{}", summary_line(note));
        }
    }

    Ok(())
}

fn ids() -> Result<()> {
    let index = Index::load()?;
    for note in index.active_recent_notes() {
        println!("{}", note.id);
    }
    Ok(())
}

fn tags() -> Result<()> {
    let index = Index::load()?;
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for note in index.active_recent_notes() {
        for tag in &note.tags {
            *counts.entry(tag.clone()).or_default() += 1;
        }
    }

    for (tag, count) in counts {
        println!("{tag}\t{count}");
    }
    Ok(())
}

fn tag_note(id: &str, tag: &str) -> Result<()> {
    validate_tag(tag)?;
    mutate_note(id, |note| {
        push_unique_sorted(&mut note.tags, tag.to_string());
        Ok(())
    })?;

    println!("tagged {id} {tag}");
    Ok(())
}

fn untag_note(id: &str, tag: &str) -> Result<()> {
    validate_tag(tag)?;
    mutate_note(id, |note| {
        note.tags.retain(|value| value != tag);
        Ok(())
    })?;

    println!("untagged {id} {tag}");
    Ok(())
}

fn collections() -> Result<()> {
    let index = Index::load()?;
    let mut collections = BTreeSet::new();
    for note in index.active_recent_notes() {
        for collection in &note.collections {
            collections.insert(collection.clone());
        }
    }

    for collection in collections {
        println!("{collection}");
    }
    Ok(())
}

fn collection(name: &str) -> Result<()> {
    validate_collection(name)?;
    let index = Index::load()?;
    let Some(ids) = index.collections.get(name) else {
        return Ok(());
    };

    for id in ids {
        if let Some(note) = index.notes.get(id)
            && index.note_is_in_active_vault(note)
        {
            println!("{}", summary_line(note));
        }
    }

    Ok(())
}

fn rm(id: &str) -> Result<()> {
    validate_id(id)?;
    let mut index = Index::load()?;
    let note = note_ref(&index, id)?.clone();
    let body = fs::read(&note.path)?;

    fs::remove_file(&note.path)?;
    index.remove_note(id);
    if let Err(err) = index.save() {
        let _ = atomic_write(&note.path, &body);
        return Err(err);
    }

    println!("removed {id}");
    Ok(())
}

fn collect(id: &str, collection: &str) -> Result<()> {
    validate_collection(collection)?;
    mutate_note(id, |note| {
        push_unique_sorted(&mut note.collections, collection.to_string());
        Ok(())
    })?;

    println!("collected {id} {collection}");
    Ok(())
}

fn uncollect(id: &str, collection: &str) -> Result<()> {
    validate_collection(collection)?;
    mutate_note(id, |note| {
        note.collections.retain(|value| value != collection);
        Ok(())
    })?;

    println!("uncollected {id} {collection}");
    Ok(())
}

fn set_kind(id: &str, kind: &str) -> Result<()> {
    validate_kind(kind)?;
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

    for note in index.active_recent_notes() {
        if note
            .status
            .as_deref()
            .is_some_and(|status| matches!(status, "open" | "waiting"))
        {
            println!("{}", summary_line(note));
        }
    }

    Ok(())
}

fn set_status(id: &str, status: &str) -> Result<()> {
    validate_status(status)?;
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

fn links(id: &str, mode: LinkMode) -> Result<()> {
    validate_id(id)?;
    let index = Index::load()?;
    ensure_note_exists(&index, id)?;

    match mode {
        LinkMode::Out => print_out_links(&index, id, false),
        LinkMode::In => print_in_links(&index, id, false),
        LinkMode::Self_ => print_self_links(&index, id),
        LinkMode::All => print_all_links(&index, id),
    }
}

fn print_out_links(index: &Index, id: &str, with_direction: bool) -> Result<()> {
    let note = note_ref(index, id)?;

    for link in &note.links {
        if ensure_note_exists(index, link).is_err() {
            continue;
        }
        if with_direction {
            println!("out {link}");
        } else {
            println!("{link}");
        }
    }

    Ok(())
}

fn print_in_links(index: &Index, id: &str, with_direction: bool) -> Result<()> {
    if let Some(ids) = index.backlinks.get(id) {
        for backlink in ids {
            if ensure_note_exists(index, backlink).is_err() {
                continue;
            }
            if with_direction {
                println!("in {backlink}");
            } else {
                println!("{backlink}");
            }
        }
    }

    Ok(())
}

fn print_self_links(index: &Index, id: &str) -> Result<()> {
    print_out_links(index, id, true)?;
    print_in_links(index, id, true)
}

fn print_all_links(index: &Index, id: &str) -> Result<()> {
    let mut seen = BTreeSet::from([id.to_string()]);
    let mut queue = VecDeque::from([(id.to_string(), 0usize)]);

    while let Some((current, depth)) = queue.pop_front() {
        let next_depth = depth + 1;

        for (direction, next) in adjacent_links(index, &current)? {
            if !seen.insert(next.clone()) {
                continue;
            }

            println!("{next_depth} {direction} {next}");
            if index.notes.contains_key(&next) {
                queue.push_back((next, next_depth));
            }
        }
    }

    Ok(())
}

fn adjacent_links(index: &Index, id: &str) -> Result<Vec<(&'static str, String)>> {
    let note = note_ref(index, id)?;
    let mut adjacent = Vec::new();

    for link in &note.links {
        if ensure_note_exists(index, link).is_ok() {
            adjacent.push(("out", link.clone()));
        }
    }

    if let Some(ids) = index.backlinks.get(id) {
        for backlink in ids {
            if ensure_note_exists(index, backlink).is_ok() {
                adjacent.push(("in", backlink.clone()));
            }
        }
    }

    Ok(adjacent)
}

fn export(path: &Path, ids: &[String]) -> Result<()> {
    let index = Index::load()?;
    let active_vault = active_vault_path(&index)?.to_path_buf();
    let export_dir = absolute_path(path)?;

    ensure_export_dir_is_not_active_vault(&export_dir, &active_vault)?;
    fs::create_dir_all(&export_dir)?;
    let export_dir = fs::canonicalize(&export_dir)?;
    let active_vault = fs::canonicalize(&active_vault)?;
    ensure_export_dir_is_not_active_vault(&export_dir, &active_vault)?;

    for id in export_ids(&index, ids)? {
        let note = note_ref(&index, &id)?;
        let body = fs::read_to_string(&note.path)?;
        let path = export_dir.join(format!("{id}.md"));
        atomic_write(&path, export_markdown(note, &body)?.as_bytes())?;
        println!("exported {id} {}", relative_to_cwd(&path).display());
    }

    Ok(())
}

fn ensure_export_dir_is_not_active_vault(export_dir: &Path, active_vault: &Path) -> Result<()> {
    if export_dir == active_vault || export_dir.starts_with(active_vault) {
        return Err(NtError::Message(
            "export path must be outside the active notes directory".to_string(),
        ));
    }

    Ok(())
}

fn export_ids(index: &Index, ids: &[String]) -> Result<Vec<String>> {
    if ids.is_empty() {
        return Ok(index
            .recent
            .iter()
            .filter_map(|id| {
                let note = index.notes.get(id)?;
                index.note_is_in_active_vault(note).then(|| id.clone())
            })
            .collect());
    }

    let mut seen = BTreeSet::new();
    let mut export_ids = Vec::new();
    for id in ids {
        validate_id(id)?;
        note_ref(index, id)?;
        if seen.insert(id.clone()) {
            export_ids.push(id.clone());
        }
    }

    Ok(export_ids)
}

fn config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show => config_show(),
        ConfigCommand::Vault { name } => match name {
            Some(name) => config_set_vault(&name),
            None => config_list_vaults(),
        },
    }
}

fn config_show() -> Result<()> {
    let index = Index::load()?;
    let vault = index.active_vault.as_deref().unwrap_or("-");
    let vault_path = index
        .active_vault_path()
        .map(relative_to_cwd)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());

    println!("vault {vault} {vault_path}");

    Ok(())
}

fn config_list_vaults() -> Result<()> {
    let index = Index::load()?;
    for (name, vault) in &index.vaults {
        let marker = if index.active_vault.as_deref() == Some(name.as_str()) {
            "*"
        } else {
            "-"
        };
        println!("{marker} {name} {}", relative_to_cwd(&vault.path).display());
    }

    Ok(())
}

fn config_set_vault(name: &str) -> Result<()> {
    let mut index = Index::load()?;
    let Some(vault) = index.vaults.get(name) else {
        return Err(NtError::Message(format!(
            "unknown vault `{name}`; run `nt config vault`"
        )));
    };
    let path = vault.path.clone();

    index.active_vault = Some(name.to_string());
    index.save()?;

    println!(
        "configured vault {name} {}",
        relative_to_cwd(&path).display()
    );
    Ok(())
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

fn active_vault_path(index: &Index) -> Result<&Path> {
    index.active_vault_path().ok_or(NtError::MissingVault)
}

fn mutate_index<F>(mutate: F) -> Result<()>
where
    F: FnOnce(&mut Index) -> Result<()>,
{
    let mut index = Index::load()?;
    mutate(&mut index)?;
    index.rebuild_derived();
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
    let in_active_vault = {
        let note = index
            .notes
            .get(id)
            .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
        index.note_is_in_active_vault(note)
    };
    if !in_active_vault {
        return Err(NtError::NoteNotFound(id.to_string()));
    }

    index
        .notes
        .get_mut(id)
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))
}

fn note_ref<'a>(index: &'a Index, id: &str) -> Result<&'a NoteMeta> {
    let note = index
        .notes
        .get(id)
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
    if index.note_is_in_active_vault(note) {
        Ok(note)
    } else {
        Err(NtError::NoteNotFound(id.to_string()))
    }
}

fn ensure_note_exists(index: &Index, id: &str) -> Result<()> {
    note_ref(index, id).map(|_| ())
}

fn push_unique_sorted(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
        values.sort();
    }
}

fn add_body_sources(note: &mut NoteMeta, body: &str) {
    for source in crate::note::sources_from_body(body) {
        push_unique_sorted(&mut note.sources, source);
    }
}

fn validate_collection(collection: &str) -> Result<()> {
    if collection.trim().is_empty() {
        return Err(NtError::Message("empty collection name".to_string()));
    }

    if collection
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_uppercase() || ch == ',')
    {
        return Err(NtError::Message(format!(
            "invalid collection `{collection}`; use lowercase names without spaces or commas"
        )));
    }

    Ok(())
}

fn validate_tag(tag: &str) -> Result<()> {
    if tag.trim().is_empty() {
        return Err(NtError::Message("empty tag".to_string()));
    }

    if tag
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_uppercase() || ch == ',')
    {
        return Err(NtError::Message(format!(
            "invalid tag `{tag}`; use lowercase names without spaces or commas"
        )));
    }

    Ok(())
}

fn validate_kind(kind: &str) -> Result<()> {
    if matches!(
        kind,
        "note" | "todo" | "meeting" | "decision" | "source" | "research" | "project"
    ) {
        Ok(())
    } else {
        Err(NtError::Message(format!("invalid kind: {kind}")))
    }
}

fn validate_status(status: &str) -> Result<()> {
    if matches!(status, "open" | "waiting" | "done" | "dropped") {
        Ok(())
    } else {
        Err(NtError::Message(format!("invalid status: {status}")))
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
            "collection" => {
                for collection in split_metadata_values(field, value)? {
                    validate_collection(&collection)?;
                    push_unique_sorted(&mut self.collections, collection);
                }
                Ok(())
            }
            "source" => push_single_value(&mut self.sources, field, value),
            "link" => {
                for link in split_metadata_values(field, value)? {
                    validate_id(&link)?;
                    ensure_note_exists(index, &link)?;
                    push_unique_sorted(&mut self.links, link);
                }
                Ok(())
            }
            "kind" => {
                set_single_metadata(&mut self.kind, field, value)?;
                validate_kind(self.kind.as_deref().unwrap_or_default())
            }
            "status" => {
                set_single_metadata(&mut self.status, field, value)?;
                validate_status(self.status.as_deref().unwrap_or_default())
            }
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
        if field == "tag" {
            validate_tag(&value)?;
        }
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
    use std::path::PathBuf;

    use crate::index::{Index, NoteMeta};

    use super::CreationMetadata;

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
}
