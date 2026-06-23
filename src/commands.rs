use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use crate::cli::{AgendaView, Cli, Command, ConfigCommand, UpdateField};
use crate::completion::print_completion;
use crate::display::{agenda_line, joined_or_dash, summary_line, summary_line_for_display};
use crate::error::{NtError, Result};
use crate::export::export_markdown;
use crate::fs::{IndexMutationLock, absolute_path, atomic_write, nt_home, relative_to_cwd};
use crate::index::{Index, NoteMeta};
use crate::listing::{ListRequest, render_link_row, render_link_table, render_row, render_table};
use crate::note::{generate_unique_id, note_path, title_from_body, validate_id};
use crate::query::Query;
use crate::terminal::{Style, paint};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => crate::help::print(&[]),
        Some(Command::Init { notes_dir }) => init(&notes_dir),
        Some(Command::Add { metadata }) => add(&metadata),
        Some(Command::Rebuild) => rebuild(),
        Some(Command::List { args }) => list(&args),
        Some(Command::Find { expr }) => find(&expr),
        Some(Command::Show { id }) => show(&id),
        Some(Command::Open { id }) => open(&id),
        Some(Command::Rm { ids }) => rm(&ids),
        Some(Command::Update { id, field, value }) => update(&id, field, &value),
        Some(Command::Agenda { view }) => agenda(view),
        Some(Command::Export { path, ids }) => export(&path, &ids),
        Some(Command::Config { command }) => config(command),
        Some(Command::Completion { shell }) => {
            print_completion(shell);
            Ok(())
        }
        Some(Command::Help { topic }) => crate::help::print(&topic),
    }
}

fn init(notes_dir: &Path) -> Result<()> {
    let notes_dir = absolute_path(notes_dir)?;
    ensure_notes_dir_is_flat(&notes_dir)?;

    let _lock = IndexMutationLock::acquire()?;
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
    for path in valid_note_paths(notes_dir)? {
        let id = id_from_note_path(&path)?;
        if let Some(existing) = index.notes.get(&id)
            && existing.path != path
        {
            return Err(NtError::Message(format!(
                "note id `{id}` already exists in index at {}",
                existing.path.display()
            )));
        }

        let (note, body) = note_meta_from_markdown(index.notes.get(&id), &path)?;
        index.upsert_note_with_body(note, &body);
    }

    Ok(())
}

fn rebuild() -> Result<()> {
    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let notes_dir = active_vault_path(&index)?.to_path_buf();
    let mut rebuilt_notes = BTreeMap::new();
    let mut rebuilt_bodies = BTreeMap::new();

    for path in valid_note_paths(&notes_dir)? {
        let id = id_from_note_path(&path)?;
        let (note, body) = note_meta_from_markdown(index.notes.get(&id), &path)?;
        rebuilt_bodies.insert(id.clone(), body);
        rebuilt_notes.insert(id, note);
    }

    let count = rebuilt_notes.len();
    index.replace_active_vault_notes_with_bodies(rebuilt_notes, &rebuilt_bodies);
    index.save()?;

    println!("rebuilt {count}");
    Ok(())
}

