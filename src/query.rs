use crate::core::AgentId;
use jiff::{Timestamp, tz::TimeZone};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AgentFilter {
    pub include: Vec<AgentId>,
    pub exclude: Vec<AgentId>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DirFilter {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// Substring include/exclude on the git remote URL. Unlike `dir`, this is stable
/// across every worktree of a repo, so `repo:` finds all sessions for one repo.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RepoFilter {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFilter {
    Today,
    Yesterday,
    LastWeek,
    LastMonth,
    /// Newer than `now - secs`.
    Within(i64),
    /// Older than `now - secs`.
    OlderThan(i64),
}

impl DateFilter {
    /// Inclusive (min, max) timestamp bounds in unix seconds; `None` = unbounded.
    pub fn range(self, now: i64) -> (Option<i64>, Option<i64>) {
        const D: i64 = 86_400;
        match self {
            DateFilter::Today => self
                .calendar_day_range(now, TimeZone::system(), 0)
                .unwrap_or((Some(now - D), Some(now))),
            DateFilter::Yesterday => self
                .calendar_day_range(now, TimeZone::system(), -1)
                .unwrap_or((Some(now - 2 * D), Some(now - D))),
            DateFilter::LastWeek => (Some(now - 7 * D), Some(now)),
            DateFilter::LastMonth => (Some(now - 30 * D), Some(now)),
            DateFilter::Within(s) => (Some(now - s), Some(now)),
            DateFilter::OlderThan(s) => (None, Some(now - s)),
        }
    }

    fn calendar_day_range(
        self,
        now: i64,
        tz: TimeZone,
        day_offset: i32,
    ) -> Option<(Option<i64>, Option<i64>)> {
        debug_assert!(matches!(self, DateFilter::Today | DateFilter::Yesterday));

        let now = Timestamp::from_second(now).ok()?.to_zoned(tz.clone());
        let today = now.date();
        let start_date = match day_offset {
            0 => today,
            -1 => today.yesterday().ok()?,
            _ => return None,
        };
        let end_date = start_date.tomorrow().ok()?;
        let start = start_date.to_zoned(tz.clone()).ok()?.timestamp().as_second();
        let end = end_date.to_zoned(tz).ok()?.timestamp().as_second().checked_sub(1)?;

        Some((Some(start), Some(end)))
    }

    pub fn summary(self) -> String {
        match self {
            DateFilter::Today => "date:today".to_string(),
            DateFilter::Yesterday => "date:yesterday".to_string(),
            DateFilter::LastWeek => "date:week".to_string(),
            DateFilter::LastMonth => "date:month".to_string(),
            DateFilter::Within(secs) => format!("date:<{}", format_duration(secs)),
            DateFilter::OlderThan(secs) => format!("date:>{}", format_duration(secs)),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
    pub free_text: String,
    pub agents: AgentFilter,
    pub dirs: DirFilter,
    pub repos: RepoFilter,
    pub date: Option<DateFilter>,
}

/// User-selectable result ordering. This is a display/ranking control, not part
/// of the parsed DSL: the TUI sets it (via the search toolbar) and
/// `SearchIndex::search` consumes it alongside `ParsedQuery`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Blend text relevance with a recency boost (the historical default). With
    /// an empty free-text query this degrades to newest-first.
    #[default]
    Relevance,
    /// Strict newest-first by session timestamp.
    Recent,
    /// Strict oldest-first by session timestamp.
    Oldest,
}

impl SortOrder {
    /// Stable label for the toolbar and status line.
    pub fn label(self) -> &'static str {
        match self {
            SortOrder::Relevance => "Relevance",
            SortOrder::Recent => "Recent",
            SortOrder::Oldest => "Oldest",
        }
    }

    /// Cycle to the next order, for a single-key toolbar toggle.
    pub fn next(self) -> SortOrder {
        match self {
            SortOrder::Relevance => SortOrder::Recent,
            SortOrder::Recent => SortOrder::Oldest,
            SortOrder::Oldest => SortOrder::Relevance,
        }
    }

    /// Cycle to the previous order.
    pub fn prev(self) -> SortOrder {
        match self {
            SortOrder::Relevance => SortOrder::Oldest,
            SortOrder::Recent => SortOrder::Relevance,
            SortOrder::Oldest => SortOrder::Recent,
        }
    }
}

impl ParsedQuery {
    pub fn free_terms(&self) -> Vec<String> {
        let mut terms = Vec::new();
        for term in self.free_text.split_whitespace() {
            let term = term.to_lowercase();
            if !term.is_empty() && !terms.contains(&term) {
                terms.push(term);
            }
        }
        terms
    }

    pub fn filter_summary(&self) -> Option<String> {
        let mut filters = Vec::new();
        if !self.agents.include.is_empty() {
            filters.push(format!(
                "agent:{}",
                self.agents.include.iter().map(|a| a.slug()).collect::<Vec<_>>().join(",")
            ));
        }
        if !self.agents.exclude.is_empty() {
            filters.push(format!(
                "-agent:{}",
                self.agents.exclude.iter().map(|a| a.slug()).collect::<Vec<_>>().join(",")
            ));
        }
        for dir in &self.dirs.include {
            filters.push(format!("dir:{dir}"));
        }
        for dir in &self.dirs.exclude {
            filters.push(format!("-dir:{dir}"));
        }
        for repo in &self.repos.include {
            filters.push(format!("repo:{repo}"));
        }
        for repo in &self.repos.exclude {
            filters.push(format!("-repo:{repo}"));
        }
        if let Some(date) = self.date {
            filters.push(date.summary());
        }
        if filters.is_empty() { None } else { Some(filters.join(",")) }
    }
}

/// Compose the effective query string for simple search mode: the guided repo
/// scope token (if any) followed by the user's free text. Mirrors how
/// `cli::initial_query` prepends filter tokens, so the existing `parse` pipeline
/// is reused unchanged rather than building a `ParsedQuery` by hand.
pub fn compose_simple(free_text: &str, repo_scope: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(slug) = repo_scope
        && !slug.is_empty()
    {
        out.push_str("repo:");
        out.push_str(slug);
    }
    if !free_text.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(free_text);
    }
    out
}

