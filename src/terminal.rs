use std::io::{self, IsTerminal};

#[derive(Clone, Copy)]
pub enum Style {
    BrightCyan,
    Dim,
    Green,
    Red,
}

pub fn stdout_color_enabled() -> bool {
    color_enabled(
        io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
        std::env::var("TERM").ok().as_deref(),
    )
}

pub fn stderr_color_enabled() -> bool {
    color_enabled(
        io::stderr().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
        std::env::var("TERM").ok().as_deref(),
    )
}

pub fn color_enabled(is_tty: bool, no_color: bool, term: Option<&str>) -> bool {
    is_tty && !no_color && term != Some("dumb")
}

pub fn paint(text: &str, style: Style, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }

    let code = match style {
        Style::BrightCyan => "96",
        Style::Dim => "2",
        Style::Green => "32",
        Style::Red => "31",
    };

    format!("\x1b[{code}m{text}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::color_enabled;

    #[test]
    fn color_requires_tty() {
        assert!(color_enabled(true, false, Some("xterm-256color")));
        assert!(!color_enabled(false, false, Some("xterm-256color")));
    }

    #[test]
    fn color_respects_no_color_and_dumb_term() {
        assert!(!color_enabled(true, true, Some("xterm-256color")));
        assert!(!color_enabled(true, false, Some("dumb")));
        assert!(color_enabled(true, false, None));
    }
}
