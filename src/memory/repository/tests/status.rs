use super::*;

#[test]
fn status_reports_dynamic_counts_and_levels() {
    let mut repository = repository();
    let empty = repository.status().unwrap();
    assert_eq!(empty.raw_count(), 0);
    assert_eq!(empty.highest_seq(), None);
    assert_eq!(empty.highest_completed_level(), None);

    append(&mut repository, 16, "status");
    repository
        .summarize(node(0, 0), NewSummary::new("status summary").unwrap())
        .unwrap();
    let status = repository.status().unwrap();
    assert_eq!(status.raw_count(), 16);
    assert_eq!(status.highest_seq(), Some(16));
    assert_eq!(status.summary_count(), 1);
    assert_eq!(status.pending_count(), 0);
    assert_eq!(status.highest_completed_level(), Some(0));
}

#[test]
fn status_counts_sparse_raw_sequences_independently_from_the_highest_sequence() {
    let repository = repository();
    repository
        .connection
        .execute(
            "INSERT INTO memories(seq, body, created)
             VALUES (100, 'sparse memory', '2026-08-22T12:34:56Z')",
            [],
        )
        .unwrap();

    let status = repository.status().unwrap();
    assert_eq!(status.raw_count(), 1);
    assert_eq!(status.highest_seq(), Some(100));
}
