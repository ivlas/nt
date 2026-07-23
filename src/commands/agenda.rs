use crate::cli::AgendaView;
use crate::display::agenda_line;
use crate::error::Result;
use crate::index::{Index, NoteMeta};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AgendaSection {
    Overdue,
    Today,
    Upcoming,
    Waiting,
    Undated,
}

pub(super) fn agenda(view: Option<AgendaView>) -> Result<()> {
    let index = Index::load()?;
    let today = crate::note::local_day_now();
    let sections = select_agenda(&index, &today, view)?;
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
    index: &'a Index,
    today: &str,
    view: Option<AgendaView>,
) -> Result<Vec<(AgendaSection, Vec<&'a NoteMeta>)>> {
    crate::note::validate_date(today)?;
    let week_end = crate::note::add_days(today, 6)?;
    let mut sections = vec![
        (AgendaSection::Overdue, Vec::new()),
        (AgendaSection::Today, Vec::new()),
        (AgendaSection::Upcoming, Vec::new()),
        (AgendaSection::Waiting, Vec::new()),
        (AgendaSection::Undated, Vec::new()),
    ];
    for note in index.active_notes() {
        if note.kind != "todo" || !matches!(note.status.as_deref(), Some("open" | "waiting")) {
            continue;
        }
        let section = agenda_section(note, today);
        let include = match view {
            None => true,
            Some(AgendaView::Today) => {
                matches!(section, AgendaSection::Overdue | AgendaSection::Today)
            }
            Some(AgendaView::Overdue) => section == AgendaSection::Overdue,
            Some(AgendaView::Waiting) => section == AgendaSection::Waiting,
            Some(AgendaView::Undated) => section == AgendaSection::Undated,
            Some(AgendaView::Week) => {
                note.status.as_deref() == Some("open")
                    && (section == AgendaSection::Overdue
                        || note
                            .scheduled
                            .as_deref()
                            .is_some_and(|day| day >= today && day <= week_end.as_str())
                        || note
                            .due
                            .as_deref()
                            .is_some_and(|day| day >= today && day <= week_end.as_str()))
            }
        };
        if include {
            sections
                .iter_mut()
                .find(|(candidate, _)| *candidate == section)
                .unwrap()
                .1
                .push(note);
        }
    }
    for (section, notes) in &mut sections {
        notes.sort_by(|left, right| {
            agenda_sort_key(left, *section).cmp(&agenda_sort_key(right, *section))
        });
    }
    Ok(sections)
}

fn agenda_section(note: &NoteMeta, today: &str) -> AgendaSection {
    if note.status.as_deref() == Some("waiting") {
        return AgendaSection::Waiting;
    }
    if note.due.as_deref().is_some_and(|due| due < today) {
        return AgendaSection::Overdue;
    }
    if note.due.as_deref() == Some(today)
        || note.scheduled.as_deref().is_some_and(|day| day <= today)
    {
        return AgendaSection::Today;
    }
    if note.due.is_some() || note.scheduled.is_some() {
        AgendaSection::Upcoming
    } else {
        AgendaSection::Undated
    }
}