pub fn parse(input: &str) -> ParsedQuery {
    let mut q = ParsedQuery::default();
    let mut free: Vec<&str> = Vec::new();

    for tok in input.split_whitespace() {
        // A leading '-' or '!' negates the entire keyword token.
        let (negated, body) = match tok.strip_prefix(['-', '!']) {
            Some(rest) if rest.contains(':') => (true, rest),
            _ => (false, tok),
        };

        if let Some((key, val)) = body.split_once(':') {
            match key {
                "agent" => parse_agent(val, negated, &mut q.agents),
                "dir" => {
                    if negated {
                        q.dirs.exclude.push(val.to_string());
                    } else {
                        q.dirs.include.push(val.to_string());
                    }
                }
                "repo" => {
                    if negated {
                        q.repos.exclude.push(val.to_string());
                    } else {
                        q.repos.include.push(val.to_string());
                    }
                }
                "date" => {
                    if let Some(df) = parse_date(val) {
                        q.date = Some(df);
                    }
                }
                _ => free.push(tok),
            }
        } else {
            free.push(tok);
        }
    }

    q.free_text = free.join(" ");
    q
}

fn parse_agent(val: &str, token_negated: bool, out: &mut AgentFilter) {
    for part in val.split(',') {
        let (neg, name) = match part.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, part),
        };
        let neg = neg ^ token_negated;
        if let Some(agent) = AgentId::from_slug(name) {
            if neg {
                out.exclude.push(agent);
            } else {
                out.include.push(agent);
            }
        }
    }
}

