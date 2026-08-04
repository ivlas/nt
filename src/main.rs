use clap::Parser;

use error::{CollectionErrorKind, MetadataErrorKind, NtError};

mod cli;
mod commands;
mod display;
mod error;
mod export;
mod fs;
mod listing;
mod note;
mod query;
mod repository;
mod terminal;

fn main() {
    let cli = cli::Cli::parse();

    if let Err(err) = commands::run(cli) {
        let message = format!("error: {}", error_message(&err));
        eprintln!(
            "{}",
            terminal::paint(
                &message,
                terminal::Style::Red,
                terminal::stderr_color_enabled()
            )
        );
        std::process::exit(1);
    }
}

fn error_message(error: &NtError) -> String {
    match error {
        NtError::InvalidMetadata {
            command,
            field,
            value,
            kind,
        } => match kind {
            MetadataErrorKind::UnknownExpression => {
                format!(
                    "unknown {command} metadata `{}`",
                    value.as_deref().unwrap_or_default()
                )
            }
            MetadataErrorKind::UnknownField => format!(
                "unknown {command} metadata field `{}`",
                field.as_deref().unwrap_or_default()
            ),
            MetadataErrorKind::TodoOnly => format!(
                "`{}` metadata is only valid for `nt todo`",
                field.as_deref().unwrap_or_default()
            ),
            MetadataErrorKind::EmptyValue if *command == "update" => format!(
                "empty `{}` update value",
                field.as_deref().unwrap_or_default()
            ),
            MetadataErrorKind::EmptyValue => format!(
                "empty {command} metadata value for `{}`",
                field.as_deref().unwrap_or_default()
            ),
            MetadataErrorKind::MultipleValues => format!(
                "`{}` metadata accepts one value",
                field.as_deref().unwrap_or_default()
            ),
            MetadataErrorKind::DuplicateField => format!(
                "`{}` metadata can be set only once",
                field.as_deref().unwrap_or_default()
            ),
            MetadataErrorKind::RequiresValue => format!(
                "`{}` {command} requires a value",
                field.as_deref().unwrap_or_default()
            ),
            MetadataErrorKind::RequiresSignedValue => format!(
                "`{}` {command} requires +value or -value",
                field.as_deref().unwrap_or_default()
            ),
        },
        NtError::InvalidCollection {
            value,
            component,
            kind,
        } => match kind {
            CollectionErrorKind::MissingQualifier => {
                format!("invalid collection `{value}`; use <vault>/<collection>")
            }
            CollectionErrorKind::InvalidVault => format!(
                "invalid vault `{}`; use lowercase names without slashes, spaces, or commas",
                component.as_deref().unwrap_or_default()
            ),
            CollectionErrorKind::InvalidName => format!(
                "invalid collection `{}`; use lowercase names without slashes, spaces, or commas",
                component.as_deref().unwrap_or_default()
            ),
        },
        NtError::ConcurrentEdit { .. } => "note changed during edit; please retry".to_string(),
        NtError::ExportFailure {
            path,
            note_id,
            source,
        } => match note_id {
            Some(note_id) => format!(
                "failed to export note {note_id} to {}: {}",
                path.display(),
                error_message(source)
            ),
            None => format!(
                "failed to export to {}: {}",
                path.display(),
                error_message(source)
            ),
        },
        _ => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::error::{CollectionErrorKind, MetadataErrorKind, NtError};

    use super::error_message;

    #[test]
    fn cli_renders_semantic_errors() {
        let metadata = NtError::InvalidMetadata {
            command: "todo",
            field: Some("urgency".to_string()),
            value: None,
            kind: MetadataErrorKind::UnknownField,
        };
        assert_eq!(
            error_message(&metadata),
            "unknown todo metadata field `urgency`"
        );

        let collection = NtError::InvalidCollection {
            value: "Personal/inbox".to_string(),
            component: Some("Personal".to_string()),
            kind: CollectionErrorKind::InvalidVault,
        };
        assert_eq!(
            error_message(&collection),
            "invalid vault `Personal`; use lowercase names without slashes, spaces, or commas"
        );

        let export = NtError::ExportFailure {
            path: PathBuf::from("archive/note.md"),
            note_id: Some("note-id".to_string()),
            source: Box::new(NtError::Message("disk full".to_string())),
        };
        assert_eq!(
            error_message(&export),
            "failed to export note note-id to archive/note.md: disk full"
        );
    }
}
