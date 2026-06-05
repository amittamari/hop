//! Transcript preview rendering: code highlighting (syntect), prose markdown
//! (pulldown-cmark), and assembling messages into scrollable, match-highlighted
//! lines.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
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

/// Render a prose (non-code) markdown string into styled lines. Handles
/// headings (bold), strong/emphasis, inline code, and list items.
pub fn render_prose(text: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut bold = false;
    let mut italic = false;
    let mut in_item = false;

    let flush = |spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>| {
        lines.push(Line::from(std::mem::take(spans)));
    };

    for ev in Parser::new(text) {
        match ev {
            Event::Start(Tag::Heading { .. }) => bold = true,
            Event::End(TagEnd::Heading(_)) => {
                bold = false;
                flush(&mut spans, &mut lines);
            }
            Event::Start(Tag::Strong) => bold = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Emphasis) => italic = false,
            Event::Start(Tag::Item) => {
                in_item = true;
                spans.push(Span::raw("• "));
            }
            Event::End(TagEnd::Item) => {
                in_item = false;
                flush(&mut spans, &mut lines);
            }
            Event::End(TagEnd::Paragraph) => flush(&mut spans, &mut lines),
            Event::SoftBreak | Event::HardBreak => {
                if in_item {
                    spans.push(Span::raw(" "));
                } else {
                    flush(&mut spans, &mut lines);
                }
            }
            Event::Code(t) => {
                spans.push(Span::styled(t.to_string(), Style::default().fg(Color::Yellow)));
            }
            Event::Text(t) => {
                let mut style = Style::default();
                if bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                spans.push(Span::styled(t.to_string(), style));
            }
            _ => {}
        }
    }
    if !spans.is_empty() {
        flush(&mut spans, &mut lines);
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_plain_text_one_line() {
        let lines = render_prose("hello world");
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.trim(), "hello world");
    }

    #[test]
    fn prose_bullets_get_marker() {
        let lines = render_prose("- one\n- two");
        let joined: String = lines.iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("• one"));
        assert!(joined.contains("• two"));
    }

    #[test]
    fn prose_bold_is_styled_bold() {
        let lines = render_prose("**strong**");
        let bold = lines.iter().flat_map(|l| &l.spans)
            .any(|s| s.content.contains("strong") && s.style.add_modifier.contains(ratatui::style::Modifier::BOLD));
        assert!(bold);
    }

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
