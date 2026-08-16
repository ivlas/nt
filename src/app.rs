use std::io::Write;
use std::path::PathBuf;

use crate::cli::input::Input;
use crate::error::{NtError, Result};

pub struct App<'a> {
    database_path: Option<PathBuf>,
    pub(crate) input: Input<'a>,
    pub(crate) output: &'a mut dyn Write,
    pub(crate) output_is_terminal: bool,
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

    pub(crate) fn database_path(&self) -> Result<&std::path::Path> {
        self.database_path.as_deref().ok_or(NtError::HomeNotFound)
    }
}
