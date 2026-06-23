use crate::error::{NtError, Result};

pub fn title_from_body(body: &str) -> Result<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let title = trimmed
            .strip_prefix("# ")
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .ok_or(NtError::InvalidTitle)?;
        return Ok(title.to_string());
    }

    Err(NtError::InvalidTitle)
}

pub fn sources_from_body(body: &str) -> Vec<String> {
    let mut sources = Vec::new();
    let mut cursor = 0;

    while cursor < body.len() {
        let Some(offset) = next_url_offset(&body[cursor..]) else {
            break;
        };
        let start = cursor + offset;
        let end = body[start..]
            .char_indices()
            .find_map(|(index, ch)| url_terminator(ch).then_some(start + index))
            .unwrap_or(body.len());
        let source = body[start..end].trim_end_matches(trailing_url_punctuation);
        if !source.is_empty() && !sources.iter().any(|value| value == source) {
            sources.push(source.to_string());
        }
        cursor = end.max(start + 1);
    }

    sources.sort();
    sources
}

fn next_url_offset(text: &str) -> Option<usize> {
    match (text.find("http://"), text.find("https://")) {
        (Some(http), Some(https)) => Some(http.min(https)),
        (Some(http), None) => Some(http),
        (None, Some(https)) => Some(https),
        (None, None) => None,
    }
}

fn url_terminator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, ')' | ']' | '>' | '"' | '\'')
}

fn trailing_url_punctuation(ch: char) -> bool {
    matches!(ch, '.' | ',' | ':' | ';' | '!' | '?')
}

#[cfg(test)]
mod tests {
    use super::{sources_from_body, title_from_body};

    #[test]
    fn extracts_title_from_markdown_heading() {
        assert_eq!(title_from_body("\n# Hello\nbody").unwrap(), "Hello");
    }

    #[test]
    fn requires_h1_title_as_first_non_empty_line() {
        assert!(title_from_body("body\n# Later").is_err());
        assert!(title_from_body("## Section\nbody").is_err());
        assert!(title_from_body("#\nbody").is_err());
        assert!(title_from_body("#   \nbody").is_err());
    }

    #[test]
    fn extracts_http_sources_from_markdown_body() {
        let body = "# Links\n\n[Spec](https://example.com/spec), <http://example.com/a>.\n";

        assert_eq!(
            sources_from_body(body),
            vec![
                "http://example.com/a".to_string(),
                "https://example.com/spec".to_string()
            ]
        );
    }
}
