use crate::storage::schema_engine::SchemaObject;

pub(crate) const MEMORIES: SchemaObject = SchemaObject {
    object_type: "table",
    name: "memories",
    sql: "CREATE TABLE memories (
         seq INTEGER PRIMARY KEY CHECK (seq > 0),
         body TEXT NOT NULL,
         created TEXT NOT NULL,
         CHECK(length(body) > 0 AND length(body) <= 1024),
         CHECK(instr(body, char(0)) = 0),
         CHECK(created GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z')
     )",
};

pub(crate) const MEMORY_SEGMENTS: SchemaObject = SchemaObject {
    object_type: "table",
    name: "memory_segments",
    sql: "CREATE TABLE memory_segments (
         pk INTEGER PRIMARY KEY CHECK (pk > 0),
         level INTEGER NOT NULL CHECK (level >= 0),
         block INTEGER NOT NULL CHECK (block >= 0),
         summary TEXT NOT NULL,
         created TEXT NOT NULL,
         UNIQUE(level, block),
         CHECK(length(summary) > 0 AND length(summary) <= 1024),
         CHECK(instr(summary, char(0)) = 0),
         CHECK(created GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z')
     )",
};

pub(crate) const MEMORY_SUMMARY_JOBS: SchemaObject = SchemaObject {
    object_type: "table",
    name: "memory_summary_jobs",
    sql: "CREATE TABLE memory_summary_jobs (
         level INTEGER NOT NULL CHECK (level >= 0),
         block INTEGER NOT NULL CHECK (block >= 0),
         PRIMARY KEY(level, block)
     ) WITHOUT ROWID",
};

pub(crate) const MEMORY_FTS: SchemaObject = SchemaObject {
    object_type: "table",
    name: "memory_fts",
    sql: "CREATE VIRTUAL TABLE memory_fts USING fts5(
         body,
         content = 'memories',
         content_rowid = 'seq',
         tokenize = 'porter unicode61 remove_diacritics 2'
     )",
};

pub(crate) const MEMORY_SEGMENT_FTS: SchemaObject = SchemaObject {
    object_type: "table",
    name: "memory_segment_fts",
    sql: "CREATE VIRTUAL TABLE memory_segment_fts USING fts5(
         summary,
         content = 'memory_segments',
         content_rowid = 'pk',
         tokenize = 'porter unicode61 remove_diacritics 2'
     )",
};

pub(crate) const MEMORY_FTS_INSERT_TRIGGER: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memories_fts_insert",
    sql: "CREATE TRIGGER memories_fts_insert AFTER INSERT ON memories BEGIN
         INSERT INTO memory_fts(rowid, body) VALUES (new.seq, new.body);
     END",
};

pub(crate) const MEMORY_FTS_DELETE_TRIGGER: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memories_fts_delete",
    sql: "CREATE TRIGGER memories_fts_delete BEFORE DELETE ON memories BEGIN
         INSERT INTO memory_fts(memory_fts, rowid, body)
             VALUES ('delete', old.seq, old.body);
     END",
};

pub(crate) const MEMORIES_IMMUTABLE_UPDATE_TRIGGER: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memories_immutable_update",
    sql: "CREATE TRIGGER memories_immutable_update BEFORE UPDATE ON memories BEGIN
         SELECT RAISE(ABORT, 'raw memories are immutable');
     END",
};

pub(crate) const MEMORIES_IMMUTABLE_IDENTITY_TRIGGER: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memories_immutable_identity",
    sql: "CREATE TRIGGER memories_immutable_identity BEFORE INSERT ON memories
     WHEN EXISTS (SELECT 1 FROM memories WHERE seq = new.seq) BEGIN
         SELECT RAISE(ABORT, 'raw memories are immutable');
     END",
};

pub(crate) const MEMORIES_IMMUTABLE_DELETE_TRIGGER: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memories_immutable_delete",
    sql: "CREATE TRIGGER memories_immutable_delete BEFORE DELETE ON memories BEGIN
         SELECT RAISE(ABORT, 'raw memories are immutable');
     END",
};

