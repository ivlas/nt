use super::{ListField, ListRow};

pub fn render_row(row: &ListRow) -> String {
    row.values.join("\t")
}

pub fn render_table(rows: &[ListRow], fields: &[ListField]) -> Vec<String> {
    let headers = fields
        .iter()
        .map(|field| field.name().to_ascii_uppercase())
        .collect::<Vec<_>>();
    let rows = rows
        .iter()
        .map(|row| row.values.clone())
        .collect::<Vec<_>>();
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
    use super::render_table;
    use crate::listing::{ListField, ListRow};

    #[test]
    fn table_has_headers_and_aligned_columns() {
        let short = ListRow {
            values: vec![
                "018fbe0a-6c00-7000-8000-000000000001".to_string(),
                "Short".to_string(),
                "open".to_string(),
            ],
        };
        let long = ListRow {
            values: vec![
                "018fbe0a-6c00-7000-8000-000000000002".to_string(),
                "A much longer title".to_string(),
                "-".to_string(),
            ],
        };

        let lines = render_table(
            &[short, long],
            &[ListField::Id, ListField::Title, ListField::Status],
        );

        assert_eq!(
            lines[0],
            "ID                                    TITLE                STATUS"
        );
        assert_eq!(
            lines[1],
            "018fbe0a-6c00-7000-8000-000000000001  Short                open"
        );
        assert_eq!(
            lines[2],
            "018fbe0a-6c00-7000-8000-000000000002  A much longer title  -"
        );
    }
}
