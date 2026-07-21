//! Transcript preview rendering: code highlighting (syntect), prose markdown
//! (pulldown-cmark), and assembling messages into scrollable, match-highlighted
//! lines.
//
// File-size note: production code is ~475 lines; colocated #[cfg(test)] tests
// push the file over the ~500-line soft limit. Kept together intentionally.

use crate::core::{AgentId, Block, Message, Role, SessionSummary, Transcript};
use crate::tui::glyphs::Glyphs;
use crate::tui::theme::Theme;
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
    // Intentional RGB island: syntect owns these foreground colors; they are
    // deliberately NOT mapped to the semantic Theme roles.
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
pub fn render_prose(text: &str, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut bold = false;
    let mut italic = false;
    let mut in_item = false;
    let mut list_depth = 0usize;
    let mut item_line_start = false;

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
            Event::Start(Tag::List(_)) => {
                if in_item && !spans.is_empty() {
                    flush(&mut spans, &mut lines);
                }
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
            }
            Event::Start(Tag::Item) => {
                in_item = true;
                item_line_start = false;
                spans.push(Span::raw(format!("{}• ", "  ".repeat(list_depth.saturating_sub(1)))));
            }
            Event::End(TagEnd::Item) => {
                in_item = false;
                item_line_start = false;
                flush(&mut spans, &mut lines);
            }
            Event::End(TagEnd::Paragraph) => flush(&mut spans, &mut lines),
            Event::SoftBreak | Event::HardBreak => {
                if in_item {
                    flush(&mut spans, &mut lines);
                    spans.push(Span::raw("  ".repeat(list_depth)));
                    item_line_start = true;
                } else {
                    flush(&mut spans, &mut lines);
                }
            }
            Event::Code(t) => {
                spans.push(Span::styled(t.to_string(), Style::default().fg(theme.code)));
            }
            Event::Text(t) => {
                let mut text = t.to_string();
                if item_line_start {
                    item_line_start = false;
                    let trimmed = text.trim_start();
                    if let Some(rest) =
                        trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* "))
                    {
                        spans.push(Span::raw("• "));
                        text = rest.to_string();
                    }
                }
                let mut style = Style::default();
                if bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                spans.push(Span::styled(text, style));
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

/// Render a transcript into styled lines with thin-rule message separators
/// (`── user ──────` / `── claude ──────`) and query-term highlighting.
pub fn render_transcript(
    msgs: &[Message],
    terms: &[String],
    agent: AgentId,
    theme: &Theme,
    glyphs: &Glyphs,
    width: u16,
) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();

    for m in msgs {
        out.push(thin_rule(m.role, agent, glyphs, width, theme));
        for b in &m.blocks {
            match b {
                Block::Prose(s) => {
                    out.extend(render_prose(s, theme));
                }
                Block::Code { lang, text } => {
                    out.extend(highlight_code(text, lang.as_deref()));
                }
            }
        }
        out.push(Line::from(""));
    }
    if !terms.is_empty() {
        for line in &mut out {
            *line = highlight_terms(line, terms, theme);
        }
    }
    out
}

/// Build a thin rule line: `── role ────────────────`
fn thin_rule(
    role: Role,
    agent: AgentId,
    glyphs: &Glyphs,
    width: u16,
    theme: &Theme,
) -> Line<'static> {
    let label = match role {
        Role::User => "user".to_string(),
        Role::Agent => {
            let badge = agent.badge().to_lowercase();
            let glyph = glyphs.agent(agent);
            if glyph.is_empty() { badge } else { format!("{glyph} {badge}") }
        }
    };
    let prefix = "── ";
    let suffix = " ";
    let dw = crate::tui::columns::display_width;
    let used = dw(prefix) + dw(&label) + dw(suffix);
    let fill = (width as usize).saturating_sub(used);
    let rule_char = "─";
    let (rule_style, label_style) = match role {
        Role::User => (
            Style::default().fg(theme.border),
            Style::default().fg(theme.preview_text).add_modifier(Modifier::BOLD),
        ),
        Role::Agent => {
            let style = Style::default().fg(theme.agent_color(agent));
            (style, style.add_modifier(Modifier::BOLD))
        }
    };
    Line::from(vec![
        Span::styled(prefix, rule_style),
        Span::styled(label, label_style),
        Span::styled(suffix, rule_style),
        Span::styled(rule_char.repeat(fill), rule_style),
    ])
}