fn agenda_sort_key(note: &NoteMeta, section: AgendaSection) -> (String, u8) {
    let date = match section {
        AgendaSection::Overdue => note.due.clone().unwrap_or_default(),
        AgendaSection::Today | AgendaSection::Upcoming => {
            [note.scheduled.as_ref(), note.due.as_ref()]
                .into_iter()
                .flatten()
                .min()
                .cloned()
                .unwrap_or_default()
        }
        AgendaSection::Waiting | AgendaSection::Undated => String::new(),
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
        AgendaSection::Waiting => "Waiting",
        AgendaSection::Undated => "Undated",
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::AgendaView;
    use crate::commands::test_helpers::{active_index, todo};

    use super::{AgendaSection, select_agenda};

    #[test]
    fn agenda_partitions_once_and_orders_dates_priorities_and_recency() {
        let notes = vec![
            todo(
                "NT20260501T000001",
                "open",
                Some("D"),
                None,
                Some("2026-05-27"),
            ),
            todo(
                "NT20260502T000001",
                "open",
                Some("S"),
                None,
                Some("2026-05-27"),
            ),
            todo(
                "NT20260503T000001",
                "open",
                Some("A"),
                Some("2026-05-28"),
                Some("2026-06-02"),
            ),
            todo("NT20260504T000001", "open", None, None, Some("2026-06-01")),
            todo(
                "NT20260505T000001",
                "waiting",
                Some("B"),
                Some("2026-05-20"),
                Some("2026-05-21"),
            ),
            todo("NT20260506T000001", "open", Some("C"), None, None),
            todo(
                "NT20260507T000001",
                "done",
                Some("S"),
                None,
                Some("2026-05-20"),
            ),
        ];
        let index = active_index(notes);
        let sections = select_agenda(&index, "2026-05-28", None).unwrap();

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
        assert_eq!(
            sections[3]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260505T000001"]
        );
        assert_eq!(
            sections[4]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260506T000001"]
        );
        assert_eq!(
            sections.iter().map(|(_, notes)| notes.len()).sum::<usize>(),
            6
        );

        let week = select_agenda(&index, "2026-05-28", Some(AgendaView::Week)).unwrap();
        assert_eq!(week.iter().map(|(_, notes)| notes.len()).sum::<usize>(), 4);

        let today = select_agenda(&index, "2026-05-28", Some(AgendaView::Today)).unwrap();
        assert_eq!(
            today
                .iter()
                .flat_map(|(_, notes)| notes.iter().map(|note| note.id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                "NT20260502T000001",
                "NT20260501T000001",
                "NT20260503T000001"
            ]
        );
    }

    #[test]
    fn agenda_orders_all_priorities_and_preserves_active_recency_for_ties() {
        let priorities = [None, Some("D"), Some("C"), Some("B"), Some("A"), Some("S")];
        let notes = priorities
            .into_iter()
            .enumerate()
            .map(|(index, priority)| {
                todo(
                    &format!("NT202605{:02}T000001", index + 10),
                    "open",
                    priority,
                    None,
                    None,
                )
            })
            .collect();
        let index = active_index(notes);
        let sections = select_agenda(&index, "2026-05-28", Some(AgendaView::Undated)).unwrap();
        let priorities: Vec<Option<&str>> = sections[4]
            .1
            .iter()
            .map(|note| note.priority.as_deref())
            .collect();
        assert_eq!(
            priorities,
            vec![Some("S"), Some("A"), Some("B"), Some("C"), Some("D"), None]
        );

        let mut newer = todo("NT20260520T000001", "open", Some("A"), None, None);
        newer.created = "2026-06-02T00:00:00Z".to_string();
        let mut older = todo("NT20260521T000001", "open", Some("A"), None, None);
        older.created = "2026-06-01T00:00:00Z".to_string();
        let index = active_index(vec![older, newer]);
        let sections = select_agenda(&index, "2026-05-28", Some(AgendaView::Undated)).unwrap();
        assert_eq!(
            sections[4]
                .1
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["NT20260520T000001", "NT20260521T000001"]
        );
    }

    #[test]
    fn agenda_excludes_non_actionable_notes_and_handles_empty_results() {
        let done = todo("NT20260520T000001", "done", Some("S"), None, None);
        let dropped = todo("NT20260521T000001", "dropped", Some("A"), None, None);
        let mut statusless = todo("NT20260522T000001", "open", Some("B"), None, None);
        statusless.status = None;
        let mut non_todo = todo("NT20260523T000001", "open", Some("C"), None, None);
        non_todo.kind = "note".to_string();
        let index = active_index(vec![done, dropped, statusless, non_todo]);

        for view in [
            None,
            Some(AgendaView::Today),
            Some(AgendaView::Week),
            Some(AgendaView::Overdue),
            Some(AgendaView::Waiting),
            Some(AgendaView::Undated),
        ] {
            let sections = select_agenda(&index, "2026-05-28", view).unwrap();
            assert!(sections.iter().all(|(_, notes)| notes.is_empty()));
        }
    }
}
