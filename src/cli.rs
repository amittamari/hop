use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "hop",
    version,
    about = "Search and resume coding-agent sessions"
)]
pub struct Cli {
    /// Pre-fill the search query.
    pub query: Option<String>,

    /// Filter by agent (claude|codex).
    #[arg(short, long)]
    pub agent: Option<String>,

    /// Filter by directory substring.
    #[arg(short, long)]
    pub dir: Option<String>,

    /// Filter by git remote URL substring (matches across all worktrees).
    #[arg(short, long)]
    pub repo: Option<String>,

    /// Force yolo resume when supported.
    #[arg(long)]
    pub yolo: bool,

    /// Wipe and rebuild the index before starting.
    #[arg(long)]
    pub rebuild: bool,
}

impl Cli {
    /// Compose the initial query string from positional + flag filters.
    pub fn initial_query(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(a) = &self.agent {
            parts.push(format!("agent:{a}"));
        }
        if let Some(d) = &self.dir {
            parts.push(format!("dir:{d}"));
        }
        if let Some(r) = &self.repo {
            parts.push(format!("repo:{r}"));
        }
        if let Some(q) = &self.query {
            parts.push(q.clone());
        }
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_query_composes_filters() {
        let cli = Cli {
            query: Some("auth".into()),
            agent: Some("claude".into()),
            dir: Some("api".into()),
            repo: Some("hop".into()),
            yolo: false,
            rebuild: false,
        };
        assert_eq!(cli.initial_query(), "agent:claude dir:api repo:hop auth");
    }
}
