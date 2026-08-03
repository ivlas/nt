#[cfg(test)]
use crate::repository::NoteMeta;
use crate::repository::{AgendaNote, FindRow};
use crate::terminal::{Style, paint};

#[cfg(test)]
pub(crate) fn summary_line(note: &NoteMeta) -> String {
    summary_line_for_display(note, false)
}

#[cfg(test)]
pub(crate) fn summary_line_for_display(note: &NoteMeta, color: bool) -> String {
    summary_line_values(&note.id, &note.created, &note.title, &note.tags, color)
}

pub(crate) fn find_summary_line(note: &FindRow) -> String {
    summary_line_values(&note.id, &note.created, &note.title, &note.tags, false)
}

fn summary_line_values(
    id: &str,
    created: &str,
    title: &str,
    tags: &[String],
    color: bool,
) -> String {
    let day = created.get(0..10).unwrap_or("unknown");
    let tags = joined_or_dash(tags);
    let padded_tags = format!("{tags:<12}");

    format!(
        "{}  {}  {}  {}",
        paint(id, Style::BrightCyan, color),
        paint(day, Style::Dim, color),
        paint(&padded_tags, Style::Green, color),
        title
    )
}

pub(crate) fn joined_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
    }
}

pub(crate) fn agenda_line(note: &AgendaNote) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}",
        note.id,
        note.priority.as_deref().unwrap_or("-"),
        note.scheduled.as_deref().unwrap_or("-"),
        note.due.as_deref().unwrap_or("-"),
        note.title
    )
}

#[cfg(test)]
mod tests {
    use crate::repository::{AgendaNote, NoteMeta};

    use super::{agenda_line, summary_line, summary_line_for_display};

    fn note(id: &str) -> NoteMeta {
        NoteMeta::new_note(
            id.to_string(),
            "personal/inbox".to_string(),
            "# Storage shape\n".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "Storage shape".to_string(),
        )
    }

    #[test]
    fn summary_line_is_stable() {
        let mut note = note("018fbe0a-6c00-7000-8000-000000000001");
        note.tags = vec!["design".to_string()];

        assert_eq!(
            summary_line(&note),
            "018fbe0a-6c00-7000-8000-000000000001  2026-05-28  design        Storage shape"
        );
    }

    #[test]
    fn summary_line_uses_dash_for_empty_tags() {
        let note = note("018fbe0a-6c00-7000-8000-000000000001");

        assert_eq!(
            summary_line(&note),
            "018fbe0a-6c00-7000-8000-000000000001  2026-05-28  -             Storage shape"
        );
    }

    #[test]
    fn summary_line_colors_human_display_when_enabled() {
        let mut note = note("018fbe0a-6c00-7000-8000-000000000001");
        note.tags = vec!["design".to_string()];

        let line = summary_line_for_display(&note, true);

        assert!(line.contains("\x1b[96m018fbe0a-6c00-7000-8000-000000000001\x1b[0m"));
        assert!(line.contains("\x1b[2m2026-05-28\x1b[0m"));
        assert!(line.contains("\x1b[32mdesign"));
        assert!(line.ends_with("Storage shape"));
    }

    #[test]
    fn agenda_line_omits_redundant_open_status() {
        let note = AgendaNote {
            id: "018fbe0a-6c00-7000-8000-000000000001".to_string(),
            priority: Some("A".to_string()),
            scheduled: Some("2026-05-28".to_string()),
            due: Some("2026-05-30".to_string()),
            created: "2026-05-28T14:30:12Z".to_string(),
            title: "Storage shape".to_string(),
        };

        assert_eq!(
            agenda_line(&note),
            "018fbe0a-6c00-7000-8000-000000000001\tA\t2026-05-28\t2026-05-30\tStorage shape"
        );
    }
}
