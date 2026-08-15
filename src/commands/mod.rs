use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use crate::cli::{Cli, Command};
use crate::error::{NtError, Result};
use crate::input::Input;
use crate::repository::AddOrRemove;

mod add;
mod edit;
mod find;
mod init;
mod link;
mod list;
mod move_note;
mod rm;
mod show;
mod tag;

pub struct App<'a> {
    database_path: Option<PathBuf>,
    input: Input<'a>,
    output: &'a mut dyn Write,
    output_is_terminal: bool,
}

impl<'a> App<'a> {
    pub fn new(
        database_path: Option<PathBuf>,
        input: Input<'a>,
        output: &'a mut dyn Write,
        output_is_terminal: bool,
    ) -> Self {
        Self {
            database_path,
            input,
            output,
            output_is_terminal,
        }
    }

    fn database_path(&self) -> Result<&std::path::Path> {
        self.database_path.as_deref().ok_or(NtError::HomeNotFound)
    }
}

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
        Some(Command::Help { topic }) => crate::cli::help::print(&topic, app.output),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor, Write};

    use clap::Parser;

    use super::{App, run};
    use crate::cli::Cli;
    use crate::error::NtError;
    use crate::input::Input;
    use crate::repository::Repository;

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
        let mut repository = Repository::open_at(&database_path).unwrap();
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
    fn command_output_failures_are_returned() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join(".nt/nt.sqlite3");
        let mut stdin = Cursor::new(Vec::new());
        let mut editor = |_| panic!("editor should not run");
        let input = Input::new(&mut stdin, false, &mut editor);
        let mut output = FailingWriter;
        let mut app = App::new(Some(database_path), input, &mut output, false);

        assert!(matches!(
            run(Cli::parse_from(["nt", "init"]), &mut app),
            Err(NtError::Io(_))
        ));
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("output failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