pub fn render_indexed_fallback(content: &str, query: &str, theme: &Theme) -> Vec<Line<'static>> {
    let parsed = crate::query::parse(query);
    render_indexed_fallback_with_terms(content, &parsed.free_terms(), theme)
}

pub fn render_indexed_fallback_with_terms(
    content: &str,
    terms: &[String],
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut out = vec![
        Line::from(Span::styled(
            "source unavailable - showing indexed text",
            Style::default().fg(theme.muted),
        )),
        Line::from(""),
    ];
    let mut body = render_prose(content, theme);
    if !terms.is_empty() {
        for line in &mut body {
            *line = highlight_terms(line, terms, theme);
        }
    }
    out.extend(body);
    out
}

/// Highlight query terms inside a line. Term matches use `Modifier::REVERSED`
/// (a glyph-level invert), which is intentionally DIFFERENT from the list
/// selection's full-row background swap (`theme.selection_bg`): inline term
/// hits should pop without repainting the whole row's background.
/// `theme.match_fg` is reserved to unify these two affordances later; for now
/// we keep REVERSED and accept the theme only to wire the call chain.
pub fn highlight_terms(line: &Line<'static>, terms: &[String], _theme: &Theme) -> Line<'static> {
    let mut out: Vec<Span<'static>> = Vec::new();
    for span in &line.spans {
        let text = span.content.to_string();
        let lower = text.to_lowercase();
        let mut idx = 0usize;
        while idx < text.len() {
            // find the earliest term match at or after idx
            let next = terms
                .iter()
                .filter_map(|t| lower[idx..].find(t.as_str()).map(|p| (idx + p, t.len())))
                .min_by_key(|&(p, _)| p);
            match next {
                Some((p, len)) if text.is_char_boundary(p) && text.is_char_boundary(p + len) => {
                    if p > idx {
                        out.push(Span::styled(text[idx..p].to_string(), span.style));
                    }
                    out.push(Span::styled(
                        text[p..p + len].to_string(),
                        span.style.add_modifier(Modifier::REVERSED),
                    ));
                    idx = p + len;
                }
                // No match, or a boundary that isn't valid in the original (rare
                // multi-byte lowercasing): emit the remainder unstyled, no panic.
                _ => {
                    out.push(Span::styled(text[idx..].to_string(), span.style));
                    break;
                }
            }
        }
        if text.is_empty() {
            out.push(span.clone());
        }
    }
    Line::from(out)
}

/// Index of the first line containing any term (case-insensitive); for scroll.
pub fn first_match_line(lines: &[Line<'static>], query: &str) -> Option<usize> {
    let parsed = crate::query::parse(query);
    first_match_line_with_terms(lines, &parsed.free_terms())
}

pub fn first_match_line_with_terms(lines: &[Line<'static>], terms: &[String]) -> Option<usize> {
    match_lines(lines, terms).into_iter().next()
}

pub fn match_lines(lines: &[Line<'static>], terms: &[String]) -> Vec<usize> {
    if terms.is_empty() {
        return Vec::new();
    }
    lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            let text: String =
                l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().to_lowercase();
            terms.iter().any(|t| text.contains(t.as_str())).then_some(i)
        })
        .collect()
}

#[derive(Default)]
pub struct PreviewState {
    transcript: Vec<Message>,
    transcript_for: Option<String>,
    key: Option<(String, String)>,
    source_unavailable: bool,
    pub lines: Vec<Line<'static>>,
}

impl PreviewState {
    pub fn source_unavailable(&self) -> bool {
        self.source_unavailable
    }

    pub fn invalidate(&mut self) {
        self.transcript_for = None;
        self.key = None;
        self.transcript = Vec::new();
        self.lines = Vec::new();
        self.source_unavailable = false;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        app: &mut crate::tui::App,
        selected: Option<&SessionSummary>,
        terms: &[String],
        load_transcript: impl FnOnce(&SessionSummary) -> Option<Transcript>,
        load_indexed_content: impl FnOnce(&SessionSummary) -> Option<String>,
        preview_width: u16,
    ) {
        let sel_key = selected.map(|s| s.document_key());
        if app.preview_visible() && sel_key != self.transcript_for {
            match selected {
                Some(session) => match load_transcript(session) {
                    Some(transcript) => {
                        self.transcript = transcript.messages;
                        self.source_unavailable = false;
                    }
                    None => {
                        self.transcript = Vec::new();
                        self.source_unavailable = true;
                    }
                },
                None => {
                    self.transcript = Vec::new();
                    self.source_unavailable = false;
                }
            }
            self.transcript_for = sel_key.clone();
        }

        let theme = *app.theme();
        let glyphs = app.glyphs().clone();
        let preview_key = (sel_key.unwrap_or_default(), app.query().to_string());
        if app.preview_visible() && self.key.as_ref() != Some(&preview_key) {
            self.lines = if self.source_unavailable {
                selected
                    .and_then(load_indexed_content)
                    .map(|content| render_indexed_fallback_with_terms(&content, terms, &theme))
                    .unwrap_or_default()
            } else {
                let agent = selected.map(|s| s.agent).unwrap_or(AgentId::Claude);
                render_transcript(&self.transcript, terms, agent, &theme, &glyphs, preview_width)
            };
            app.set_preview_matches(match_lines(&self.lines, terms));
            self.key = Some(preview_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Block, Message, Role};

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn msgs() -> Vec<Message> {
        vec![
            Message { role: Role::User, blocks: vec![Block::Prose("fix the auth bug".into())] },
            Message {
                role: Role::Agent,
                blocks: vec![
                    Block::Prose("the refresh token dropped".into()),
                    Block::Code { lang: Some("rust".into()), text: "fn refresh() {}".into() },
                ],
            },
        ]
    }

    #[test]
    fn transcript_has_role_separators() {
        let theme = crate::tui::theme::Theme::default();
        let lines = render_transcript(
            &msgs(),
            &[],
            crate::core::AgentId::Claude,
            &theme,
            &Glyphs::ascii(),
            80,
        );
        let joined: String = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("── user"), "user separator should be present");
        assert!(joined.contains("── claude"), "agent separator should be present");
        assert!(joined.contains("fix the auth bug"));
        assert!(joined.contains("fn refresh"));
    }

    #[test]
    fn first_match_line_is_found() {
        let theme = crate::tui::theme::Theme::default();
        let terms = vec!["refresh".into()];
        let lines = render_transcript(
            &msgs(),
            &terms,
            crate::core::AgentId::Claude,
            &theme,
            &Glyphs::ascii(),
            80,
        );
        let idx = first_match_line(&lines, "refresh");
        assert!(idx.is_some());
    }

    #[test]
    fn filter_tokens_are_not_match_terms() {
        let theme = crate::tui::theme::Theme::default();
        let terms = crate::query::parse("agent:claude").free_terms();
        let lines = render_transcript(
            &msgs(),
            &terms,
            crate::core::AgentId::Claude,
            &theme,
            &Glyphs::ascii(),
            80,
        );
        assert_eq!(first_match_line(&lines, "agent:claude"), None);
    }

    #[test]
    fn match_terms_highlighted() {
        let theme = crate::tui::theme::Theme::default();
        let terms = vec!["auth".into()];
        let lines = render_transcript(
            &msgs(),
            &terms,
            crate::core::AgentId::Claude,
            &theme,
            &Glyphs::ascii(),
            80,
        );
        let any_reverse = lines.iter().flat_map(|l| &l.spans).any(|s| {
            s.content.contains("auth")
                && s.style.add_modifier.contains(ratatui::style::Modifier::REVERSED)
        });
        assert!(any_reverse);
    }

    #[test]
    fn indexed_fallback_explains_missing_source_and_highlights() {
        let theme = crate::tui::theme::Theme::default();
        let lines = render_indexed_fallback("refresh token failed", "token", &theme);
        let joined: String = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("source unavailable"));
        assert!(joined.contains("refresh token failed"));
        let any_reverse = lines.iter().flat_map(|l| &l.spans).any(|s| {
            s.content.contains("token")
                && s.style.add_modifier.contains(ratatui::style::Modifier::REVERSED)
        });
        assert!(any_reverse);
    }

    #[test]
    fn inline_code_uses_theme_code_role() {
        let theme = crate::tui::theme::Theme::default();
        let lines = render_prose("use the `cargo test` command", &theme);
        let found = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("cargo test") && s.style.fg == Some(theme.code))
        });
        assert!(found, "inline code span should use theme.code");
    }

    #[test]
    fn prose_plain_text_one_line() {
        let theme = crate::tui::theme::Theme::default();
        let lines = render_prose("hello world", &theme);
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.trim(), "hello world");
    }

    #[test]
    fn prose_bullets_get_marker() {
        let theme = crate::tui::theme::Theme::default();
        let lines = render_prose("- one\n- two", &theme);
        let joined: String = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("• one"));
        assert!(joined.contains("• two"));
    }

    #[test]
    fn nested_prose_bullets_are_indented() {
        let theme = crate::tui::theme::Theme::default();
        let lines = render_prose("- one\n  - two", &theme);
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect();
        assert!(rendered.iter().any(|line| line == "• one"));
        assert!(rendered.iter().any(|line| line == "  • two"));
    }

    #[test]
    fn prose_bold_is_styled_bold() {
        let theme = crate::tui::theme::Theme::default();
        let lines = render_prose("**strong**", &theme);
        let bold = lines.iter().flat_map(|l| &l.spans).any(|s| {
            s.content.contains("strong")
                && s.style.add_modifier.contains(ratatui::style::Modifier::BOLD)
        });
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

    #[test]
    fn invalidate_clears_all_stale_state() {
        let mut app = crate::tui::App::new();
        app.set_preview(true, 50);
        let session = crate::core::SessionSummary {
            id: "sess-1".into(),
            agent: crate::core::AgentId::Claude,
            title: "test session".into(),
            directory: "/tmp".into(),
            timestamp: 1000,
            message_count: 2,
            ..Default::default()
        };
        let transcript = crate::core::Transcript { messages: msgs() };

        let mut ps = PreviewState::default();
        let terms: Vec<String> = vec!["auth".into()];
        let t = transcript.clone();
        ps.update(&mut app, Some(&session), &terms, |_| Some(t), |_| None, 80);
        assert!(!ps.transcript.is_empty());
        assert!(!ps.lines.is_empty());
        assert!(ps.transcript_for.is_some());
        assert!(ps.key.is_some());

        ps.invalidate();

        assert!(ps.transcript.is_empty(), "transcript should be cleared");
        assert!(ps.lines.is_empty(), "lines should be cleared");
        assert!(ps.transcript_for.is_none(), "transcript_for should be None");
        assert!(ps.key.is_none(), "key should be None");
        assert!(!ps.source_unavailable, "source_unavailable should be false");
    }

    #[test]
    fn match_highlight_handles_multibyte_without_panic() {
        let theme = crate::tui::theme::Theme::default();
        let msgs = vec![Message {
            role: Role::User,
            blocks: vec![Block::Prose("café au lait latte".into())],
        }];
        let terms = vec!["latte".into()];
        let lines = render_transcript(
            &msgs,
            &terms,
            crate::core::AgentId::Claude,
            &theme,
            &Glyphs::ascii(),
            80,
        );
        // did not panic; and the ASCII term is still reverse-highlighted
        let any_rev = lines.iter().flat_map(|l| &l.spans).any(|s| {
            s.content.contains("latte")
                && s.style.add_modifier.contains(ratatui::style::Modifier::REVERSED)
        });
        assert!(any_rev);
    }

    #[test]
    fn separator_rendering_has_role_rules() {
        let theme = crate::tui::theme::Theme::default();
        let lines = render_transcript(
            &msgs(),
            &[],
            crate::core::AgentId::Claude,
            &theme,
            &Glyphs::ascii(),
            80,
        );
        let joined: String = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("── user"), "user separator should be present");
        assert!(joined.contains("── claude"), "agent separator should be present");
        assert!(joined.contains("fix the auth bug"));
        assert!(joined.contains("fn refresh"));
    }

    #[test]
    fn separator_role_name_is_bold() {
        let theme = crate::tui::theme::Theme::default();
        let lines = render_transcript(
            &msgs(),
            &[],
            crate::core::AgentId::Claude,
            &theme,
            &Glyphs::ascii(),
            80,
        );
        let bold_role = lines.iter().flat_map(|l| &l.spans).any(|s| {
            s.content == "user" && s.style.add_modifier.contains(ratatui::style::Modifier::BOLD)
        });
        assert!(bold_role, "role name in separator should be bold");
    }

    #[test]
    fn agent_separator_uses_brand_color_and_registered_glyph() {
        let theme = Theme::default();
        let mut glyphs = Glyphs::nerd();
        glyphs.set_agent_glyph(AgentId::Claude, "✱");

        let line = thin_rule(Role::Agent, AgentId::Claude, &glyphs, 40, &theme);

        assert_eq!(line.spans[1].content, "✱ claude");
        assert!(
            line.spans.iter().all(|span| span.style.fg == Some(theme.agent_color(AgentId::Claude))),
            "every agent separator span should use the brand color"
        );
    }

    #[test]
    fn user_separator_keeps_neutral_styles_and_has_no_glyph() {
        let theme = Theme::default();
        let mut glyphs = Glyphs::nerd();
        glyphs.set_agent_glyph(AgentId::Claude, "✱");

        let line = thin_rule(Role::User, AgentId::Claude, &glyphs, 40, &theme);

        assert_eq!(line.spans[1].content, "user");
        assert_eq!(line.spans[0].style.fg, Some(theme.border));
        assert_eq!(line.spans[1].style.fg, Some(theme.preview_text));
        assert_eq!(line.spans[2].style.fg, Some(theme.border));
        assert_eq!(line.spans[3].style.fg, Some(theme.border));
    }

    #[test]
    fn agent_body_lines_remain_undecorated() {
        let theme = Theme::default();
        let glyphs = Glyphs::ascii();
        let messages = vec![Message {
            role: Role::Agent,
            blocks: vec![
                Block::Prose("agent prose".into()),
                Block::Code { lang: Some("rust".into()), text: "fn agent_code() {}".into() },
            ],
        }];

        let lines = render_transcript(&messages, &[], AgentId::Claude, &theme, &glyphs, 80);
        let prose = lines.iter().find(|line| line_text(line).contains("agent prose")).unwrap();
        let code = lines.iter().find(|line| line_text(line).contains("fn agent_code")).unwrap();

        assert_eq!(line_text(prose), "agent prose");
        assert!(line_text(code).starts_with("  fn agent_code"));
        assert!(lines.last().is_some_and(|line| line.spans.is_empty()));
    }
}
