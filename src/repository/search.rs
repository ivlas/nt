use std::error::Error;
use std::str::FromStr;

use crate::error::Result;
use crate::listing::{ListField, ListRow};
use crate::note::Date;
use crate::query::{ListFilter, Query};

use super::{AgendaNote, FindRow, Repository};

const AGENDA_SQL: &str = "SELECT id, priority, scheduled, due, created, title
     FROM notes
     WHERE kind = 'todo'
       AND status = 'open'
       AND (scheduled <= ?1 OR due <= ?1)
     ORDER BY created DESC, id DESC";

impl Repository {
    pub fn list_rows(&self, fields: &[ListField], filters: &[ListFilter]) -> Result<Vec<ListRow>> {
        let (sql, parameters) = list_query_sql(fields, filters);

        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(parameters.iter()), |row| {
            let values = (0..fields.len())
                .map(|column| row.get(column))
                .collect::<rusqlite::Result<Vec<String>>>()?;
            Ok(ListRow { values })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn find_rows(&self, query: &Query) -> Result<Vec<FindRow>> {
        let (sql, parameters) = find_query_sql(query);

        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(parameters.iter()), |row| {
            Ok((
                domain_from_row(row, 0)?,
                domain_from_row(row, 1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut found: Vec<FindRow> = Vec::new();
        for row in rows {
            let (id, created, title, tag) = row?;
            if let Some(current) = found.last_mut().filter(|current| current.id == id) {
                if let Some(tag) = tag {
                    current.tags.push(tag);
                }
            } else {
                found.push(FindRow {
                    id,
                    created,
                    title,
                    tags: tag.into_iter().collect(),
                });
            }
        }
        Ok(found)
    }

    pub fn agenda_notes(&self, through: &Date) -> Result<Vec<AgendaNote>> {
        let mut statement = self.connection.prepare(AGENDA_SQL)?;
        let rows = statement.query_map([through.as_str()], |row| {
            Ok(AgendaNote {
                id: domain_from_row(row, 0)?,
                priority: optional_domain_from_row(row, 1)?,
                scheduled: optional_domain_from_row(row, 2)?,
                due: optional_domain_from_row(row, 3)?,
                created: domain_from_row(row, 4)?,
                title: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

fn list_query_sql(fields: &[ListField], filters: &[ListFilter]) -> (String, Vec<String>) {
    let projection = fields
        .iter()
        .map(|field| list_field_sql(*field))
        .collect::<Vec<_>>()
        .join(", ");
    let mut sql = format!("SELECT {projection} FROM notes n");
    let mut parameters = Vec::new();
    if !filters.is_empty() {
        sql.push_str(" WHERE ");
        for (index, filter) in filters.iter().enumerate() {
            if index > 0 {
                sql.push_str(" AND ");
            }
            push_list_filter_sql(&mut sql, &mut parameters, filter);
        }
    }
    sql.push_str(" ORDER BY n.created DESC, n.id DESC");
    (sql, parameters)
}

fn find_query_sql(query: &Query) -> (String, Vec<String>) {
    let query = query.sql();
    let sql = format!(
        "SELECT n.id, n.created, n.title, output_tags.tag
         FROM notes n
         LEFT JOIN note_tags output_tags ON output_tags.note_id = n.id
         WHERE {}
         ORDER BY n.created DESC, n.id DESC, output_tags.tag",
        query.predicate
    );
    (sql, query.parameters)
}

fn list_field_sql(field: ListField) -> &'static str {
    match field {
        ListField::Id => "n.id",
        ListField::Home => {
            "(SELECT v.name || '/' || c.name
              FROM collections c JOIN vaults v ON v.id = c.vault_id
              WHERE c.id = n.home_collection_id)"
        }
        ListField::Created => "n.created",
        ListField::Updated => "n.updated",
        ListField::Title => "n.title",
        ListField::Kind => "n.kind",
        ListField::Status => "COALESCE(n.status, '-')",
        ListField::Priority => "COALESCE(n.priority, '-')",
        ListField::Scheduled => "COALESCE(n.scheduled, '-')",
        ListField::Due => "COALESCE(n.due, '-')",
        ListField::Closed => "COALESCE(n.closed, '-')",
        ListField::Tag => {
            "COALESCE((SELECT group_concat(value, ',') FROM (
                 SELECT tag AS value FROM note_tags
                 WHERE note_id = n.id ORDER BY tag
             )), '-')"
        }
        ListField::Collection => {
            "COALESCE((SELECT group_concat(value, ',') FROM (
                 SELECT v.name || '/' || c.name AS value
                 FROM note_collections nc
                 JOIN collections c ON c.id = nc.collection_id
                 JOIN vaults v ON v.id = c.vault_id
                 WHERE nc.note_id = n.id ORDER BY v.name, c.name
             )), '-')"
        }
        ListField::Link => {
            "COALESCE((SELECT group_concat(value, ',') FROM (
                 SELECT target_id AS value FROM note_links
                 WHERE note_id = n.id ORDER BY target_id
             )), '-')"
        }
        ListField::Source => {
            "COALESCE((SELECT group_concat(value, ',') FROM (
                 SELECT source AS value FROM note_sources
                 WHERE note_id = n.id ORDER BY source
             )), '-')"
        }
    }
}