fn parse_date(val: &str) -> Option<DateFilter> {
    match val {
        "today" => return Some(DateFilter::Today),
        "yesterday" => return Some(DateFilter::Yesterday),
        "week" => return Some(DateFilter::LastWeek),
        "month" => return Some(DateFilter::LastMonth),
        _ => {}
    }
    let (older, rest) = match val.strip_prefix('>') {
        Some(r) => (true, r),
        None => (false, val.strip_prefix('<').unwrap_or(val)),
    };
    let secs = parse_duration(rest)?;
    Some(if older { DateFilter::OlderThan(secs) } else { DateFilter::Within(secs) })
}

fn parse_duration(s: &str) -> Option<i64> {
    let (num, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit())?);
    let n: i64 = num.parse().ok()?;
    let mult = match unit {
        "h" => 3_600,
        "d" => 86_400,
        "w" => 604_800,
        _ => return None,
    };
    Some(n * mult)
}

fn format_duration(secs: i64) -> String {
    if secs % 604_800 == 0 {
        format!("{}w", secs / 604_800)
    } else if secs % 86_400 == 0 {
        format!("{}d", secs / 86_400)
    } else if secs % 3_600 == 0 {
        format!("{}h", secs / 3_600)
    } else {
        format!("{secs}s")
    }
}

/// Tab autocomplete for the last whitespace-delimited token.
/// Returns the full completed input string, or `None` if nothing to complete.
pub fn autocomplete(input: &str) -> Option<String> {
    let last = input.split_whitespace().last()?;
    let prefix_len = input.len() - last.len();
    let prefix = &input[..prefix_len];

    let completion = if let Some(partial) = last.strip_prefix("agent:") {
        complete_value(partial, &["claude", "codex", "cursor"]).map(|v| format!("agent:{v}"))
    } else if let Some(partial) = last.strip_prefix("date:") {
        complete_value(partial, &["today", "yesterday", "week", "month"])
            .map(|v| format!("date:{v}"))
    } else {
        None
    }?;

    Some(format!("{prefix}{completion}"))
}

