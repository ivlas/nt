use std::collections::BTreeSet;

pub(super) fn tokenize_text(text: &str) -> BTreeSet<String> {
    let mut terms = BTreeSet::new();
    let mut term = String::new();

    for char in text.chars() {
        if char.is_alphanumeric() {
            term.push(char.to_ascii_lowercase());
        } else if is_combining_mark(char) && !term.is_empty() {
            term.push(char);
        } else if !term.is_empty() {
            terms.insert(std::mem::take(&mut term));
        }
    }

    if !term.is_empty() {
        terms.insert(term);
    }

    terms
}

fn is_combining_mark(char: char) -> bool {
    matches!(
        char,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe20}'..='\u{fe2f}'
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::tokenize_text;

    #[test]
    fn tokenizes_text_to_lowercase_unique_terms() {
        assert_eq!(
            tokenize_text("QEMU, qemu; Firecracker/v1"),
            BTreeSet::from([
                "firecracker".to_string(),
                "qemu".to_string(),
                "v1".to_string()
            ])
        );
        assert_eq!(
            tokenize_text("RE\u{301}SUME\u{301}"),
            BTreeSet::from(["re\u{301}sume\u{301}".to_string()])
        );
    }
}
