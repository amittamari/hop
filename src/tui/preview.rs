//! Transcript preview rendering: code highlighting (syntect), prose markdown
//! (pulldown-cmark), and assembling messages into scrollable, match-highlighted
//! lines.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEMES: OnceLock<ThemeSet> = OnceLock::new();

fn map_lang(l: &str) -> &str {
    match l {
        "js" => "javascript",
        "ts" => "typescript",
        "py" => "python",
        "rb" => "ruby",
        "sh" => "bash",
        "yml" => "yaml",
        "rs" => "rust",
        other => other,
    }
}

/// Highlight a code block into indented ratatui lines. Lazily loads syntect's
/// default assets on first use; safe to call from the render path (memoize at
/// the call site per selection).
pub fn highlight_code(code: &str, lang: Option<&str>) -> Vec<Line<'static>> {
    let ps = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let ts = THEMES.get_or_init(ThemeSet::load_defaults);
    let theme = &ts.themes["base16-ocean.dark"];
    let syntax = lang
        .map(map_lang)
        .and_then(|l| ps.find_syntax_by_token(l))
        .unwrap_or_else(|| ps.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, theme);
    let mut out = Vec::new();
    for line in code.lines() {
        let ranges = h.highlight_line(line, ps).unwrap_or_default();
        let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
        for (style, text) in ranges {
            let c = style.foreground;
            spans.push(Span::styled(
                text.to_string(),
                Style::default().fg(Color::Rgb(c.r, c.g, c.b)),
            ));
        }
        out.push(Line::from(spans));
    }
    if out.is_empty() {
        out.push(Line::from(Span::raw("  ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_into_indented_lines() {
        let lines = highlight_code("fn main() {}", Some("rust"));
        assert_eq!(lines.len(), 1);
        // first span is the 2-space indent
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.starts_with("  "));
        assert!(text.contains("fn main"));
    }

    #[test]
    fn unknown_lang_falls_back_to_plain() {
        let lines = highlight_code("x = 1", Some("nope-lang"));
        assert_eq!(lines.len(), 1);
    }
}
