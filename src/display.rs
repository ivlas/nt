use crate::index::NoteMeta;
use crate::terminal::{Style, paint};

pub(crate) fn summary_line(note: &NoteMeta) -> String {
    summary_line_for_display(note, false)
}

pub(crate) fn summary_line_for_display(note: &NoteMeta, color: bool) -> String {
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

pub(crate) fn joined_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
    }
}

pub(crate) fn agenda_line(note: &NoteMeta) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        note.id,
        note.status.as_deref().unwrap_or("-"),
        note.priority.as_deref().unwrap_or("-"),
        note.scheduled.as_deref().unwrap_or("-"),
        note.due.as_deref().unwrap_or("-"),
        note.title
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::index::NoteMeta;

    use super::{summary_line, summary_line_for_display};

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
}
