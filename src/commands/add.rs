use crate::error::{NtError, Result};
use crate::note::{CollectionPath, NewNote, NoteId, Repository, Tag};
use crate::schema;

use super::{App, write_commit_output};

pub(super) fn add(app: &mut App<'_>, metadata: &[String], body_arguments: &[String]) -> Result<()> {
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    let metadata = CaptureMetadata::parse(metadata)?;
    let body = app.input.read_body(body_arguments, None)?;
    let note = NewNote::new(metadata.collection, body)?
        .with_tags(metadata.tags)
        .with_links(metadata.links);
    let id = repository.create_note(note)?;
    write_commit_output(app.output, format_args!("saved {id}\n"))?;
    Ok(())
}

struct CaptureMetadata {
    collection: CollectionPath,
    tags: Vec<Tag>,
    links: Vec<NoteId>,
}

impl CaptureMetadata {
    fn parse(expressions: &[String]) -> Result<Self> {
        let mut collection = None;
        let mut tags = Vec::new();
        let mut links = Vec::new();
        for expression in expressions {
            let Some((field, value)) = expression.split_once(':') else {
                return invalid_metadata(expression);
            };
            if value.is_empty() {
                return invalid_metadata(expression);
            }
            match field {
                "collection" => {
                    if collection.is_some() || value.contains(',') {
                        return invalid_metadata(expression);
                    }
                    collection = Some(value.parse()?);
                }
                "tag" => parse_set(value, |value| value.parse(), &mut tags)?,
                "link" => parse_set(value, |value| value.parse(), &mut links)?,
                _ => return invalid_metadata(expression),
            }
        }
        Ok(Self {
            collection: collection.unwrap_or_else(CollectionPath::inbox),
            tags,
            links,
        })
    }
}

fn parse_set<T>(value: &str, parse: impl Fn(&str) -> Result<T>, output: &mut Vec<T>) -> Result<()> {
    for value in value.split(',') {
        if value.is_empty() {
            return invalid_metadata(value);
        }
        output.push(parse(value)?);
    }
    Ok(())
}

fn invalid_metadata<T>(expression: &str) -> Result<T> {
    Err(NtError::InvalidValue {
        field: "metadata",
        value: expression.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::CaptureMetadata;

    #[test]
    fn parses_capture_metadata_with_inbox_default() {
        let metadata =
            CaptureMetadata::parse(&["tag:rust,sqlite".to_string(), "tag:rust".to_string()])
                .unwrap();
        assert_eq!(metadata.collection.as_str(), "inbox");
        assert_eq!(metadata.tags.len(), 3);
    }

    #[test]
    fn rejects_unknown_duplicate_and_empty_metadata() {
        for expressions in [
            vec!["kind:note"],
            vec!["collection:a", "collection:b"],
            vec!["tag:rust,"],
        ] {
            let expressions = expressions
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
            assert!(CaptureMetadata::parse(&expressions).is_err());
        }
    }
}