pub(crate) const MEMORY_SEGMENT_FTS_INSERT_TRIGGER: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memory_segments_fts_insert",
    sql: "CREATE TRIGGER memory_segments_fts_insert AFTER INSERT ON memory_segments BEGIN
         INSERT INTO memory_segment_fts(rowid, summary) VALUES (new.pk, new.summary);
     END",
};

pub(crate) const MEMORY_SEGMENT_FTS_DELETE_TRIGGER: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memory_segments_fts_delete",
    sql: "CREATE TRIGGER memory_segments_fts_delete BEFORE DELETE ON memory_segments BEGIN
         INSERT INTO memory_segment_fts(memory_segment_fts, rowid, summary)
             VALUES ('delete', old.pk, old.summary);
     END",
};

pub(crate) const MEMORY_SEGMENTS_IMMUTABLE_UPDATE_TRIGGER: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memory_segments_immutable_update",
    sql: "CREATE TRIGGER memory_segments_immutable_update
     BEFORE UPDATE ON memory_segments BEGIN
         SELECT RAISE(ABORT, 'memory summaries are immutable');
     END",
};

pub(crate) const MEMORY_SEGMENTS_IMMUTABLE_IDENTITY_TRIGGER: SchemaObject = SchemaObject {
    object_type: "trigger",
    name: "memory_segments_immutable_identity",
    sql: "CREATE TRIGGER memory_segments_immutable_identity
     BEFORE INSERT ON memory_segments
     WHEN EXISTS (
         SELECT 1 FROM memory_segments
         WHERE pk = new.pk OR (level = new.level AND block = new.block)
     ) BEGIN
         SELECT RAISE(ABORT, 'memory summaries are immutable');
     END",
};

pub(crate) const MEMORIES_CREATED_INDEX: SchemaObject = SchemaObject {
    object_type: "index",
    name: "memories_created_idx",
    sql: "CREATE INDEX memories_created_idx ON memories(created DESC, seq DESC)",
};

pub(crate) const MEMORY_SEGMENTS_CREATED_INDEX: SchemaObject = SchemaObject {
    object_type: "index",
    name: "memory_segments_created_idx",
    sql: "CREATE INDEX memory_segments_created_idx
         ON memory_segments(created DESC, level DESC, block DESC)",
};

pub(crate) const OBJECTS: &[SchemaObject] = &[
    MEMORIES,
    MEMORY_SEGMENTS,
    MEMORY_SUMMARY_JOBS,
    MEMORY_FTS,
    MEMORY_SEGMENT_FTS,
    MEMORY_FTS_INSERT_TRIGGER,
    MEMORY_FTS_DELETE_TRIGGER,
    MEMORIES_IMMUTABLE_IDENTITY_TRIGGER,
    MEMORIES_IMMUTABLE_UPDATE_TRIGGER,
    MEMORIES_IMMUTABLE_DELETE_TRIGGER,
    MEMORY_SEGMENT_FTS_INSERT_TRIGGER,
    MEMORY_SEGMENT_FTS_DELETE_TRIGGER,
    MEMORY_SEGMENTS_IMMUTABLE_UPDATE_TRIGGER,
    MEMORY_SEGMENTS_IMMUTABLE_IDENTITY_TRIGGER,
    MEMORIES_CREATED_INDEX,
    MEMORY_SEGMENTS_CREATED_INDEX,
];

#[cfg(test)]
pub(crate) const REQUIRED_SHADOW_TABLES: &[&str] = &[
    "memory_fts_data",
    "memory_fts_idx",
    "memory_fts_docsize",
    "memory_fts_config",
    "memory_segment_fts_data",
    "memory_segment_fts_idx",
    "memory_segment_fts_docsize",
    "memory_segment_fts_config",
];

