use crate::error::{NtError, Result};

pub(super) fn normalize_body(body: &str) -> Result<(String, String)> {
    if body.is_empty() {
        return Err(NtError::EmptyBody);
    }

    let body = body.replace("\r\n", "\n").replace('\r', "\n");
    if body.is_empty() {
        return Err(NtError::EmptyBody);
    }

    let first_line = body
        .split_once('\n')
        .map_or(body.as_str(), |(line, _)| line);
    let title = first_line
        .strip_prefix("# ")
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .ok_or(NtError::InvalidTitle)?;

    Ok((body.clone(), title.to_string()))
}

#[cfg(test)]
mod tests {
    use crate::error::NtError;

    use super::normalize_body;

    #[test]
    fn normalizes_line_endings_and_derives_title() {
        let (body, title) = normalize_body("#  Storage  \r\n\rDetails\r\n").unwrap();
        assert_eq!(body, "#  Storage  \n\nDetails\n");
        assert_eq!(title, "Storage");
    }

    #[test]
    fn preserves_content_other_than_line_endings() {
        let (body, _) = normalize_body("# Title\n\nbody  ").unwrap();
        assert_eq!(body, "# Title\n\nbody  ");
    }

    #[test]
    fn requires_a_leading_nonempty_atx_h1() {
        assert!(matches!(normalize_body(""), Err(NtError::EmptyBody)));
        for body in ["\n# Later", "body\n# Later", " ## Title", "#", "#   "] {
            assert!(matches!(normalize_body(body), Err(NtError::InvalidTitle)));
        }
    }
}
