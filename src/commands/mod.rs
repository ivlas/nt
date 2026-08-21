use std::fmt;
use std::io::Write;
use std::str::FromStr;

pub(crate) use crate::app::App;
use crate::cli::{Cli, Command, LibraryCommand};
use crate::domains::note::AddOrRemove;
use crate::error::{NtError, Result};

mod add;
mod edit;
mod find;
mod init;
mod library;
mod link;
mod list;
mod move_note;
mod reference;
mod rm;
mod show;
mod tag;

fn parse_add_or_remove<T>(value: &str, field: &'static str) -> Result<AddOrRemove<T>>
where
    T: FromStr<Err = NtError>,
{
    if let Some(value) = value.strip_prefix('+') {
        return Ok(AddOrRemove::Add(value.parse()?));
    }
    if let Some(value) = value.strip_prefix('-') {
        return Ok(AddOrRemove::Remove(value.parse()?));
    }
    Err(NtError::InvalidValue {
        field,
        value: value.to_string(),
    })
}

fn write_commit_output(output: &mut dyn Write, arguments: fmt::Arguments<'_>) -> Result<()> {
    output
        .write_fmt(arguments)
        .and_then(|()| output.flush())
        .map_err(NtError::CommittedButOutputFailed)
}

pub fn run(cli: Cli, app: &mut App<'_>) -> Result<()> {
    match cli.command {
        None => crate::cli::help::print(&[], app.output),
        Some(Command::Init) => init::init(app),
        Some(Command::Add { metadata, body }) => add::add(app, &metadata, &body),
        Some(Command::Show { id }) => show::show(app, &id),
        Some(Command::List { filters }) => list::list(app, &filters),
        Some(Command::Find { expressions }) => find::find(app, &expressions),
        Some(Command::Rm { ids }) => rm::rm(app, &ids),
        Some(Command::Edit { id, body }) => edit::edit(app, &id, &body),
        Some(Command::Move { id, collection }) => move_note::move_note(app, &id, &collection),
        Some(Command::Tag { id, operation }) => tag::tag(app, &id, &operation),
        Some(Command::Link { id, operation }) => link::link(app, &id, &operation),
        Some(Command::Library { command }) => match command {
            LibraryCommand::Add { source, title } => library::add(app, &source, &title),
            LibraryCommand::Capture { id } => library::capture(app, &id),
            LibraryCommand::Show { id } => library::show(app, &id),
            LibraryCommand::Find { expressions } => library::find(app, &expressions),
            LibraryCommand::Summary { id } => library::summary(app, &id),
            LibraryCommand::History { id } => library::history(app, &id),
        },
        Some(Command::Reference {
            note_id,
            library_id,
        }) => reference::reference(app, &note_id, &library_id),
        Some(Command::Unref {
            note_id,
            library_id,
        }) => reference::unreference(app, &note_id, &library_id),
        Some(Command::Help { topic }) => crate::cli::help::print(&topic, app.output),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor, Write};
    use std::path::Path;

    use clap::Parser;

    use super::{App, run};
    use crate::cli::Cli;
    use crate::cli::input::Input;
    use crate::domains::note::{CollectionPath, NewNote, NoteQuery, Repository, timestamp_now};
    use crate::error::NtError;

    #[test]
    fn commands_run_directly_with_supplied_storage_and_io() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join(".nt/nt.sqlite3");
        let mut stdin = Cursor::new("# Direct input\nBody");
        let mut editor = |_| panic!("editor should not run");
        let input = Input::new(&mut stdin, false, &mut editor);
        let mut output = Vec::new();
        let mut app = App::new(Some(database_path.clone()), input, &mut output, false);

        run(Cli::parse_from(["nt", "init"]), &mut app).unwrap();
        run(Cli::parse_from(["nt", "add"]), &mut app).unwrap();
        drop(app);
        let saved = String::from_utf8(output.clone()).unwrap();
        let id = saved
            .lines()
            .find_map(|line| line.strip_prefix("saved "))
            .unwrap()
            .to_string();
        let mut stdin = Cursor::new(Vec::new());
        let mut editor = |_| panic!("editor should not run");
        let input = Input::new(&mut stdin, false, &mut editor);
        let mut show_app = App::new(Some(database_path.clone()), input, &mut output, false);
        run(Cli::parse_from(["nt", "show", &id]), &mut show_app).unwrap();
        drop(show_app);

        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("initialized\nsaved {id}\n# Direct input\nBody")
        );
        let repository = Repository::open_at(&database_path).unwrap();
        assert_eq!(
            repository.get_note(&id.parse().unwrap()).unwrap().body(),
            "# Direct input\nBody"
        );
    }

    #[test]
    fn help_does_not_require_a_storage_path() {
        let mut stdin = Cursor::new(Vec::new());
        let mut editor = |_| panic!("editor should not run");
        let input = Input::new(&mut stdin, false, &mut editor);
        let mut output = Vec::new();
        let mut app = App::new(None, input, &mut output, false);

        run(Cli::parse_from(["nt", "help"]), &mut app).unwrap();
        drop(app);

        assert!(String::from_utf8(output).unwrap().starts_with("nt\n"));
    }

    #[test]
    fn edit_reports_a_body_change_committed_while_editor_is_open() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join(".nt/nt.sqlite3");
        Repository::initialize_at(&database_path).unwrap();
        let id = Repository::open_at(&database_path)
            .unwrap()
            .create_note(NewNote::new(CollectionPath::inbox(), "# Original").unwrap())
            .unwrap();

        let concurrent_path = database_path.clone();
        let concurrent_id = id.clone();
        let mut editor = move |_| -> crate::error::Result<String> {
            let mut repository = Repository::open_at(&concurrent_path)?;
            let mut note = repository.get_note(&concurrent_id)?;
            let expected_version = note.body_version();
            note.replace_body("# Concurrent", timestamp_now()?)?;
            repository.replace_body(&note, expected_version)?;
            Ok("# User edit".to_string())
        };
        let mut stdin = Cursor::new(Vec::new());
        let input = Input::new(&mut stdin, true, &mut editor);
        let mut output = Vec::new();
        let mut app = App::new(Some(database_path.clone()), input, &mut output, false);
        let id_text = id.to_string();

        let result = run(Cli::parse_from(["nt", "edit", id_text.as_str()]), &mut app);
        drop(app);

        assert!(matches!(result, Err(NtError::ConcurrentEdit(changed)) if changed == id_text));
        assert!(output.is_empty());
        assert_eq!(
            Repository::open_at(&database_path)
                .unwrap()
                .get_note(&id)
                .unwrap()
                .body(),
            "# Concurrent"
        );
    }

    #[test]
    fn mutations_remain_committed_when_success_output_fails() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join(".nt/nt.sqlite3");

        assert_committed_output_failure(&database_path, &["init"], "");
        let repository = Repository::open_at(&database_path).unwrap();
        drop(repository);

        assert_committed_output_failure(&database_path, &["add"], "# Added");
        let mut repository = Repository::open_at(&database_path).unwrap();
        let mut summaries = Vec::new();
        repository
            .visit_note_summaries(&NoteQuery::default(), |summary| {
                summaries.push(summary);
                Ok(())
            })
            .unwrap();
        assert_eq!(summaries.len(), 1);
        let id = summaries[0].id().clone();
        assert_eq!(repository.get_note(&id).unwrap().body(), "# Added");
        let target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
            .unwrap();
        drop(repository);

        assert_committed_output_failure(&database_path, &["edit", &id.to_string()], "# Edited");
        let repository = Repository::open_at(&database_path).unwrap();
        assert_eq!(repository.get_note(&id).unwrap().body(), "# Edited");
        drop(repository);

        assert_committed_output_failure(&database_path, &["move", &id.to_string(), "work/nt"], "");
        let repository = Repository::open_at(&database_path).unwrap();
        let moved = NoteQuery::parse_list(&["collection:work/nt".to_string()]).unwrap();
        let mut moved_ids = Vec::new();
        repository
            .visit_note_summaries(&moved, |summary| {
                moved_ids.push(summary.id().clone());
                Ok(())
            })
            .unwrap();
        assert_eq!(moved_ids.as_slice(), std::slice::from_ref(&id));
        drop(repository);

        assert_committed_output_failure(&database_path, &["tag", &id.to_string(), "+rust"], "");
        let repository = Repository::open_at(&database_path).unwrap();
        let tagged = NoteQuery::parse_list(&["tag:rust".to_string()]).unwrap();
        let mut tagged_ids = Vec::new();
        repository
            .visit_note_summaries(&tagged, |summary| {
                tagged_ids.push(summary.id().clone());
                Ok(())
            })
            .unwrap();
        assert_eq!(tagged_ids.as_slice(), std::slice::from_ref(&id));
        drop(repository);

        assert_committed_output_failure(
            &database_path,
            &["link", &id.to_string(), &format!("+{target}")],
            "",
        );
        let repository = Repository::open_at(&database_path).unwrap();
        let mut outgoing = None;
        repository
            .visit_note_summaries(&NoteQuery::default(), |summary| {
                if summary.id() == &id {
                    outgoing = Some(summary.outgoing());
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(outgoing, Some(1));
        drop(repository);

        assert_committed_output_failure(
            &database_path,
            &["library", "add", "https://example.com", "Example"],
            "external evidence",
        );
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        let library_id: String = connection
            .query_row(
                "SELECT id FROM library_items WHERE source = 'https://example.com'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(connection);

        assert_committed_output_failure(
            &database_path,
            &["library", "capture", &library_id],
            "changed evidence",
        );
        assert_committed_output_failure(
            &database_path,
            &["library", "summary", &library_id],
            "capture summary",
        );
        assert_committed_output_failure(
            &database_path,
            &["ref", &target.to_string(), &library_id],
            "",
        );
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        let state: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM library_captures),
                     (SELECT COUNT(*) FROM library_summaries),
                     (SELECT COUNT(*) FROM note_library_refs)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(state, (2, 1, 1));
        drop(connection);

        assert_committed_output_failure(
            &database_path,
            &["unref", &target.to_string(), &library_id],
            "",
        );
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        let references: i64 = connection
            .query_row("SELECT COUNT(*) FROM note_library_refs", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(references, 0);
        drop(connection);

        assert_committed_output_failure(&database_path, &["rm", &id.to_string()], "");
        let repository = Repository::open_at(&database_path).unwrap();
        assert!(matches!(
            repository.get_note(&id),
            Err(NtError::NoteNotFound(_))
        ));
        assert!(repository.get_note(&target).is_ok());
    }

    #[test]
    fn mutations_report_success_output_flush_failures() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join(".nt/nt.sqlite3");
        let mut stdin = Cursor::new(Vec::new());
        let mut editor = |_| panic!("editor should not run");
        let input = Input::new(&mut stdin, false, &mut editor);
        let mut output = FlushFailingWriter;
        let mut app = App::new(Some(database_path.clone()), input, &mut output, false);

        assert!(matches!(
            run(Cli::parse_from(["nt", "init"]), &mut app),
            Err(NtError::CommittedButOutputFailed(_))
        ));
        assert!(Repository::open_at(&database_path).is_ok());
    }

    fn assert_committed_output_failure(path: &Path, arguments: &[&str], body: &str) {
        let mut stdin = Cursor::new(body.as_bytes());
        let mut editor = |_| panic!("editor should not run");
        let input = Input::new(&mut stdin, false, &mut editor);
        let mut output = FailingWriter;
        let mut app = App::new(Some(path.to_path_buf()), input, &mut output, false);
        let cli = Cli::parse_from(std::iter::once("nt").chain(arguments.iter().copied()));

        assert!(matches!(
            run(cli, &mut app),
            Err(NtError::CommittedButOutputFailed(_))
        ));
    }

    struct FailingWriter;

    struct FlushFailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("output failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Write for FlushFailingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("flush failed"))
        }
    }
}