fn valid_note_paths(notes_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(notes_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let path = entry.path();
        let stem = path.file_stem().and_then(|value| value.to_str());
        let extension = path.extension().and_then(|value| value.to_str());
        if extension == Some("md") && stem.is_some_and(|value| validate_id(value).is_ok()) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn id_from_note_path(path: &Path) -> Result<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .ok_or_else(|| NtError::Message(format!("invalid note filename: {}", path.display())))
}

fn note_meta_from_markdown(existing: Option<&NoteMeta>, path: &Path) -> Result<(NoteMeta, String)> {
    let id = id_from_note_path(path)?;
    let created = crate::note::iso_from_id(&id)?;
    let updated = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map(crate::note::timestamp_from_system_time)
        .map(|timestamp| timestamp.iso)
        .unwrap_or_else(|_| created.clone());
    let body = fs::read_to_string(path)?;

    let mut note = NoteMeta::new_note(
        id,
        path.to_path_buf(),
        created,
        updated,
        title_from_body(&body)?,
    );
    if let Some(existing) = existing {
        note.kind = existing.kind.clone();
        note.status = existing.status.clone();
        note.priority = existing.priority.clone();
        note.scheduled = existing.scheduled.clone();
        note.due = existing.due.clone();
        note.closed = existing.closed.clone();
        note.tags = existing.tags.clone();
        note.collections = existing.collections.clone();
        note.links = existing.links.clone();
        note.sources = existing.sources.clone();
    }
    add_body_sources(&mut note, &body);
    Ok((note, body))
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
            || stem.is_none_or(|value| validate_id(value).is_err())
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
    let body = read_note_body_for_add()?;
    let title = title_from_body(&body)?;
    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let notes_dir = active_vault_path(&index)?.to_path_buf();
    let metadata = CreationMetadata::parse(metadata, &index)?;
    let timestamp = generate_unique_id(&notes_dir, &index)?;
    let path = note_path(&notes_dir, &timestamp.id)?;
    let mut note = NoteMeta::new_note(
        timestamp.id.clone(),
        path.clone(),
        timestamp.iso.clone(),
        timestamp.iso.clone(),
        title,
    );
    metadata.apply(&mut note, &timestamp.iso);
    add_body_sources(&mut note, &body);

    atomic_write(&path, body.as_bytes())?;

    index.upsert_note_with_body(note, &body);
    if let Err(err) = index.save() {
        let _ = fs::remove_file(&path);
        return Err(err);
    }

    println!("saved {}", timestamp.id);
    Ok(())
}

fn list(args: &[String]) -> Result<()> {
    let index = Index::load()?;
    match ListRequest::parse(args)? {
        ListRequest::Notes { fields, query } => {
            let notes = matching_notes(&index, &query)?;

            if io::stdout().is_terminal() {
                for line in render_table(&notes, &fields) {
                    println!("{line}");
                }
            } else {
                for note in notes {
                    println!("{}", render_row(note, &fields));
                }
            }
            Ok(())
        }
        ListRequest::Tags(tag) => {
            list_metadata(&index, tag.as_deref(), validate_tag, |note| &note.tags)
        }
        ListRequest::Collections(collection) => {
            list_metadata(&index, collection.as_deref(), validate_collection, |note| {
                &note.collections
            })
        }
        ListRequest::LinkGraph { query, from, to } => {
            list_link_graph(&index, &query, from.as_deref(), to.as_deref())
        }
    }
}

fn matching_notes<'a>(index: &'a Index, query: &Query) -> Result<Vec<&'a NoteMeta>> {
    let candidates = query.candidate_ids(index);
    index
        .active_recent_notes()
        .filter(|note| candidates.as_ref().is_none_or(|ids| ids.contains(&note.id)))
        .filter_map(|note| match query.matches(index, note) {
            Ok(true) => Some(Ok(note)),
            Ok(false) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn list_link_graph(
    index: &Index,
    query: &Query,
    from_id: Option<&str>,
    to_id: Option<&str>,
) -> Result<()> {
    let links = matching_notes(index, query)?
        .into_iter()
        .filter(|note| from_id.is_none_or(|id| note.id == id))
        .flat_map(move |from| {
            from.links
                .iter()
                .filter(move |id| to_id.is_none_or(|selected| id.as_str() == selected))
                .filter_map(move |id| note_ref(index, id).ok().map(|to| (from, to)))
        })
        .collect::<Vec<_>>();

    if io::stdout().is_terminal() {
        for line in render_link_table(&links) {
            println!("{line}");
        }
    } else {
        for (from, to) in links {
            println!("{}", render_link_row(from, to));
        }
    }

    Ok(())
}

fn list_metadata<'a>(
    index: &'a Index,
    selected: Option<&str>,
    validate: impl Fn(&str) -> Result<()>,
    values: impl Fn(&'a NoteMeta) -> &'a [String],
) -> Result<()> {
    if let Some(selected) = selected {
        validate(selected)?;
        return print_note_list(
            index
                .active_recent_notes()
                .filter(|note| values(note).iter().any(|value| value == selected)),
        );
    }

    let mut available = BTreeSet::new();
    for note in index.active_recent_notes() {
        available.extend(values(note).iter().map(String::as_str));
    }
    for value in available {
        println!("{value}");
    }
    Ok(())
}

fn print_note_list<'a>(notes: impl IntoIterator<Item = &'a NoteMeta>) -> Result<()> {
    let color = crate::terminal::stdout_color_enabled();
    for note in notes {
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
        "priority {}\n",
        note.priority.as_deref().unwrap_or("-")
    ));
    text.push_str(&format!(
        "scheduled {}\n",
        note.scheduled.as_deref().unwrap_or("-")
    ));
    text.push_str(&format!("due {}\n", note.due.as_deref().unwrap_or("-")));
    text.push_str(&format!(
        "closed {}\n",
        note.closed.as_deref().unwrap_or("-")
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

fn open(id: &str) -> Result<()> {
    validate_id(id)?;
    let index = Index::load()?;
    let note = note_ref(&index, id)?.clone();
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let body = fs::read_to_string(&note.path)?;
    let original_body = body.as_bytes().to_vec();
    let open_path = open_temp_path(id)?;
    atomic_write(&open_path, body.as_bytes())?;

    let status = ProcessCommand::new(&editor).arg(&open_path).status()?;
    if !status.success() {
        let _ = fs::remove_file(&open_path);
        return Err(NtError::EditorFailed(editor));
    }

    let body = fs::read_to_string(&open_path)?;
    if body.trim().is_empty() {
        let _ = fs::remove_file(&open_path);
        return Err(NtError::EmptyNote);
    }
    let title = match title_from_body(&body) {
        Ok(title) => title,
        Err(err) => {
            let _ = fs::remove_file(&open_path);
            return Err(err);
        }
    };
    let _ = fs::remove_file(&open_path);

    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let note = note_ref(&index, id)?.clone();
    let current_body = fs::read(&note.path)?;
    if current_body != original_body {
        return Err(NtError::Message(
            "note changed during edit; please retry".to_string(),
        ));
    }

    atomic_write(&note.path, body.as_bytes())?;
    let timestamp = crate::note::timestamp_now();
    let note_path = note.path.clone();
    let mut updated = note;
    updated.updated = timestamp.iso;
    updated.title = title;
    add_body_sources(&mut updated, &body);

    index.upsert_note_with_body(updated, &body);
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
    let candidates = query.candidate_ids(&index);

    if candidates.as_ref().is_some_and(BTreeSet::is_empty) {
        return Ok(());
    }

    for note in index.active_recent_notes() {
        if !candidates.as_ref().is_none_or(|ids| ids.contains(&note.id)) {
            continue;
        }

        if query.matches(&index, note)? {
            println!("{}", summary_line(note));
        }
    }

    Ok(())
}

fn rm(ids: &[String]) -> Result<()> {
    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let mut seen = BTreeSet::new();
    let mut notes = Vec::with_capacity(ids.len());

    for id in ids {
        validate_id(id)?;
        if !seen.insert(id.as_str()) {
            return Err(NtError::Message(format!("duplicate note id: {id}")));
        }

        let note = note_ref(&index, id)?.clone();
        let body = fs::read(&note.path)?;
        notes.push((note, body));
    }

    for (position, (note, _)) in notes.iter().enumerate() {
        if let Err(err) = fs::remove_file(&note.path) {
            restore_removed_notes(&notes[..position]);
            return Err(err.into());
        }
    }

    index.remove_notes(ids.iter().map(String::as_str));
    if let Err(err) = index.save() {
        restore_removed_notes(&notes);
        return Err(err);
    }

    for id in ids {
        println!("removed {id}");
    }
    Ok(())
}

fn restore_removed_notes(notes: &[(NoteMeta, Vec<u8>)]) {
    for (note, body) in notes {
        let _ = atomic_write(&note.path, body);
    }
}

#[derive(Debug)]
enum UpdateOperation {
    Kind(Option<String>),
    Status(Option<String>),
    Priority(Option<String>),
    Scheduled(Option<String>),
    Due(Option<String>),
    Set {
        field: UpdateField,
        add: bool,
        value: String,
    },
}

impl UpdateOperation {
    fn parse(field: UpdateField, raw: &str, index: &Index) -> Result<Self> {
        match field {
            UpdateField::Kind => {
                if raw != "-" {
                    validate_kind(raw)?;
                }
                Ok(Self::Kind((raw != "-").then(|| raw.to_string())))
            }
            UpdateField::Status => {
                if raw != "-" {
                    validate_status(raw)?;
                }
                Ok(Self::Status((raw != "-").then(|| raw.to_string())))
            }
            UpdateField::Priority => {
                if raw != "-" {
                    validate_priority(raw)?;
                }
                Ok(Self::Priority((raw != "-").then(|| raw.to_string())))
            }
            UpdateField::Scheduled | UpdateField::Due => {
                if raw != "-" {
                    crate::note::validate_date(raw)?;
                }
                let value = (raw != "-").then(|| raw.to_string());
                Ok(if matches!(field, UpdateField::Scheduled) {
                    Self::Scheduled(value)
                } else {
                    Self::Due(value)
                })
            }
            UpdateField::Tag
            | UpdateField::Collection
            | UpdateField::Link
            | UpdateField::Source => {
                let (add, value) = raw
                    .strip_prefix('+')
                    .map(|value| (true, value))
                    .or_else(|| raw.strip_prefix('-').map(|value| (false, value)))
                    .ok_or_else(|| {
                        NtError::Message(format!(
                            "`{}` update requires +value or -value",
                            field_name(field)
                        ))
                    })?;
                if value.is_empty() {
                    return Err(NtError::Message(format!(
                        "empty `{}` update value",
                        field_name(field)
                    )));
                }
                match field {
                    UpdateField::Tag => validate_tag(value)?,
                    UpdateField::Collection => validate_collection(value)?,
                    UpdateField::Link => {
                        validate_id(value)?;
                        ensure_note_exists(index, value)?;
                    }
                    UpdateField::Source => {}
                    _ => unreachable!(),
                }
                Ok(Self::Set {
                    field,
                    add,
                    value: value.to_string(),
                })
            }
        }
    }

    fn apply(self, note: &mut NoteMeta, now: &str) {
        match self {
            Self::Kind(value) => note.kind = value.unwrap_or_else(|| "note".to_string()),
            Self::Status(value) => apply_status_transition(note, value, now),
            Self::Priority(value) => note.priority = value,
            Self::Scheduled(value) => note.scheduled = value,
            Self::Due(value) => note.due = value,
            Self::Set { field, add, value } => {
                let values = match field {
                    UpdateField::Tag => &mut note.tags,
                    UpdateField::Collection => &mut note.collections,
                    UpdateField::Link => &mut note.links,
                    UpdateField::Source => &mut note.sources,
                    _ => unreachable!(),
                };
                if add {
                    push_unique_sorted(values, value);
                } else {
                    values.retain(|item| item != &value);
                }
            }
        }
    }
}

fn field_name(field: UpdateField) -> &'static str {
    match field {
        UpdateField::Kind => "kind",
        UpdateField::Status => "status",
        UpdateField::Priority => "priority",
        UpdateField::Scheduled => "scheduled",
        UpdateField::Due => "due",
        UpdateField::Tag => "tag",
        UpdateField::Collection => "collection",
        UpdateField::Link => "link",
        UpdateField::Source => "source",
    }
}

fn update(id: &str, field: UpdateField, value: &str) -> Result<()> {
    validate_id(id)?;
    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    ensure_note_exists(&index, id)?;
    let operation = UpdateOperation::parse(field, value, &index)?;
    let now = crate::note::timestamp_now().iso;
    operation.apply(note_mut(&mut index, id)?, &now);
    index.rebuild_derived();
    index.save()?;
    println!("updated {id} {} {value}", field_name(field));
    Ok(())
}

fn apply_status_transition(note: &mut NoteMeta, status: Option<String>, now: &str) {
    let is_terminal = status.as_deref().is_some_and(is_terminal_status);
    if is_terminal && note.status != status {
        note.closed = Some(now.to_string());
    } else if !is_terminal {
        note.closed = None;
    }
    note.status = status;
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "done" | "dropped")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AgendaSection {
    Overdue,
    Today,
    Upcoming,
    Waiting,
    Undated,
}

fn agenda(view: Option<AgendaView>) -> Result<()> {
    let index = Index::load()?;
    let today = crate::note::local_day_now();
    let sections = select_agenda(&index, &today, view)?;
    let show_headers = view.is_none();
    for (section, notes) in sections {
        if notes.is_empty() {
            continue;
        }
        if show_headers {
            println!("{}", section_name(section));
        }
        for note in notes {
            println!("{}", agenda_line(note));
        }
    }
    Ok(())
}

fn select_agenda<'a>(
    index: &'a Index,
    today: &str,
    view: Option<AgendaView>,
) -> Result<Vec<(AgendaSection, Vec<&'a NoteMeta>)>> {
    crate::note::validate_date(today)?;
    let week_end = crate::note::add_days(today, 6)?;
    let mut sections = vec![
        (AgendaSection::Overdue, Vec::new()),
        (AgendaSection::Today, Vec::new()),
        (AgendaSection::Upcoming, Vec::new()),
        (AgendaSection::Waiting, Vec::new()),
        (AgendaSection::Undated, Vec::new()),
    ];
    for note in index.active_recent_notes() {
        if note.kind != "todo" || !matches!(note.status.as_deref(), Some("open" | "waiting")) {
            continue;
        }
        let section = agenda_section(note, today);
        let include = match view {
            None => true,
            Some(AgendaView::Today) => {
                matches!(section, AgendaSection::Overdue | AgendaSection::Today)
            }
            Some(AgendaView::Overdue) => section == AgendaSection::Overdue,
            Some(AgendaView::Waiting) => section == AgendaSection::Waiting,
            Some(AgendaView::Undated) => section == AgendaSection::Undated,
            Some(AgendaView::Week) => {
                note.status.as_deref() == Some("open")
                    && (section == AgendaSection::Overdue
                        || note
                            .scheduled
                            .as_deref()
                            .is_some_and(|day| day >= today && day <= week_end.as_str())
                        || note
                            .due
                            .as_deref()
                            .is_some_and(|day| day >= today && day <= week_end.as_str()))
            }
        };
        if include {
            sections
                .iter_mut()
                .find(|(candidate, _)| *candidate == section)
                .unwrap()
                .1
                .push(note);
        }
    }
    for (section, notes) in &mut sections {
        notes.sort_by(|left, right| {
            agenda_sort_key(left, *section).cmp(&agenda_sort_key(right, *section))
        });
    }
    Ok(sections)
}

fn agenda_section(note: &NoteMeta, today: &str) -> AgendaSection {
    if note.status.as_deref() == Some("waiting") {
        return AgendaSection::Waiting;
    }
    if note.due.as_deref().is_some_and(|due| due < today) {
        return AgendaSection::Overdue;
    }
    if note.due.as_deref() == Some(today)
        || note.scheduled.as_deref().is_some_and(|day| day <= today)
    {
        return AgendaSection::Today;
    }
    if note.due.is_some() || note.scheduled.is_some() {
        AgendaSection::Upcoming
    } else {
        AgendaSection::Undated
    }
}

fn agenda_sort_key(note: &NoteMeta, section: AgendaSection) -> (String, u8) {
    let date = match section {
        AgendaSection::Overdue => note.due.clone().unwrap_or_default(),
        AgendaSection::Today | AgendaSection::Upcoming => {
            [note.scheduled.as_ref(), note.due.as_ref()]
                .into_iter()
                .flatten()
                .min()
                .cloned()
                .unwrap_or_default()
        }
        AgendaSection::Waiting | AgendaSection::Undated => String::new(),
    };
    (date, priority_rank(note.priority.as_deref()))
}

fn priority_rank(priority: Option<&str>) -> u8 {
    match priority {
        Some("S") => 0,
        Some("A") => 1,
        Some("B") => 2,
        Some("C") => 3,
        Some("D") => 4,
        _ => 5,
    }
}

fn section_name(section: AgendaSection) -> &'static str {
    match section {
        AgendaSection::Overdue => "Overdue",
        AgendaSection::Today => "Today",
        AgendaSection::Upcoming => "Upcoming",
        AgendaSection::Waiting => "Waiting",
        AgendaSection::Undated => "Undated",
    }
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
    let _lock = IndexMutationLock::acquire()?;
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

fn open_temp_path(id: &str) -> Result<PathBuf> {
    editor_temp_path("open", Some(id))
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

fn validate_priority(priority: &str) -> Result<()> {
    if matches!(priority, "S" | "A" | "B" | "C" | "D") {
        Ok(())
    } else {
        Err(NtError::Message(format!(
            "invalid priority `{priority}`; use S, A, B, C, or D"
        )))
    }
}

#[derive(Debug, Default)]
struct CreationMetadata {
    kind: Option<String>,
    status: Option<String>,
    priority: Option<String>,
    scheduled: Option<String>,
    due: Option<String>,
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
            "priority" => {
                set_single_metadata(&mut self.priority, field, value)?;
                validate_priority(self.priority.as_deref().unwrap_or_default())
            }
            "scheduled" => {
                set_single_metadata(&mut self.scheduled, field, value)?;
                crate::note::validate_date(self.scheduled.as_deref().unwrap_or_default())
            }
            "due" => {
                set_single_metadata(&mut self.due, field, value)?;
                crate::note::validate_date(self.due.as_deref().unwrap_or_default())
            }
            _ => Err(NtError::Message(format!(
                "unknown add metadata field `{field}`"
            ))),
        }
    }

    fn apply(self, note: &mut NoteMeta, now: &str) {
        if let Some(kind) = self.kind {
            note.kind = kind;
        }
        apply_status_transition(note, self.status, now);
        note.priority = self.priority;
        note.scheduled = self.scheduled;
        note.due = self.due;
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

    use crate::cli::AgendaView;
    use crate::index::{Index, NoteMeta, VaultMeta};

    use super::{AgendaSection, CreationMetadata, apply_status_transition, select_agenda};

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

        metadata.apply(&mut note, "2026-05-28T14:30:12Z");

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

        let err = CreationMetadata::parse(&["unknown".to_string()], &Index::default()).unwrap_err();
        assert!(err.to_string().contains("unknown add metadata"));

        let err = CreationMetadata::parse(&["tag:".to_string()], &Index::default()).unwrap_err();
        assert_eq!(err.to_string(), "empty add metadata value for `tag`");

        let err = CreationMetadata::parse(
            &["kind:note".to_string(), "kind:todo".to_string()],
            &Index::default(),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "`kind` metadata can be set only once");

        let err =
            CreationMetadata::parse(&["link:NT99999999T999999".to_string()], &Index::default())
                .unwrap_err();
        assert_eq!(err.to_string(), "note not found: NT99999999T999999");
    }

    #[test]
    fn status_transitions_manage_closed_deterministically() {
        let mut note = note("NT20260528T143012");
        apply_status_transition(&mut note, Some("done".to_string()), "2026-05-28T15:00:00Z");
        assert_eq!(note.closed.as_deref(), Some("2026-05-28T15:00:00Z"));

        apply_status_transition(&mut note, Some("done".to_string()), "2026-05-29T15:00:00Z");
        assert_eq!(note.closed.as_deref(), Some("2026-05-28T15:00:00Z"));
        apply_status_transition(
            &mut note,
            Some("dropped".to_string()),
            "2026-05-30T15:00:00Z",
        );
        assert_eq!(note.closed.as_deref(), Some("2026-05-30T15:00:00Z"));

        apply_status_transition(
            &mut note,
            Some("dropped".to_string()),
            "2026-05-31T15:00:00Z",
        );
        assert_eq!(note.closed.as_deref(), Some("2026-05-30T15:00:00Z"));

        apply_status_transition(&mut note, Some("open".to_string()), "2026-06-01T15:00:00Z");
        assert_eq!(note.closed, None);
    }

    fn active_index(notes: Vec<NoteMeta>) -> Index {
        let mut index = Index::default();
        index.active_vault = Some("notes".to_string());
        index.vaults.insert(
            "notes".to_string(),
            VaultMeta {
                path: PathBuf::from("notes"),
                created: "2026-05-01T00:00:00Z".to_string(),
            },
        );
        for note in notes {
            index.upsert_note(note);
        }
        index
    }

    fn todo(
        id: &str,
        status: &str,
        priority: Option<&str>,
        scheduled: Option<&str>,
        due: Option<&str>,
    ) -> NoteMeta {
        let mut note = note(id);
        note.created = crate::note::iso_from_id(id).unwrap();
        note.kind = "todo".to_string();
        note.status = Some(status.to_string());
        note.priority = priority.map(str::to_string);
        note.scheduled = scheduled.map(str::to_string);
        note.due = due.map(str::to_string);
        note
    }

    #[test]
    fn agenda_partitions_once_and_orders_dates_priorities_and_recency() {
        let notes = vec![
            todo(
                "NT20260501T000001",
                "open",
                Some("D"),
                None,
                Some("2026-05-27"),
            ),
            todo(
                "NT20260502T000001",
                "open",
                Some("S"),
                None,
                Some("2026-05-27"),
            ),
            todo(
                "NT20260503T000001",
                "open",
                Some("A"),
                Some("2026-05-28"),
                Some("2026-06-02"),
            ),
            todo("NT20260504T000001", "open", None, None, Some("2026-06-01")),
            todo(
                "NT20260505T000001",
                "waiting",
                Some("B"),
                Some("2026-05-20"),
                Some("2026-05-21"),
            ),
            todo("NT20260506T000001", "open", Some("C"), None, None),
            todo(
                "NT20260507T000001",
                "done",
                Some("S"),
                None,
                Some("2026-05-20"),
            ),
        ];
        let index = active_index(notes);
        let sections = select_agenda(&index, "2026-05-28", None).unwrap();

        assert_eq!(sections[0].0, AgendaSection::Overdue);
        assert_eq!(
            sections[0]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260502T000001", "NT20260501T000001"]
        );
        assert_eq!(
            sections[1]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260503T000001"]
        );
        assert_eq!(
            sections[2]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260504T000001"]
        );
        assert_eq!(
            sections[3]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260505T000001"]
        );
        assert_eq!(
            sections[4]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260506T000001"]
        );
        assert_eq!(
            sections.iter().map(|(_, notes)| notes.len()).sum::<usize>(),
            6
        );

        let week = select_agenda(&index, "2026-05-28", Some(AgendaView::Week)).unwrap();
        assert_eq!(week.iter().map(|(_, notes)| notes.len()).sum::<usize>(), 4);

        let today = select_agenda(&index, "2026-05-28", Some(AgendaView::Today)).unwrap();
        assert_eq!(
            today
                .iter()
                .flat_map(|(_, notes)| notes.iter().map(|note| note.id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                "NT20260502T000001",
                "NT20260501T000001",
                "NT20260503T000001"
            ]
        );
    }

    #[test]
    fn agenda_orders_all_priorities_and_preserves_active_recency_for_ties() {
        let priorities = [None, Some("D"), Some("C"), Some("B"), Some("A"), Some("S")];
        let notes = priorities
            .into_iter()
            .enumerate()
            .map(|(index, priority)| {
                todo(
                    &format!("NT202605{:02}T000001", index + 10),
                    "open",
                    priority,
                    None,
                    None,
                )
            })
            .collect();
        let index = active_index(notes);
        let sections = select_agenda(&index, "2026-05-28", Some(AgendaView::Undated)).unwrap();
        let priorities: Vec<Option<&str>> = sections[4]
            .1
            .iter()
            .map(|note| note.priority.as_deref())
            .collect();
        assert_eq!(
            priorities,
            vec![Some("S"), Some("A"), Some("B"), Some("C"), Some("D"), None]
        );

        let mut newer = todo("NT20260520T000001", "open", Some("A"), None, None);
        newer.created = "2026-06-02T00:00:00Z".to_string();
        let mut older = todo("NT20260521T000001", "open", Some("A"), None, None);
        older.created = "2026-06-01T00:00:00Z".to_string();
        let index = active_index(vec![older, newer]);
        let sections = select_agenda(&index, "2026-05-28", Some(AgendaView::Undated)).unwrap();
        assert_eq!(
            sections[4]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260520T000001", "NT20260521T000001"]
        );
    }

    #[test]
    fn agenda_excludes_non_actionable_notes_and_handles_empty_results() {
        let done = todo("NT20260520T000001", "done", Some("S"), None, None);
        let dropped = todo("NT20260521T000001", "dropped", Some("A"), None, None);
        let mut statusless = todo("NT20260522T000001", "open", Some("B"), None, None);
        statusless.status = None;
        let mut non_todo = todo("NT20260523T000001", "open", Some("C"), None, None);
        non_todo.kind = "note".to_string();
        let index = active_index(vec![done, dropped, statusless, non_todo]);

        for view in [
            None,
            Some(AgendaView::Today),
            Some(AgendaView::Week),
            Some(AgendaView::Overdue),
            Some(AgendaView::Waiting),
            Some(AgendaView::Undated),
        ] {
            let sections = select_agenda(&index, "2026-05-28", view).unwrap();
            assert!(sections.iter().all(|(_, notes)| notes.is_empty()));
        }
    }
}
