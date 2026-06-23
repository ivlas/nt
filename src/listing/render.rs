use crate::index::NoteMeta;

use super::field::ListField;

pub fn render_row(note: &NoteMeta, fields: &[ListField]) -> String {
    fields
        .iter()
        .map(|field| field.render(note))
        .collect::<Vec<_>>()
        .join("\t")
}

pub fn render_table(notes: &[&NoteMeta], fields: &[ListField]) -> Vec<String> {
    let headers = fields
        .iter()
        .map(|field| field.name().to_ascii_uppercase())
        .collect::<Vec<_>>();
    let rows = notes
        .iter()
        .map(|note| {
            fields
                .iter()
                .map(|field| field.render(note))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    render_columns(headers, rows)
}

pub fn render_link_row(from: &NoteMeta, to: &NoteMeta) -> String {
    [&from.id, &from.title, &to.id, &to.title]
        .map(String::as_str)
        .join("\t")
}

pub fn render_link_table(links: &[(&NoteMeta, &NoteMeta)]) -> Vec<String> {
    let headers = ["FROM ID", "FROM TITLE", "TO ID", "TO TITLE"]
        .map(str::to_string)
        .to_vec();
    let rows = links
        .iter()
        .map(|(from, to)| {
            vec![
                from.id.clone(),
                from.title.clone(),
                to.id.clone(),
                to.title.clone(),
            ]
        })
        .collect();

    render_columns(headers, rows)
}

fn render_columns(headers: Vec<String>, rows: Vec<Vec<String>>) -> Vec<String> {
    let widths = headers
        .iter()
        .enumerate()
        .map(|(column, _)| {
            rows.iter()
                .map(|row| row[column].chars().count())
                .chain([headers[column].len()])
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();

    std::iter::once(format_columns(headers.iter().cloned(), &widths))
        .chain(
            rows.into_iter()
                .map(|row| format_columns(row.into_iter(), &widths)),
        )
        .collect()
}

fn format_columns(values: impl Iterator<Item = String>, widths: &[usize]) -> String {
    let last = widths.len().saturating_sub(1);
    values
        .enumerate()
        .map(|(column, value)| {
            if column == last {
                value
            } else {
                let padding = widths[column].saturating_sub(value.chars().count()) + 2;
                format!("{value}{}", " ".repeat(padding))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::index::NoteMeta;

    use super::render_table;
    use crate::listing::ListField;

    #[test]
    fn table_has_headers_and_aligned_columns() {
        let mut short = NoteMeta::new_note(
            "NT20260621T100000".to_string(),
            PathBuf::from("NT20260621T100000.md"),
            "2026-06-21T10:00:00Z".to_string(),
            "2026-06-21T10:00:00Z".to_string(),
            "Short".to_string(),
        );
        short.status = Some("open".to_string());
        let long = NoteMeta::new_note(
            "NT20260621T110000".to_string(),
            PathBuf::from("NT20260621T110000.md"),
            "2026-06-21T11:00:00Z".to_string(),
            "2026-06-21T11:00:00Z".to_string(),
            "A much longer title".to_string(),
        );

        let lines = render_table(
            &[&short, &long],
            &[ListField::Id, ListField::Title, ListField::Status],
        );

        assert_eq!(lines[0], "ID                 TITLE                STATUS");
        assert_eq!(lines[1], "NT20260621T100000  Short                open");
        assert_eq!(lines[2], "NT20260621T110000  A much longer title  -");
    }
}
