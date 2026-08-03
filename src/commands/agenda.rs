use crate::cli::AgendaView;
use crate::display::agenda_line;
use crate::error::Result;
use crate::repository::{AgendaNote, Repository};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AgendaSection {
    Overdue,
    Today,
    Upcoming,
}

pub(super) fn agenda(view: Option<AgendaView>) -> Result<()> {
    let repository = Repository::open()?;
    let today = crate::note::local_day_now();
    crate::note::validate_date(&today)?;
    let through = match view {
        None => today.clone(),
        Some(AgendaView::Week) => crate::note::add_days(&today, 6)?,
    };
    let notes = repository.agenda_notes(&through)?;
    let sections = select_agenda(&notes, &today);
    let show_headers = view.is_none();
    for (section, notes) in sections {
        if notes.is_empty() {
            continue;
        }
        if show_headers {
            println!("{}", section_name(section));
        }
        for note in notes {
            println!("{}", agenda_line(note));
        }
    }
    Ok(())
}

fn select_agenda<'a>(
    notes: &'a [AgendaNote],
    today: &str,
) -> Vec<(AgendaSection, Vec<&'a AgendaNote>)> {
    let mut sections = vec![
        (AgendaSection::Overdue, Vec::new()),
        (AgendaSection::Today, Vec::new()),
        (AgendaSection::Upcoming, Vec::new()),
    ];
    for note in notes {
        let Some(section) = agenda_section(note, today) else {
            continue;
        };
        sections
            .iter_mut()
            .find(|(candidate, _)| *candidate == section)
            .unwrap()
            .1
            .push(note);
    }
    for (section, notes) in &mut sections {
        notes.sort_by(|left, right| {
            agenda_sort_key(left, *section)
                .cmp(&agenda_sort_key(right, *section))
                .then_with(|| right.created.cmp(&left.created))
                .then_with(|| right.id.cmp(&left.id))
        });
    }
    sections
}

fn agenda_section(note: &AgendaNote, today: &str) -> Option<AgendaSection> {
    if note.due.as_deref().is_some_and(|due| due < today) {
        return Some(AgendaSection::Overdue);
    }
    if note.due.as_deref() == Some(today)
        || note.scheduled.as_deref().is_some_and(|day| day <= today)
    {
        return Some(AgendaSection::Today);
    }
    if note.due.is_some() || note.scheduled.is_some() {
        Some(AgendaSection::Upcoming)
    } else {
        None
    }
}

fn agenda_sort_key(note: &AgendaNote, section: AgendaSection) -> (&str, u8) {
    let date = match section {
        AgendaSection::Overdue => note.due.as_deref().unwrap_or_default(),
        AgendaSection::Today | AgendaSection::Upcoming => {
            [note.scheduled.as_deref(), note.due.as_deref()]
                .into_iter()
                .flatten()
                .min()
                .unwrap_or_default()
        }
    };
    (date, priority_rank(note.priority.as_deref()))
}

fn priority_rank(priority: Option<&str>) -> u8 {
    match priority {
        Some("S") => 0,
        Some("A") => 1,
        Some("B") => 2,
        Some("C") => 3,
        Some("D") => 4,
        _ => 5,
    }
}

fn section_name(section: AgendaSection) -> &'static str {
    match section {
        AgendaSection::Overdue => "Overdue",
        AgendaSection::Today => "Today",
        AgendaSection::Upcoming => "Upcoming",
    }
}

#[cfg(test)]
mod tests {
    use crate::repository::AgendaNote;

    use super::{AgendaSection, select_agenda};

    fn todo(
        id: &str,
        priority: Option<&str>,
        scheduled: Option<&str>,
        due: Option<&str>,
    ) -> AgendaNote {
        AgendaNote {
            id: id.to_string(),
            priority: priority.map(str::to_string),
            scheduled: scheduled.map(str::to_string),
            due: due.map(str::to_string),
            created: "2026-05-28T14:30:12Z".to_string(),
            title: id.to_string(),
        }
    }

    #[test]
    fn agenda_sections_filtered_todos_and_orders_by_date_then_priority() {
        let notes = vec![
            todo("NT20260501T000001", Some("D"), None, Some("2026-05-27")),
            todo("NT20260502T000001", Some("S"), None, Some("2026-05-27")),
            todo(
                "NT20260503T000001",
                Some("A"),
                Some("2026-05-28"),
                Some("2026-06-02"),
            ),
            todo("NT20260504T000001", None, None, Some("2026-06-01")),
        ];
        let sections = select_agenda(&notes, "2026-05-28");

        assert_eq!(sections[0].0, AgendaSection::Overdue);
        assert_eq!(
            sections[0]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260502T000001", "NT20260501T000001"]
        );
        assert_eq!(
            sections[1]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260503T000001"]
        );
        assert_eq!(
            sections[2]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260504T000001"]
        );
    }

    #[test]
    fn agenda_orders_priorities_and_preserves_active_recency_for_ties() {
        let priorities = [None, Some("D"), Some("C"), Some("B"), Some("A"), Some("S")];
        let notes: Vec<_> = priorities
            .into_iter()
            .enumerate()
            .map(|(index, priority)| {
                todo(
                    &format!("NT202605{:02}T000001", index + 10),
                    priority,
                    None,
                    Some("2026-05-28"),
                )
            })
            .collect();
        let sections = select_agenda(&notes, "2026-05-28");
        let priorities: Vec<Option<&str>> = sections[1]
            .1
            .iter()
            .map(|note| note.priority.as_deref())
            .collect();
        assert_eq!(
            priorities,
            vec![Some("S"), Some("A"), Some("B"), Some("C"), Some("D"), None]
        );

        let mut newer = todo("NT20260520T000001", Some("A"), None, Some("2026-05-28"));
        newer.created = "2026-06-02T00:00:00Z".to_string();
        let mut older = todo("NT20260521T000001", Some("A"), None, Some("2026-05-28"));
        older.created = "2026-06-01T00:00:00Z".to_string();
        let notes = [older, newer];
        let sections = select_agenda(&notes, "2026-05-28");
        assert_eq!(
            sections[1]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260520T000001", "NT20260521T000001"]
        );
    }
}
