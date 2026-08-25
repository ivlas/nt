use std::collections::BTreeSet;

use rusqlite::params;

use super::super::super::{CollectionPath, NewNote, NoteId, NoteQuery};
use super::{repository, summaries};

#[test]
fn list_read_and_find_are_complete_by_default() {
    let mut repository = repository();
    for index in 0..1101 {
        repository
            .create_note(
                NewNote::new(
                    CollectionPath::inbox(),
                    format!("# Note {index}\nshared limit term"),
                )
                .unwrap(),
            )
            .unwrap();
    }

    let list = NoteQuery::parse_list(&[]).unwrap();
    assert_eq!(summaries(&repository, &list).len(), 1101);
    let mut read_count = 0;
    repository
        .visit_notes(&list, |_| {
            read_count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(read_count, 1101);
    let find = NoteQuery::parse_find(&["shared".to_string()]).unwrap();
    assert_eq!(summaries(&repository, &find).len(), 1101);

    let list = NoteQuery::parse_list(&["limit:7".to_string()]).unwrap();
    assert_eq!(summaries(&repository, &list).len(), 7);
    let mut read_count = 0;
    repository
        .visit_notes(&list, |_| {
            read_count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(read_count, 7);
    let find = NoteQuery::parse_find(&["shared".to_string(), "limit:5".to_string()]).unwrap();
    assert_eq!(summaries(&repository, &find).len(), 5);
}

#[test]
fn list_filters_are_and_combined_and_negatable() {
    let mut repository = repository();
    repository
        .create_note(
            NewNote::new("work/nt".parse().unwrap(), "# Rust")
                .unwrap()
                .with_tags(["rust".parse().unwrap()]),
        )
        .unwrap();
    repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Other").unwrap())
        .unwrap();
    let query = NoteQuery::parse_list(&[
        "collection:work/nt".to_string(),
        "not:tag:sqlite".to_string(),
    ])
    .unwrap();
    let notes = summaries(&repository, &query);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title(), "Rust");
}

#[test]
fn id_prefix_filters_preserve_matching_composition_and_ordering() {
    let repository = repository();
    let first: NoteId = "0198abcd-0000-7000-8000-000000000001".parse().unwrap();
    let second: NoteId = "0198abcd-0000-7000-8000-000000000002".parse().unwrap();
    let third: NoteId = "0198abcd-1000-7000-8000-000000000003".parse().unwrap();
    repository
        .connection
        .execute_batch(
            "INSERT INTO notes(id, collection, body, title, created, updated) VALUES
                 ('0198abcd-0000-7000-8000-000000000001', 'work/nt',
                  '# First\nSQLite prefix audit', 'First',
                  '2026-01-01T00:00:00Z', '2026-01-03T00:00:00Z'),
                 ('0198abcd-0000-7000-8000-000000000002', 'inbox',
                  '# Second\nOther prefix audit', 'Second',
                  '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z'),
                 ('0198abcd-1000-7000-8000-000000000003', 'work/nt',
                  '# Third\nSQLite prefix audit', 'Third',
                  '2026-01-01T00:00:00Z', '2026-01-03T00:00:00Z');
             INSERT INTO note_tags(note_pk, tag)
             SELECT pk, 'rust' FROM notes
             WHERE id IN (
                 '0198abcd-0000-7000-8000-000000000001',
                 '0198abcd-1000-7000-8000-000000000003'
             )",
        )
        .unwrap();

    let ids = |query: NoteQuery| {
        summaries(&repository, &query)
            .into_iter()
            .map(|summary| summary.id().clone())
            .collect::<Vec<_>>()
    };

    assert_eq!(
        ids(NoteQuery::parse_list(&["id:0198".to_string()]).unwrap()),
        [third.clone(), first.clone(), second.clone()]
    );
    assert_eq!(
        ids(NoteQuery::parse_list(&["id:0198abcd-0".to_string()]).unwrap()),
        [first.clone(), second]
    );
    assert_eq!(
        ids(NoteQuery::parse_list(&[format!("id:{first}")]).unwrap()),
        std::slice::from_ref(&first)
    );
    assert!(ids(NoteQuery::parse_list(&["id:0199".to_string()]).unwrap()).is_empty());

    let tagged = NoteQuery::parse_list(&["id:0198".to_string(), "tag:rust".to_string()]).unwrap();
    assert_eq!(ids(tagged), [third.clone(), first.clone()]);

    let found = NoteQuery::parse_find(&["sqlite".to_string(), "id:0198".to_string()]).unwrap();
    assert_eq!(ids(found), [third, first]);
}

#[test]
fn directional_link_filters_compose_and_preserve_order_and_limits() {
    let mut repository = repository();
    let b = repository
        .create_note(
            NewNote::new(CollectionPath::inbox(), "# B\nsqlite target")
                .unwrap()
                .with_tags(["rust".parse().unwrap()]),
        )
        .unwrap();
    let c = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# C\nother target").unwrap())
        .unwrap();
    let a = repository
        .create_note(
            NewNote::new(CollectionPath::inbox(), "# A\nsource")
                .unwrap()
                .with_links([b.clone(), c.clone()]),
        )
        .unwrap();
    let d = repository
        .create_note(
            NewNote::new(CollectionPath::inbox(), "# D\nsource")
                .unwrap()
                .with_links([b.clone()]),
        )
        .unwrap();
    repository
        .connection
        .execute(
            "UPDATE notes SET updated = '2026-01-01T00:00:00Z' WHERE id IN (?1, ?2)",
            params![b.to_string(), c.to_string()],
        )
        .unwrap();
    repository
        .connection
        .execute(
            "UPDATE notes SET updated = '2026-01-02T00:00:00Z' WHERE id IN (?1, ?2)",
            params![a.to_string(), d.to_string()],
        )
        .unwrap();

    let ids = |query: NoteQuery| {
        summaries(&repository, &query)
            .into_iter()
            .map(|summary| summary.id().clone())
            .collect::<Vec<_>>()
    };
    let mut expected_sources = vec![a.clone(), d.clone()];
    expected_sources.sort_by_key(|id| std::cmp::Reverse(id.to_string()));
    let mut expected_targets = vec![b.clone(), c.clone()];
    expected_targets.sort_by_key(|id| std::cmp::Reverse(id.to_string()));

    assert_eq!(
        ids(NoteQuery::parse_list(&[format!("links-to:{b}")]).unwrap()),
        expected_sources
    );
    assert_eq!(
        ids(NoteQuery::parse_list(&[format!("linked-from:{a}")]).unwrap()),
        expected_targets
    );
    assert_eq!(
        ids(NoteQuery::parse_list(&[format!("linked-from:{d}")]).unwrap()),
        std::slice::from_ref(&b)
    );
    assert!(ids(NoteQuery::parse_list(&[format!("linked-from:{c}")]).unwrap()).is_empty());

    let tagged =
        NoteQuery::parse_list(&[format!("linked-from:{a}"), "tag:rust".to_string()]).unwrap();
    assert_eq!(ids(tagged), std::slice::from_ref(&b));

    let found = NoteQuery::parse_find(&["sqlite".to_string(), format!("linked-from:{a}")]).unwrap();
    assert_eq!(ids(found), std::slice::from_ref(&b));

    let excluded = NoteQuery::parse_list(&[format!("not:linked-from:{a}")]).unwrap();
    assert_eq!(
        ids(excluded).into_iter().collect::<BTreeSet<_>>(),
        [a.clone(), d.clone()].into_iter().collect()
    );

    let limited =
        NoteQuery::parse_list(&[format!("linked-from:{a}"), "limit:1".to_string()]).unwrap();
    assert_eq!(ids(limited), expected_targets[..1]);

    let missing = "018fbe0a-6c00-7000-8000-000000000001";
    assert!(ids(NoteQuery::parse_list(&[format!("linked-from:{missing}")]).unwrap()).is_empty());
}

#[test]
fn find_uses_literal_complete_tokens_with_structured_filters() {
    let mut repository = repository();
    repository
        .create_note(
            NewNote::new(
                "work/nt".parse().unwrap(),
                "# Café storage\nOwnership and borrowing.",
            )
            .unwrap()
            .with_tags(["rust".parse().unwrap()]),
        )
        .unwrap();
    repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Storage shed").unwrap())
        .unwrap();

    let query =
        NoteQuery::parse_find(&["cafe ownership".to_string(), "tag:rust".to_string()]).unwrap();
    let notes = summaries(&repository, &query);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title(), "Café storage");

    let prefix = NoteQuery::parse_find(&["stor".to_string()]).unwrap();
    assert!(summaries(&repository, &prefix).is_empty());
    let punctuation = NoteQuery::parse_find(&["(storage*)".to_string()]).unwrap();
    assert_eq!(summaries(&repository, &punctuation).len(), 2);
}

#[test]
fn list_find_and_limits_break_timestamp_ties_by_descending_id() {
    let mut repository = repository();
    let first = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# First\nordered").unwrap())
        .unwrap();
    let second = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Second\nordered").unwrap())
        .unwrap();
    let newest = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Newest\nordered").unwrap())
        .unwrap();
    repository
        .connection
        .execute(
            "UPDATE notes SET updated = '2026-01-02T00:00:00Z' WHERE id IN (?1, ?2)",
            params![first.to_string(), second.to_string()],
        )
        .unwrap();
    repository
        .connection
        .execute(
            "UPDATE notes SET updated = '2026-01-03T00:00:00Z' WHERE id = ?1",
            [newest.to_string()],
        )
        .unwrap();

    let mut tied = [first, second];
    tied.sort_by_key(|id| std::cmp::Reverse(id.to_string()));
    let expected = vec![newest, tied[0].clone(), tied[1].clone()];
    for query in [
        NoteQuery::parse_list(&[]).unwrap(),
        NoteQuery::parse_find(&["ordered".to_string()]).unwrap(),
    ] {
        let actual = summaries(&repository, &query)
            .into_iter()
            .map(|summary| summary.id().clone())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    let limited = NoteQuery::parse_find(&["ordered".to_string(), "limit:2".to_string()]).unwrap();
    let actual = summaries(&repository, &limited)
        .into_iter()
        .map(|summary| summary.id().clone())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected[..2]);
}
