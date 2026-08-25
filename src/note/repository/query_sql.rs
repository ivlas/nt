use rusqlite::types::Value;

use super::super::{Filter, NoteQuery};
use crate::lexical::fts_and_expression;

pub(super) fn compile_ordered_query(query: &NoteQuery) -> (String, Vec<Value>) {
    let mut parameters = Vec::new();
    let mut expressions = query
        .filters()
        .iter()
        .map(|filter| compile_filter(filter, &mut parameters))
        .collect::<Vec<_>>();
    if !query.lexical_terms().is_empty() {
        let fts_query = fts_and_expression(query.lexical_terms());
        let parameter = push_parameter(&mut parameters, &fts_query);
        expressions.push(format!(
            "n.pk IN (SELECT rowid FROM note_fts WHERE note_fts MATCH ?{parameter})"
        ));
    }
    let where_sql = if expressions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", expressions.join(" AND "))
    };
    let limit_sql = if let Some(limit) = query.limit() {
        parameters.push(Value::Integer(limit));
        format!("LIMIT ?{}", parameters.len())
    } else {
        String::new()
    };
    (
        format!("{where_sql} ORDER BY n.updated DESC, n.id DESC {limit_sql}"),
        parameters,
    )
}

fn compile_filter(filter: &Filter, parameters: &mut Vec<Value>) -> String {
    match filter {
        Filter::IdPrefix(prefix) => {
            let lower = push_parameter(parameters, prefix);
            let upper = push_parameter(parameters, &prefix_upper_bound(prefix));
            format!("n.id >= ?{lower} AND n.id < ?{upper}")
        }
        Filter::Collection(collection) => {
            let parameter = push_parameter(parameters, collection.as_str());
            format!("n.collection = ?{parameter}")
        }
        Filter::Tag(tag) => {
            let parameter = push_parameter(parameters, tag.as_str());
            format!(
                "EXISTS (SELECT 1 FROM note_tags filter_tags
                 WHERE filter_tags.note_pk = n.pk AND filter_tags.tag = ?{parameter})"
            )
        }
        Filter::LinksTo(target) => {
            let parameter = push_parameter(parameters, &target.to_string());
            format!(
                "EXISTS (SELECT 1 FROM note_links filter_links
                 JOIN notes filter_target ON filter_target.pk = filter_links.target_note_pk
                 WHERE filter_links.note_pk = n.pk AND filter_target.id = ?{parameter})"
            )
        }
        Filter::LinkedFrom(source) => {
            let parameter = push_parameter(parameters, &source.to_string());
            format!(
                "n.pk IN (SELECT filter_links.target_note_pk
                 FROM notes filter_source
                 JOIN note_links filter_links ON filter_links.note_pk = filter_source.pk
                 WHERE filter_source.id = ?{parameter})"
            )
        }
        Filter::CreatedSince(timestamp) => {
            let parameter = push_parameter(parameters, timestamp.as_str());
            format!("n.created >= ?{parameter}")
        }
        Filter::UpdatedSince(timestamp) => {
            let parameter = push_parameter(parameters, timestamp.as_str());
            format!("n.updated >= ?{parameter}")
        }
        Filter::Not(inner) => format!("NOT ({})", compile_filter(inner, parameters)),
    }
}

fn prefix_upper_bound(prefix: &str) -> String {
    let mut upper = prefix.as_bytes().to_vec();
    *upper
        .last_mut()
        .expect("validated ID prefixes are nonempty") += 1;
    String::from_utf8(upper).expect("validated ID prefixes are ASCII")
}

fn push_parameter(parameters: &mut Vec<Value>, value: &str) -> usize {
    parameters.push(Value::Text(value.to_string()));
    parameters.len()
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::super::Repository;
    use super::*;
    use crate::schema;

    fn repository() -> Repository {
        let mut connection = rusqlite::Connection::open_in_memory().unwrap();
        schema::initialize(&mut connection).unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON")
            .unwrap();
        Repository { connection }
    }

    fn query_plan(
        connection: &rusqlite::Connection,
        sql: &str,
        parameters: impl rusqlite::Params,
    ) -> Vec<String> {
        connection
            .prepare(sql)
            .unwrap()
            .query_map(parameters, |row| row.get(3))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    #[ignore = "manual 20,000-note query-plan audit"]
    fn audit_id_prefix_query_plans_at_representative_scale() {
        let repository = repository();
        repository
            .connection
            .execute_batch(
                "WITH RECURSIVE generated(x) AS (
                     VALUES(0)
                     UNION ALL
                     SELECT x + 1 FROM generated WHERE x < 19999
                 )
                 INSERT INTO notes(id, collection, body, title, created, updated, note_revision)
                 SELECT printf(
                            '0198abcd-%04x-7%03x-8%03x-%012x',
                            x, x % 4096, x / 4096, x
                        ),
                        CASE x % 3
                            WHEN 0 THEN 'inbox'
                            WHEN 1 THEN 'work/nt'
                            ELSE 'research/sqlite'
                        END,
                        printf('# Audit note %d\nSQLite prefix retrieval', x),
                        printf('Audit note %d', x),
                        strftime('%Y-%m-%dT%H:%M:%SZ', 1767225600 + x, 'unixepoch'),
                        strftime(
                            '%Y-%m-%dT%H:%M:%SZ',
                            1767225600 + ((x * 7919) % 20000),
                            'unixepoch'
                        ),
                        x + 1
                 FROM generated",
            )
            .unwrap();
        assert_eq!(
            repository
                .connection
                .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            20_000
        );

        println!("SQLite {}", rusqlite::version());
        let baseline = query_plan(
            &repository.connection,
            "EXPLAIN QUERY PLAN
             SELECT n.id, n.updated FROM notes n
             ORDER BY n.updated DESC, n.id DESC",
            [],
        );
        println!("unfiltered baseline: {baseline:?}");
        for (name, prefix) in [
            ("short", "0198"),
            ("medium", "0198abcd-1"),
            ("almost-full", "0198abcd-1234-7234-8001-00000000123"),
            ("full", "0198abcd-1234-7234-8001-000000001234"),
        ] {
            let before = query_plan(
                &repository.connection,
                "EXPLAIN QUERY PLAN
                 SELECT n.id, n.updated FROM notes n
                 WHERE substr(n.id, 1, length(?1)) = ?1
                 ORDER BY n.updated DESC, n.id DESC",
                [prefix],
            );
            let upper = prefix_upper_bound(prefix);
            let after = query_plan(
                &repository.connection,
                "EXPLAIN QUERY PLAN
                 SELECT n.id, n.updated FROM notes n
                 WHERE n.id >= ?1 AND n.id < ?2
                 ORDER BY n.updated DESC, n.id DESC",
                params![prefix, upper],
            );
            let before_count = repository
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM notes
                     WHERE substr(id, 1, length(?1)) = ?1",
                    [prefix],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap();
            let after_count = repository
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM notes WHERE id >= ?1 AND id < ?2",
                    params![prefix, prefix_upper_bound(prefix)],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap();

            assert_eq!(before_count, after_count);
            println!("{name} ({before_count} matches)\n  before: {before:?}\n  after: {after:?}");
        }
    }
}
