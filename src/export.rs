use crate::error::Result;
use crate::index::NoteMeta;

pub(crate) fn export_markdown(note: &NoteMeta, body: &str) -> Result<String> {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::index::NoteMeta;

    use super::export_markdown;

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
        assert!(exported.ends_with("# Storage\n"));
    }
}
