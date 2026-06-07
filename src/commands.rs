use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use crate::cli::{Cli, Command, ConfigCommand, LinkMode};
use crate::completion::print_completion;
use crate::config::Config;
use crate::error::{NtError, Result};
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
        Command::Discuss { id, prompt } => discuss(&id, &prompt),
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
        Command::Agent { prompt } => crate::agent::run(&prompt),
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

    let mut index = Index::load()?;
    let timestamp = crate::note::timestamp_now();
    let vault = index.create_vault_for_path(notes_dir.clone(), timestamp.iso)?;

    fs::create_dir_all(&notes_dir)?;
    fs::create_dir_all(nt_home()?)?;

    index.active_vault = Some(vault.clone());
    index.save()?;
    crate::skills::ensure_defaults()?;

    println!(
        "initialized {vault} {}",
        relative_to_cwd(&notes_dir).display()
    );
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

    atomic_write(&path, body.as_bytes())?;

    index.upsert_note(note);
    index.save()?;

    println!("saved {}", timestamp.id);
    Ok(())
}

fn list() -> Result<()> {
    let index = Index::load()?;
    let color = crate::terminal::stdout_color_enabled();

    for id in &index.recent {
        let Some(note) = index.notes.get(id) else {
            continue;
        };
        if !index.note_is_in_active_vault(note) {
            continue;
        }
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

fn show_text(id: &str) -> Result<String> {
    show_text_for_display(id, false)
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
    index.save()?;

    println!("saved {id}");
    Ok(())
}

fn discuss(id: &str, prompt: &[String]) -> Result<()> {
    let note_context = show_text(id)?;

    crate::agent::discuss(id, &note_context, prompt)
}

fn find(exprs: &[String]) -> Result<()> {
    let index = Index::load()?;
    let query = Query::parse(exprs)?;

    for id in &index.recent {
        let Some(note) = index.notes.get(id) else {
            continue;
        };
        if !index.note_is_in_active_vault(note) {
            continue;
        }

        if query.matches(&index, note) {
            println!("{}", summary_line(note));
        }
    }

    Ok(())
}

fn ids() -> Result<()> {
    let index = Index::load()?;
    for id in &index.recent {
        if let Some(note) = index.notes.get(id)
            && index.note_is_in_active_vault(note)
        {
            println!("{id}");
        }
    }
    Ok(())
}

fn tags() -> Result<()> {
    let index = Index::load()?;
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for id in &index.recent {
        let Some(note) = index.notes.get(id) else {
            continue;
        };
        if !index.note_is_in_active_vault(note) {
            continue;
        }
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
    let mut collections = BTreeSet::new();
    for id in &index.recent {
        let Some(note) = index.notes.get(id) else {
            continue;
        };
        if !index.note_is_in_active_vault(note) {
            continue;
        }
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

    fs::remove_file(&note.path)?;
    index.remove_note(id);
    index.save()?;

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

    for id in &index.recent {
        let Some(note) = index.notes.get(id) else {
            continue;
        };

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

fn export_markdown(note: &NoteMeta, body: &str) -> Result<String> {
    let mut text = String::new();
    text.push_str("---\n");
    text.push_str(&format!("id: {}\n", json_value(&note.id)?));
    text.push_str(&format!(
        "path: {}\n",
        json_value(&note.path.to_string_lossy())?
    ));
    text.push_str(&format!("created: {}\n", json_value(&note.created)?));
    text.push_str(&format!("updated: {}\n", json_value(&note.updated)?));
    text.push_str(&format!("title: {}\n", json_value(&note.title)?));
    text.push_str(&format!("kind: {}\n", json_value(&note.kind)?));
    text.push_str("status: ");
    match &note.status {
        Some(status) => text.push_str(&json_value(status)?),
        None => text.push_str("null"),
    }
    text.push('\n');
    text.push_str(&format!("tags: {}\n", json_list(&note.tags)?));
    text.push_str(&format!("collections: {}\n", json_list(&note.collections)?));
    text.push_str(&format!("links: {}\n", json_list(&note.links)?));
    text.push_str(&format!("sources: {}\n", json_list(&note.sources)?));
    text.push_str("---\n\n");
    text.push_str(body);

    Ok(text)
}

fn json_value(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn json_list(values: &[String]) -> Result<String> {
    Ok(serde_json::to_string(values)?)
}

fn config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show => config_show(),
        ConfigCommand::Vault { name } => match name {
            Some(name) => config_set_vault(&name),
            None => config_list_vaults(),
        },
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
    let config = Config::load()?;
    config.print()?;

    let index = Index::load()?;
    let vault = index.active_vault.as_deref().unwrap_or("-");
    let vault_path = index
        .active_vault_path()
        .map(relative_to_cwd)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    let skills = crate::skills::available_skill_paths()?;

    println!("vault {vault} {vault_path}");
    println!("agent_workspace {}", relative_to_cwd(&nt_home()?).display());
    println!(
        "skills_dir {}",
        relative_to_cwd(&crate::skills::skills_dir()?).display()
    );
    println!(
        "agents_md {}",
        relative_to_cwd(&crate::skills::agents_md_path()?).display()
    );
    println!("agent_output {}", agent_output_name(config.agent.output));
    for (name, path) in skills {
        println!("skill {name} {}", relative_to_cwd(&path).display());
    }

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

fn validate_collection(collection: &str) -> Result<()> {
    if collection.trim().is_empty() {
        return Err(NtError::Message("empty collection name".to_string()));
    }

    if collection
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_uppercase())
    {
        return Err(NtError::Message(format!(
            "invalid collection `{collection}`; use lowercase names"
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

fn summary_line(note: &NoteMeta) -> String {
    summary_line_for_display(note, false)
}

fn summary_line_for_display(note: &NoteMeta, color: bool) -> String {
    let day = note.created.get(0..10).unwrap_or("unknown");
    let tags = joined_or_dash(&note.tags);
    let padded_tags = format!("{tags:<12}");

    format!(
        "{}  {}  {}  {}",
        paint(&format!("{:<17}", note.id), Style::BrightCyan, color),
        paint(day, Style::Dim, color),
        paint(&padded_tags, Style::Green, color),
        note.title
    )
}

fn joined_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
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

    use super::{CreationMetadata, export_markdown, summary_line, summary_line_for_display};

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
    fn summary_line_colors_human_display_when_enabled() {
        let mut note = note("NT20260528T143012");
        note.tags = vec!["design".to_string()];

        let line = summary_line_for_display(&note, true);

        assert!(line.contains("\x1b[96mNT20260528T143012\x1b[0m"));
        assert!(line.contains("\x1b[2m2026-05-28\x1b[0m"));
        assert!(line.contains("\x1b[32mdesign"));
        assert!(line.ends_with("Storage shape"));
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

    #[test]
    fn export_markdown_adds_front_matter_from_note_metadata() {
        let mut note = note("NT20260528T143012");
        note.path = PathBuf::from("/tmp/notes/NT20260528T143012.md");
        note.title = "Storage: \"shape\"".to_string();
        note.kind = "decision".to_string();
        note.status = Some("open".to_string());
        note.tags = vec!["cli".to_string(), "storage".to_string()];
        note.collections = vec!["projects/nt".to_string()];
        note.links = vec!["NT20260527T120000".to_string()];
        note.sources = vec!["https://example.com/a,b".to_string()];

        let exported = export_markdown(&note, "# Storage\n\nBody.\n").unwrap();

        assert_eq!(
            exported,
            "---\n\
id: \"NT20260528T143012\"\n\
path: \"/tmp/notes/NT20260528T143012.md\"\n\
created: \"2026-05-28T14:30:12Z\"\n\
updated: \"2026-05-28T14:30:12Z\"\n\
title: \"Storage: \\\"shape\\\"\"\n\
kind: \"decision\"\n\
status: \"open\"\n\
tags: [\"cli\",\"storage\"]\n\
collections: [\"projects/nt\"]\n\
links: [\"NT20260527T120000\"]\n\
sources: [\"https://example.com/a,b\"]\n\
---\n\n\
# Storage\n\n\
Body.\n"
        );
    }

    #[test]
    fn export_markdown_uses_null_status_and_empty_lists() {
        let note = note("NT20260528T143012");

        let exported = export_markdown(&note, "# Storage\n").unwrap();

        assert!(exported.contains("status: null\n"));
        assert!(exported.contains("tags: []\n"));
        assert!(exported.contains("collections: []\n"));
        assert!(exported.ends_with("# Storage\n"));
    }
}
