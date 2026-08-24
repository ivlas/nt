use crate::storage::schema_engine::SchemaObject;

pub(crate) const MEMORY: SchemaObject = SchemaObject {
    object_type: "table",
    name: "memory",
    sql: "CREATE TABLE memory (
         sequence INTEGER PRIMARY KEY CHECK (sequence >= 0),
         created_at TEXT NOT NULL,
         body TEXT NOT NULL,
         CHECK(created_at GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(length(body) > 0 AND length(body) <= 512),
         CHECK(instr(body, char(0)) = 0),
         CHECK(instr(body, char(10)) = 0 AND instr(body, char(13)) = 0)
     )",
};

pub(crate) const MEMORY_SUMMARY: SchemaObject = SchemaObject {
    object_type: "table",
    name: "memory_summary",
    sql: "CREATE TABLE memory_summary (
         lo INTEGER NOT NULL CHECK (lo >= 0),
         hi INTEGER NOT NULL CHECK (hi > lo),
         body TEXT NOT NULL,
         PRIMARY KEY(lo, hi),
         CHECK(hi - lo >= 2),
         CHECK(((hi - lo) & (hi - lo - 1)) = 0),
         CHECK(lo % (hi - lo) = 0),
         CHECK(length(body) > 0 AND length(body) <= 512),
         CHECK(instr(body, char(0)) = 0),
         CHECK(instr(body, char(10)) = 0 AND instr(body, char(13)) = 0)
     ) WITHOUT ROWID",
};

pub(crate) const MEMORY_SUMMARY_SIZE: SchemaObject = SchemaObject {
    object_type: "index",
    name: "memory_summary_size",
    sql: "CREATE INDEX memory_summary_size ON memory_summary(hi - lo, lo)",
};

pub(crate) const MEMORY_IMMUTABLE_INSERT: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memory_immutable_insert",
    sql: "CREATE TRIGGER memory_immutable_insert BEFORE INSERT ON memory
     WHEN new.sequence != COALESCE((SELECT MAX(sequence) + 1 FROM memory), 0) BEGIN
         SELECT RAISE(ABORT, 'raw memory must append contiguously');
     END",
};

pub(crate) const MEMORY_IMMUTABLE_UPDATE: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memory_immutable_update",
    sql: "CREATE TRIGGER memory_immutable_update BEFORE UPDATE ON memory BEGIN
         SELECT RAISE(ABORT, 'raw memory is immutable');
     END",
};

pub(crate) const MEMORY_IMMUTABLE_DELETE: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memory_immutable_delete",
    sql: "CREATE TRIGGER memory_immutable_delete BEFORE DELETE ON memory BEGIN
         SELECT RAISE(ABORT, 'raw memory is immutable');
     END",
};

pub(crate) const OBJECTS: &[SchemaObject] = &[
    MEMORY,
    MEMORY_SUMMARY,
    MEMORY_SUMMARY_SIZE,
    MEMORY_IMMUTABLE_INSERT,
    MEMORY_IMMUTABLE_UPDATE,
    MEMORY_IMMUTABLE_DELETE,
];

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::OBJECTS;

    fn schema() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        for object in OBJECTS {
            connection.execute_batch(object.sql).unwrap();
        }
        connection
    }

    #[test]
    fn raw_rows_are_contiguous_single_line_and_immutable() {
        let connection = schema();
        assert!(
            connection
                .execute(
                    "INSERT INTO memory(sequence, created_at, body)
                     VALUES (1, '2026-08-22T12:34:56Z', 'nonzero start')",
                    [],
                )
                .is_err()
        );
        connection
            .execute(
                "INSERT INTO memory(sequence, created_at, body)
                 VALUES (0, '2026-08-22T12:34:56Z', 'event')",
                [],
            )
            .unwrap();
        assert!(
            connection
                .execute(
                    "INSERT INTO memory(sequence, created_at, body)
                     VALUES (2, '2026-08-22T12:34:56Z', 'skipped sequence')",
                    [],
                )
                .is_err()
        );
        for sql in [
            "UPDATE memory SET body = 'changed' WHERE sequence = 0",
            "DELETE FROM memory WHERE sequence = 0",
            "INSERT OR REPLACE INTO memory(sequence, created_at, body) VALUES (0, '2026-08-22T12:34:56Z', 'changed')",
        ] {
            assert!(connection.execute(sql, []).is_err());
        }
        assert!(
            connection
                .execute(
                    "INSERT INTO memory(sequence, created_at, body)
                     VALUES (1, '2026-08-22T12:34:56Z', 'two' || char(10) || 'lines')",
                    [],
                )
                .is_err()
        );
    }

    #[test]
    fn summaries_must_be_aligned_binary_ranges() {
        let connection = schema();
        for (lo, hi) in [(0, 2), (4, 8), (0, 16)] {
            connection
                .execute(
                    "INSERT INTO memory_summary(lo, hi, body) VALUES (?1, ?2, 'summary')",
                    [lo, hi],
                )
                .unwrap();
        }
        for (lo, hi) in [(0, 1), (0, 3), (2, 6), (-1, 1)] {
            assert!(
                connection
                    .execute(
                        "INSERT INTO memory_summary(lo, hi, body) VALUES (?1, ?2, 'bad')",
                        [lo, hi],
                    )
                    .is_err()
            );
        }
    }
}
