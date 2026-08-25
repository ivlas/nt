use crate::cli::rendering::print_changes;
use crate::error::{NtError, Result};
use crate::note::Repository;
use crate::schema;

use super::App;

pub(super) fn changes(app: &mut App<'_>, cursor: &str) -> Result<()> {
    let revision = parse_cursor(cursor)?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    print_changes(&repository, revision, app.output, app.output_is_terminal)
}

fn parse_cursor(cursor: &str) -> Result<u64> {
    let Some(value) = cursor.strip_prefix("since:") else {
        return invalid_cursor(cursor);
    };
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return invalid_cursor(cursor);
    }
    let revision = value.parse::<u64>().map_err(|_| NtError::InvalidValue {
        field: "changes cursor",
        value: cursor.to_string(),
    })?;
    if i64::try_from(revision).is_err() {
        return invalid_cursor(cursor);
    }
    Ok(revision)
}

fn invalid_cursor<T>(cursor: &str) -> Result<T> {
    Err(NtError::InvalidValue {
        field: "changes cursor",
        value: cursor.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_cursor;

    #[test]
    fn cursor_is_an_exclusive_nonnegative_revision() {
        assert_eq!(parse_cursor("since:0").unwrap(), 0);
        assert_eq!(parse_cursor("since:42").unwrap(), 42);
        for invalid in ["0", "since:", "since:-1", "since:+1", "after:1"] {
            assert!(parse_cursor(invalid).is_err(), "{invalid}");
        }
    }
}
