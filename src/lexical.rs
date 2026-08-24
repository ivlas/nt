pub(crate) fn literal_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (index, character) in value.char_indices() {
        if character.is_alphanumeric() {
            start.get_or_insert(index);
        } else if let Some(start) = start.take() {
            tokens.push(value[start..index].to_string());
        }
    }
    if let Some(start) = start {
        tokens.push(value[start..].to_string());
    }
    tokens
}

pub(crate) fn normalized_terms(values: &[String]) -> Vec<String> {
    let mut terms = values
        .iter()
        .flat_map(|value| literal_tokens(value))
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    terms
}

pub(crate) fn fts_and_expression(terms: &[String]) -> String {
    terms
        .iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

#[cfg(test)]
mod tests {
    use super::{fts_and_expression, literal_tokens, normalized_terms};
    use crate::note::NoteQuery;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn assert_note_terms(values: &[&str], expected: &[&str]) {
        let values = strings(values);
        let note = NoteQuery::parse_find(&values).unwrap();
        assert_eq!(note.lexical_terms(), expected);
    }

    #[test]
    fn note_queries_use_literal_term_contracts() {
        assert_note_terms(&["beta alpha"], &["alpha", "beta"]);
        assert_note_terms(&["zebra café"], &["café", "zebra"]);
        assert_note_terms(&["beta alpha", "beta"], &["alpha", "beta"]);
        assert_note_terms(
            &["ownership-borrow.alpha"],
            &["alpha", "borrow", "ownership"],
        );
        assert_note_terms(&["*** alpha", "---"], &["alpha"]);
        assert!(normalized_terms(&strings(&["***", "---"])).is_empty());
    }

    #[test]
    fn fts_construction_quotes_escapes_and_joins_literals() {
        assert_eq!(fts_and_expression(&[]), "");
        assert_eq!(
            fts_and_expression(&strings(&["a\"b", "c"])),
            "\"a\"\"b\" AND \"c\""
        );
    }

    #[test]
    fn precomposed_characters_remain_in_tokens() {
        assert_eq!(literal_tokens("café"), ["café"]);
    }

    #[test]
    fn combining_marks_split_tokens() {
        assert_eq!(literal_tokens("cafe\u{301}"), ["cafe"]);
    }

    #[test]
    fn non_latin_scripts_are_alphanumeric_tokens() {
        assert_eq!(
            normalized_terms(&strings(&["東京 Привет"])),
            ["Привет", "東京"]
        );
    }

    #[test]
    fn adjacent_mixed_scripts_remain_one_token() {
        assert_eq!(literal_tokens("rust東京"), ["rust東京"]);
    }

    #[test]
    fn emoji_separate_adjacent_words() {
        assert_eq!(literal_tokens("hello🙂世界"), ["hello", "世界"]);
    }
}