fn complete_value(partial: &str, candidates: &[&str]) -> Option<String> {
    if partial.is_empty() {
        return None;
    }
    let matches: Vec<&&str> = candidates.iter().filter(|c| c.starts_with(partial)).collect();
    // Only complete when unambiguous and not already complete.
    match matches.as_slice() {
        [only] if **only != partial => Some((**only).to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AgentId;

    #[test]
    fn plain_free_text() {
        let q = parse("auth refresh token");
        assert_eq!(q.free_text, "auth refresh token");
        assert!(q.agents.include.is_empty() && q.agents.exclude.is_empty());
        assert!(q.dirs.include.is_empty() && q.dirs.exclude.is_empty());
        assert!(q.date.is_none());
    }

    #[test]
    fn agent_multi_value_and_negation() {
        let q = parse("agent:claude,!codex login");
        assert_eq!(q.agents.include, vec![AgentId::Claude]);
        assert_eq!(q.agents.exclude, vec![AgentId::Codex]);
        assert_eq!(q.free_text, "login");
    }

    #[test]
    fn agent_token_negation_prefix() {
        let q = parse("-agent:codex");
        assert_eq!(q.agents.exclude, vec![AgentId::Codex]);
        assert!(q.agents.include.is_empty());
    }

    #[test]
    fn dir_filters_include_and_exclude() {
        let q = parse("dir:api -dir:vendor bug");
        assert_eq!(q.dirs.include, vec!["api".to_string()]);
        assert_eq!(q.dirs.exclude, vec!["vendor".to_string()]);
        assert_eq!(q.free_text, "bug");
    }

    #[test]
    fn repo_filters_include_and_exclude() {
        let q = parse("repo:hop -repo:vendor bug");
        assert_eq!(q.repos.include, vec!["hop".to_string()]);
        assert_eq!(q.repos.exclude, vec!["vendor".to_string()]);
        assert_eq!(q.free_text, "bug");
    }

    #[test]
    fn date_keywords_and_comparisons() {
        assert_eq!(parse("date:today").date, Some(DateFilter::Today));
        assert_eq!(parse("date:yesterday").date, Some(DateFilter::Yesterday));
        assert_eq!(parse("date:week").date, Some(DateFilter::LastWeek));
        assert_eq!(parse("date:month").date, Some(DateFilter::LastMonth));
        assert_eq!(parse("date:<1h").date, Some(DateFilter::Within(3600)));
        assert_eq!(parse("date:<2d").date, Some(DateFilter::Within(2 * 86400)));
        assert_eq!(parse("date:>1w").date, Some(DateFilter::OlderThan(7 * 86400)));
    }

    #[test]
    fn date_range_windows() {
        let now = 1_000_000i64;
        assert_eq!(DateFilter::LastWeek.range(now), (Some(now - 7 * 86400), Some(now)));
        assert_eq!(DateFilter::LastMonth.range(now), (Some(now - 30 * 86400), Some(now)));
        assert_eq!(DateFilter::Within(3600).range(now), (Some(now - 3600), Some(now)));
        assert_eq!(DateFilter::OlderThan(3600).range(now), (None, Some(now - 3600)));
    }

    #[test]
    fn date_today_uses_local_calendar_day() {
        let tz = TimeZone::get("America/New_York").unwrap();
        let now = jiff::civil::date(2024, 3, 10)
            .at(12, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp()
            .as_second();

        let expected_start =
            jiff::civil::date(2024, 3, 10).to_zoned(tz.clone()).unwrap().timestamp().as_second();
        let expected_end =
            jiff::civil::date(2024, 3, 11).to_zoned(tz.clone()).unwrap().timestamp().as_second()
                - 1;

        assert_eq!(
            DateFilter::Today.calendar_day_range(now, tz, 0),
            Some((Some(expected_start), Some(expected_end)))
        );
        assert_ne!(expected_start, now - 86_400);
    }

    #[test]
    fn date_yesterday_uses_previous_local_calendar_day() {
        let tz = TimeZone::get("America/New_York").unwrap();
        let now = jiff::civil::date(2024, 3, 10)
            .at(12, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp()
            .as_second();

        let expected_start =
            jiff::civil::date(2024, 3, 9).to_zoned(tz.clone()).unwrap().timestamp().as_second();
        let expected_end =
            jiff::civil::date(2024, 3, 10).to_zoned(tz.clone()).unwrap().timestamp().as_second()
                - 1;

        assert_eq!(
            DateFilter::Yesterday.calendar_day_range(now, tz, -1),
            Some((Some(expected_start), Some(expected_end)))
        );
    }

    #[test]
    fn autocomplete_agent_value() {
        assert_eq!(autocomplete("agent:cl").as_deref(), Some("agent:claude"));
        assert_eq!(autocomplete("bug agent:co").as_deref(), Some("bug agent:codex"));
        // already complete -> no suggestion
        assert_eq!(autocomplete("agent:claude"), None);
        // free text -> no suggestion
        assert_eq!(autocomplete("auth"), None);
    }

    #[test]
    fn autocomplete_date_value() {
        assert_eq!(autocomplete("date:to").as_deref(), Some("date:today"));
        assert_eq!(autocomplete("date:y").as_deref(), Some("date:yesterday"));
    }

    #[test]
    fn free_terms_ignore_filters_and_deduplicate() {
        let q = parse("auth agent:codex auth dir:api");
        assert_eq!(q.free_terms(), vec!["auth".to_string()]);
    }

    #[test]
    fn filter_summary_is_parsed_not_raw_text() {
        let q = parse("auth -agent:codex dir:api -dir:vendor repo:hop date:<2d");
        assert_eq!(
            q.filter_summary().as_deref(),
            Some("-agent:codex,dir:api,-dir:vendor,repo:hop,date:<2d")
        );
    }
}