fn push_list_filter_sql(sql: &mut String, parameters: &mut Vec<String>, filter: &ListFilter) {
    match filter {
        ListFilter::Id(value) => {
            sql.push_str("n.id LIKE ?");
            parameters.push(format!("{value}%"));
        }
        ListFilter::Tag(value) => {
            sql.push_str(
                "n.id IN (SELECT nt.note_id FROM note_tags nt
                 WHERE LOWER(nt.tag) = ?)",
            );
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Day(value) => {
            sql.push_str("substr(n.created, 1, 10) = ?");
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Since(value) => {
            sql.push_str("substr(n.created, 1, 10) >= ?");
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Before(value) => {
            sql.push_str("substr(n.created, 1, 10) < ?");
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Kind(value) => {
            sql.push_str("LOWER(n.kind) = ?");
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Status(value) => {
            sql.push_str("(n.status IS NOT NULL AND LOWER(n.status) = ?)");
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Priority(value) => {
            sql.push_str("COALESCE(n.priority = ?, 0)");
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Scheduled(value) => {
            sql.push_str("COALESCE(n.scheduled = ?, 0)");
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Due(value) => {
            sql.push_str("COALESCE(n.due = ?, 0)");
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Closed(value) => {
            sql.push_str("COALESCE(substr(n.closed, 1, 10) = ?, 0)");
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Collection(value) => {
            sql.push_str(
                "EXISTS (SELECT 1 FROM note_collections nc
                 JOIN collections c ON c.id = nc.collection_id
                 JOIN vaults v ON v.id = c.vault_id
                 WHERE nc.note_id = n.id AND LOWER(v.name || '/' || c.name) = ?)",
            );
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Link(value) => {
            sql.push_str(
                "EXISTS (SELECT 1 FROM note_links nl
                 WHERE nl.note_id = n.id AND LOWER(nl.target_id) = ?)",
            );
            parameters.push(value.as_str().to_string());
        }
        ListFilter::Not(filter) => {
            sql.push_str("NOT (");
            push_list_filter_sql(sql, parameters, filter);
            sql.push(')');
        }
    }
}

fn domain_from_row<T>(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<T>
where
    T: FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let value = row.get::<_, String>(index)?;
    value.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn optional_domain_from_row<T>(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<T>>
where
    T: FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    row.get::<_, Option<String>>(index)?
        .map(|value| {
            value.parse().map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use rusqlite::{Connection, params};
    use tempfile::tempdir;

    use crate::listing::ListField;
    use crate::query::Query;

    use super::{AGENDA_SQL, find_query_sql, list_query_sql};
    use crate::repository::{NoteMeta, Repository, schema::configure_and_initialize};

    #[test]
    fn agenda_query_filters_rows_in_sql_and_does_not_materialize_bodies() {
        let directory = tempdir().unwrap();
        let mut connection = Connection::open(directory.path().join("nt.sqlite3")).unwrap();
        configure_and_initialize(&mut connection).unwrap();
        let mut repository = Repository { connection };
        repository
            .create_vault("personal", &"2026-05-01T00:00:00Z".parse().unwrap())
            .unwrap();

        {
            let mut insert = |id: &str,
                              kind: &str,
                              status: Option<&str>,
                              scheduled: Option<&str>,
                              due: Option<&str>| {
                let mut note = NoteMeta::new_note(
                    id.parse().unwrap(),
                    "personal/inbox".parse().unwrap(),
                    "# Body that agenda must not load\n".to_string(),
                    "2026-05-01T00:00:00Z".parse().unwrap(),
                    "2026-05-01T00:00:00Z".parse().unwrap(),
                    id.to_string(),
                );
                note.kind = kind.parse().unwrap();
                note.status = status.map(|value| value.parse().unwrap());
                note.scheduled = scheduled.map(|value| value.parse().unwrap());
                note.due = due.map(|value| value.parse().unwrap());
                note.closed = status
                    .filter(|status| matches!(*status, "done" | "dropped"))
                    .map(|_| "2026-05-28T00:00:00Z".parse().unwrap());
                repository.insert_note(&note).unwrap();
            };

            insert(
                "018fbe0a-6c00-7000-8000-000000000001",
                "todo",
                Some("open"),
                None,
                Some("2026-05-27"),
            );
            insert(
                "018fbe0a-6c00-7000-8000-000000000002",
                "todo",
                Some("open"),
                Some("2026-05-28"),
                None,
            );
            insert(
                "018fbe0a-6c00-7000-8000-000000000003",
                "todo",
                Some("open"),
                None,
                Some("2026-06-03"),
            );
            insert(
                "018fbe0a-6c00-7000-8000-000000000004",
                "todo",
                Some("open"),
                None,
                Some("2026-06-04"),
            );
            insert(
                "018fbe0a-6c00-7000-8000-000000000005",
                "todo",
                Some("waiting"),
                None,
                Some("2026-05-27"),
            );
            insert(
                "018fbe0a-6c00-7000-8000-000000000006",
                "todo",
                Some("open"),
                None,
                None,
            );
            insert(
                "018fbe0a-6c00-7000-8000-000000000007",
                "todo",
                Some("done"),
                None,
                Some("2026-05-27"),
            );
            insert(
                "018fbe0a-6c00-7000-8000-000000000008",
                "todo",
                Some("dropped"),
                Some("2026-05-27"),
                None,
            );
            insert(
                "018fbe0a-6c00-7000-8000-000000000009",
                "note",
                None,
                None,
                None,
            );
        }

        repository
            .connection
            .execute(
                "UPDATE notes SET body = x'80' WHERE id = '018fbe0a-6c00-7000-8000-000000000001'",
                [],
            )
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET title = x'80'
                 WHERE id IN (
                     '018fbe0a-6c00-7000-8000-000000000004',
                     '018fbe0a-6c00-7000-8000-000000000005',
                     '018fbe0a-6c00-7000-8000-000000000006',
                     '018fbe0a-6c00-7000-8000-000000000007',
                     '018fbe0a-6c00-7000-8000-000000000008',
                     '018fbe0a-6c00-7000-8000-000000000009')",
                [],
            )
            .unwrap();

        let today = repository
            .agenda_notes(&"2026-05-28".parse().unwrap())
            .unwrap();
        assert_eq!(
            today
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "018fbe0a-6c00-7000-8000-000000000002",
                "018fbe0a-6c00-7000-8000-000000000001"
            ]
        );

        let week = repository
            .agenda_notes(&"2026-06-03".parse().unwrap())
            .unwrap();
        assert_eq!(
            week.iter().map(|note| note.id.as_str()).collect::<Vec<_>>(),
            vec![
                "018fbe0a-6c00-7000-8000-000000000003",
                "018fbe0a-6c00-7000-8000-000000000002",
                "018fbe0a-6c00-7000-8000-000000000001"
            ]
        );
    }

    #[test]
    fn agenda_plan_scans_only_open_todos_in_recency_order() {
        let repository = query_plan_repository(2_000);
        let plan = explain_plan(
            &repository.connection,
            AGENDA_SQL,
            &["2026-01-20".to_string()],
        );

        assert_plan_contains(&plan, "SCAN notes USING INDEX notes_open_todos_created");
        assert_plan_excludes(&plan, "notes_created");
        assert_plan_excludes(&plan, "USE TEMP B-TREE");
    }

    #[test]
    fn common_list_filters_use_targeted_indexes() {
        let repository = query_plan_repository(2_000);
        for (expression, index) in [
            ("id:01900001", "notes_id_nocase"),
            ("day:2026-01-15", "notes_created_day"),
            ("status:open", "notes_status_created"),
            ("tag:rust", "note_tags_lower_tag_note"),
        ] {
            let filters = Query::parse_list_filters(&[expression.to_string()]).unwrap();
            let (sql, parameters) = list_query_sql(&[ListField::Id], &filters);
            let plan = explain_plan(&repository.connection, &sql, &parameters);
            assert_plan_contains(&plan, index);
            assert_plan_excludes(&plan, "CORRELATED");
        }
    }

    #[test]
    fn structured_find_uses_scalar_index_and_bounded_relationship_probe() {
        let repository = query_plan_repository(2_000);
        let query = Query::parse(&[
            "status:open".to_string(),
            "collection:personal/inbox".to_string(),
        ])
        .unwrap();
        let (sql, parameters) = find_query_sql(&query);
        let plan = explain_plan(&repository.connection, &sql, &parameters);

        assert_plan_contains(&plan, "notes_status_created");
        assert_plan_contains(&plan, "CORRELATED SCALAR SUBQUERY");
        assert_plan_contains(&plan, "sqlite_autoindex_note_collections_1 (note_id=?)");
        assert_plan_contains(&plan, "sqlite_autoindex_collections_1 (id=?)");
        assert_plan_contains(&plan, "sqlite_autoindex_vaults_1 (id=?)");
    }

    #[test]
    fn fts_find_builds_match_set_before_note_lookup() {
        let repository = query_plan_repository(2_000);
        let query = Query::parse(&["title:quasar".to_string()]).unwrap();
        let (sql, parameters) = find_query_sql(&query);
        let plan = explain_plan(&repository.connection, &sql, &parameters);

        assert_plan_contains(&plan, "LIST SUBQUERY");
        assert_plan_contains(&plan, "note_fts VIRTUAL TABLE INDEX");
        assert_plan_contains(&plan, "search_rows USING INTEGER PRIMARY KEY (rowid=?)");
        assert_plan_excludes(&plan, "CORRELATED");
    }

    #[test]
    fn unused_raw_tag_index_is_not_created() {
        let repository = query_plan_repository(1);
        let index_exists: bool = repository
            .connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_schema
                     WHERE type = 'index' AND name = 'note_tags_tag'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!index_exists);
    }

    #[test]
    #[ignore = "manual microbenchmark with 10,000 realistic notes"]
    fn query_workload_benchmark() {
        let repository = query_plan_repository(10_000);
        let workloads = [
            ("agenda", benchmark_agenda as fn(&Repository) -> usize),
            ("list-status", benchmark_list_status),
            ("list-tag", benchmark_list_tag),
            ("find-structured", benchmark_find_structured),
            ("find-fts", benchmark_find_fts),
        ];

        for (name, workload) in workloads {
            let start = Instant::now();
            let mut rows = 0;
            for _ in 0..20 {
                rows = workload(&repository);
            }
            eprintln!("{name}: {:?} total, {rows} rows/run", start.elapsed());
        }
    }

    fn query_plan_repository(note_count: usize) -> Repository {
        let mut connection = Connection::open_in_memory().unwrap();
        configure_and_initialize(&mut connection).unwrap();
        let mut repository = Repository { connection };
        repository
            .create_vault("personal", &"2026-01-01T00:00:00Z".parse().unwrap())
            .unwrap();
        for index in 0..20 {
            repository
                .create_vault(
                    &format!("fixture{index}"),
                    &"2026-01-01T00:00:00Z".parse().unwrap(),
                )
                .unwrap();
        }
        let collection_id: String = repository
            .connection
            .query_row(
                "SELECT c.id FROM collections c
                 JOIN vaults v ON v.id = c.vault_id
                 WHERE v.name = 'personal' AND c.name = 'inbox'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let transaction = repository.connection.transaction().unwrap();
        for index in 0..note_count {
            let id = format!("019{index:05x}-0000-7000-8000-{index:012x}");
            let day = index % 28 + 1;
            let created = format!("2026-01-{day:02}T12:00:00Z");
            let is_todo = index % 4 == 0;
            let status = is_todo.then_some(if index % 20 == 0 { "waiting" } else { "open" });
            let scheduled = (index % 12 == 0).then(|| format!("2026-01-{day:02}"));
            let due = (index % 8 == 0).then(|| format!("2026-01-{day:02}"));
            let title = if index % 997 == 0 {
                format!("Rare quasar note {index}")
            } else {
                format!("Ordinary project note {index}")
            };
            let body = if index % 997 == 0 {
                "A selective quasar marker"
            } else {
                "Common project material"
            };
            transaction
                .execute(
                    "INSERT INTO notes
                     (id, home_collection_id, body, created, updated, title, kind,
                      status, priority, scheduled, due, closed)
                     VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
                    params![
                        id,
                        collection_id,
                        body,
                        created,
                        title,
                        if is_todo { "todo" } else { "note" },
                        status,
                        is_todo.then_some("B"),
                        scheduled,
                        due,
                    ],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO note_collections (note_id, collection_id) VALUES (?1, ?2)",
                    params![id, collection_id],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO note_tags (note_id, tag) VALUES (?1, ?2)",
                    params![id, if index % 10 == 0 { "rust" } else { "general" }],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO note_tags (note_id, tag) VALUES (?1, ?2)",
                    params![id, format!("project-{}", index % 100)],
                )
                .unwrap();
        }
        transaction.commit().unwrap();
        repository
    }

    fn explain_plan(connection: &Connection, sql: &str, parameters: &[String]) -> Vec<String> {
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        statement
            .query_map(rusqlite::params_from_iter(parameters), |row| row.get(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    fn assert_plan_contains(plan: &[String], expected: &str) {
        assert!(
            plan.iter().any(|step| step.contains(expected)),
            "plan does not contain {expected:?}:\n{}",
            plan.join("\n")
        );
    }

    fn assert_plan_excludes(plan: &[String], unexpected: &str) {
        assert!(
            plan.iter().all(|step| !step.contains(unexpected)),
            "plan contains {unexpected:?}:\n{}",
            plan.join("\n")
        );
    }

    fn benchmark_agenda(repository: &Repository) -> usize {
        repository
            .agenda_notes(&"2026-01-20".parse().unwrap())
            .unwrap()
            .len()
    }

    fn benchmark_list_status(repository: &Repository) -> usize {
        let filters = Query::parse_list_filters(&["status:open".to_string()]).unwrap();
        repository
            .list_rows(&[ListField::Id], &filters)
            .unwrap()
            .len()
    }

    fn benchmark_list_tag(repository: &Repository) -> usize {
        let filters = Query::parse_list_filters(&["tag:rust".to_string()]).unwrap();
        repository
            .list_rows(&[ListField::Id], &filters)
            .unwrap()
            .len()
    }

    fn benchmark_find_structured(repository: &Repository) -> usize {
        let query = Query::parse(&[
            "status:open".to_string(),
            "tag:project-4".to_string(),
            "since:2026-01-10".to_string(),
        ])
        .unwrap();
        repository.find_rows(&query).unwrap().len()
    }

    fn benchmark_find_fts(repository: &Repository) -> usize {
        let query = Query::parse(&["title:quasar".to_string()]).unwrap();
        repository.find_rows(&query).unwrap().len()
    }
}