#[cfg(test)]
pub(crate) const ALLOWED_TRIGGERS: &[&str] = &[
    "memories_fts_insert",
    "memories_fts_delete",
    "memories_immutable_identity",
    "memories_immutable_update",
    "memories_immutable_delete",
    "memory_segments_fts_insert",
    "memory_segments_fts_delete",
    "memory_segments_immutable_update",
    "memory_segments_immutable_identity",
];

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, params};

    use super::{ALLOWED_TRIGGERS, OBJECTS, REQUIRED_SHADOW_TABLES};

    const CREATED: &str = "2026-08-22T12:34:56Z";

    fn schema() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        for object in OBJECTS {
            connection.execute_batch(object.sql).unwrap();
        }
        connection
    }

    #[test]
    fn manifest_has_expected_fts_and_immutability_triggers() {
        assert_eq!(REQUIRED_SHADOW_TABLES.len(), 8);
        assert_eq!(ALLOWED_TRIGGERS.len(), 9);
        assert!(
            OBJECTS
                .iter()
                .filter(|object| object.object_type == "trigger")
                .all(|object| ALLOWED_TRIGGERS.contains(&object.name))
        );
    }

    #[test]
    fn raw_memories_are_sqlite_sequenced_and_constrained() {
        let connection = schema();
        connection
            .execute(
                "INSERT INTO memories(body, created) VALUES (?1, ?2)",
                params!["alpha memory", CREATED],
            )
            .unwrap();
        assert_eq!(connection.last_insert_rowid(), 1);

        for body in ["".to_string(), "x".repeat(1_025), "a\0b".to_string()] {
            assert!(
                connection
                    .execute(
                        "INSERT INTO memories(body, created) VALUES (?1, ?2)",
                        params![body, CREATED],
                    )
                    .is_err()
            );
        }
        assert!(
            connection
                .execute(
                    "INSERT INTO memories(body, created) VALUES ('body', 'not-a-timestamp')",
                    [],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO memories(seq, body, created) VALUES (0, 'body', ?1)",
                    [CREATED],
                )
                .is_err()
        );
    }

    #[test]
    fn segments_and_jobs_enforce_node_and_content_constraints() {
        let connection = schema();
        connection
            .execute(
                "INSERT INTO memory_segments(level, block, summary, created)
                 VALUES (0, 0, 'summary', ?1)",
                [CREATED],
            )
            .unwrap();
        assert!(
            connection
                .execute(
                    "INSERT INTO memory_segments(level, block, summary, created)
                     VALUES (0, 0, 'duplicate', ?1)",
                    [CREATED],
                )
                .is_err()
        );
        for (level, block) in [(-1, 0), (0, -1)] {
            assert!(
                connection
                    .execute(
                        "INSERT INTO memory_summary_jobs(level, block) VALUES (?1, ?2)",
                        params![level, block],
                    )
                    .is_err()
            );
        }
        assert!(
            connection
                .execute(
                    "INSERT INTO memory_segments(level, block, summary, created)
                     VALUES (0, 1, ?1, ?2)",
                    params!["x".repeat(1_025), CREATED],
                )
                .is_err()
        );
    }

    #[test]
    fn external_content_indexes_follow_inserts_and_deletes() {
        let connection = schema();
        connection
            .execute(
                "INSERT INTO memories(body, created) VALUES ('alpha raw', ?1)",
                [CREATED],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO memory_segments(level, block, summary, created)
                 VALUES (0, 0, 'beta summary', ?1)",
                [CREATED],
            )
            .unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM memory_fts WHERE memory_fts MATCH 'alpha'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM memory_segment_fts
                     WHERE memory_segment_fts MATCH 'beta'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );

        assert!(
            connection
                .execute("UPDATE memories SET body = 'changed' WHERE seq = 1", [])
                .is_err()
        );
        assert!(
            connection
                .execute("DELETE FROM memories WHERE seq = 1", [])
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT OR REPLACE INTO memories(seq, body, created)
                     VALUES (1, 'replacement', ?1)",
                    [CREATED],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "UPDATE memory_segments SET summary = 'changed' WHERE pk = 1",
                    [],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT OR REPLACE INTO memory_segments(pk, level, block, summary, created)
                     VALUES (1, 0, 0, 'replacement', ?1)",
                    [CREATED],
                )
                .is_err()
        );
        connection
            .execute("DELETE FROM memory_segments WHERE pk = 1", [])
            .unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM memory_fts WHERE memory_fts MATCH 'alpha'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM memory_segment_fts
                     WHERE memory_segment_fts MATCH 'beta'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }
}
