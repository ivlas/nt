use crate::error::Result;
use crate::repository::NoteMeta;

pub(crate) fn export_markdown(note: &NoteMeta, body: &str) -> Result<String> {
    let mut text = String::new();
    text.push_str("---\n");
    text.push_str(&format!("id: {}\n", json_value(note.id.as_str())?));
    text.push_str(&format!("home: {}\n", json_value(&note.home_collection)?));
    text.push_str(&format!("created: {}\n", json_value(&note.created)?));
    text.push_str(&format!("updated: {}\n", json_value(&note.updated)?));
    text.push_str(&format!("title: {}\n", json_value(&note.title)?));
    text.push_str(&format!("kind: {}\n", json_value(note.kind.as_str())?));
    text.push_str("status: ");
    match &note.status {
        Some(status) => text.push_str(&json_value(status.as_str())?),
        None => text.push_str("null"),
    }
    text.push('\n');
    optional_value(
        &mut text,
        "priority",
        note.priority.map(|value| value.as_str()),
    )?;
    optional_value(&mut text, "scheduled", note.scheduled.as_deref())?;
    optional_value(&mut text, "due", note.due.as_deref())?;
    optional_value(&mut text, "closed", note.closed.as_deref())?;
    text.push_str(&format!("tags: {}\n", json_list(&note.tags)?));
    text.push_str(&format!("collections: {}\n", json_list(&note.collections)?));
    text.push_str(&format!("links: {}\n", json_list(&note.links)?));
    text.push_str(&format!("sources: {}\n", json_list(&note.sources)?));
    text.push_str("---\n\n");
    text.push_str(body);

    Ok(text)
}

fn optional_value(text: &mut String, field: &str, value: Option<&str>) -> Result<()> {
    text.push_str(field);
    text.push_str(": ");
    match value {
        Some(value) => text.push_str(&json_value(value)?),
        None => text.push_str("null"),
    }
    text.push('\n');
    Ok(())
}

fn json_value(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn json_list<T: AsRef<str>>(values: &[T]) -> Result<String> {
    let values: Vec<_> = values.iter().map(AsRef::as_ref).collect();
    Ok(serde_json::to_string(&values)?)
}

#[cfg(test)]
mod tests {
    use crate::repository::NoteMeta;

    use super::export_markdown;

    fn note(id: &str) -> NoteMeta {
        NoteMeta::new_note(
            id.parse().unwrap(),
            "personal/inbox".to_string(),
            "# Storage shape\n".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "Storage shape".to_string(),
        )
    }

    #[test]
    fn export_markdown_adds_front_matter_from_note_metadata() {
        let mut note = note("018fbe0a-6c00-7000-8000-000000000001");
        note.title = "Storage: \"shape\"".to_string();
        note.kind = crate::note::NoteKind::Todo;
        note.status = Some(crate::note::Status::Open);
        note.tags = vec!["cli".to_string(), "storage".to_string()];
        note.collections = vec!["personal/inbox".to_string(), "work/project_a".to_string()];
        note.links = vec!["018fbe0a-6c00-7000-8000-000000000002".parse().unwrap()];
        note.sources = vec!["https://example.com/a,b".to_string()];

        let exported = export_markdown(&note, "# Storage\n\nBody.\n").unwrap();

        assert_eq!(
            exported,
            "---\n\
id: \"018fbe0a-6c00-7000-8000-000000000001\"\n\
home: \"personal/inbox\"\n\
created: \"2026-05-28T14:30:12Z\"\n\
updated: \"2026-05-28T14:30:12Z\"\n\
title: \"Storage: \\\"shape\\\"\"\n\
kind: \"todo\"\n\
status: \"open\"\n\
priority: null\n\
scheduled: null\n\
due: null\n\
closed: null\n\
tags: [\"cli\",\"storage\"]\n\
collections: [\"personal/inbox\",\"work/project_a\"]\n\
links: [\"018fbe0a-6c00-7000-8000-000000000002\"]\n\
sources: [\"https://example.com/a,b\"]\n\
---\n\n\
# Storage\n\n\
Body.\n"
        );
    }

    #[test]
    fn export_markdown_uses_null_status_and_empty_lists() {
        let note = note("018fbe0a-6c00-7000-8000-000000000001");

        let exported = export_markdown(&note, "# Storage\n").unwrap();

        assert!(exported.contains("status: null\n"));
        assert!(exported.contains("tags: []\n"));
        assert!(exported.ends_with("# Storage\n"));
    }
}
