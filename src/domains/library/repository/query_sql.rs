use rusqlite::types::Value;

use super::super::{LibraryFilter, LibraryQuery};

pub(super) fn compile_query(query: &LibraryQuery) -> (String, Vec<Value>) {
    let mut parameters = Vec::new();
    let mut expressions = query
        .filters()
        .iter()
        .map(|filter| compile_filter(filter, &mut parameters))
        .collect::<Vec<_>>();
    let fts_query = query
        .lexical_terms()
        .iter()
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" AND ");
    let parameter = push_parameter(&mut parameters, &fts_query);
    expressions.push(format!(
        "c.pk IN (SELECT rowid FROM library_capture_fts
         WHERE library_capture_fts MATCH ?{parameter})"
    ));
    (format!("WHERE {}", expressions.join(" AND ")), parameters)
}

fn compile_filter(filter: &LibraryFilter, parameters: &mut Vec<Value>) -> String {
    match filter {
        LibraryFilter::IdPrefix(prefix) => {
            let lower = push_parameter(parameters, prefix);
            let upper = push_parameter(parameters, &prefix_upper_bound(prefix));
            format!("i.id >= ?{lower} AND i.id < ?{upper}")
        }
        LibraryFilter::Source(source) => {
            let parameter = push_parameter(parameters, source);
            format!("i.source = ?{parameter}")
        }
        LibraryFilter::Title(title) => {
            let parameter = push_parameter(parameters, title);
            format!("instr(lower(i.title), lower(?{parameter})) > 0")
        }
        LibraryFilter::CapturedSince(timestamp) => {
            let parameter = push_parameter(parameters, timestamp.as_str());
            format!("c.captured >= ?{parameter}")
        }
        LibraryFilter::CapturedBefore(timestamp) => {
            let parameter = push_parameter(parameters, timestamp.as_str());
            format!("c.captured < ?{parameter}")
        }
    }
}

fn prefix_upper_bound(prefix: &str) -> String {
    let mut upper = prefix.as_bytes().to_vec();
    *upper.last_mut().expect("validated prefixes are nonempty") += 1;
    String::from_utf8(upper).expect("validated prefixes are ASCII")
}

fn push_parameter(parameters: &mut Vec<Value>, value: &str) -> usize {
    parameters.push(Value::Text(value.to_string()));
    parameters.len()
}
