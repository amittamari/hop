use clap::{Parser, Subcommand as ClapSubcommand};

#[derive(Parser, Debug)]
#[command(name = "hop", about = "Search and resume coding-agent sessions")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Print version and check for updates.
    #[arg(short = 'V', long)]
    pub version: bool,
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

    /// Search across all repos, disabling auto-scoping to the current repo.
    #[arg(long)]
    pub all: bool,

    /// Force yolo resume when supported.
    #[arg(long)]
    pub yolo: bool,

    /// Wipe and rebuild the index before starting.
    #[arg(long)]
    pub rebuild: bool,
}

#[derive(ClapSubcommand, Debug)]
pub enum Command {
    /// Manage configuration file.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage session metadata hooks.
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },
    /// Internal: capture session metadata (called by hooks).
    Meta {
        #[command(subcommand)]
        action: MetaAction,
    },
}

#[derive(ClapSubcommand, Debug)]
pub enum ConfigAction {
    /// Scaffold a commented config template.
    Init,
    /// Open the config file in your editor.
    Edit,
    /// Print the effective configuration.
    Show,
    /// Print the config file path.
    Path,
}

#[derive(ClapSubcommand, Debug)]
pub enum HooksAction {
    /// Install hooks for detected providers.
    Install {
        /// Install for all detected providers without prompting.
        #[arg(long)]
        all: bool,
        /// Install for a specific provider only.
        #[arg(long)]
        provider: Option<String>,
    },
    /// Remove all hop hooks.
    Uninstall {
        #[arg(long)]
        all: bool,
        #[arg(long)]
        provider: Option<String>,
    },
    /// Show hook installation status.
    Status,
}

#[derive(ClapSubcommand, Debug)]
pub enum MetaAction {
    /// Capture session metadata from hook stdin.
    Capture {
        /// Agent name (claude, codex, cursor).
        #[arg(long)]
        agent: String,
        /// Hook event (start, stop).
        #[arg(long)]
        event: String,
    },
}

impl Cli {
    /// Compose the initial query string from positional + flag filters. When
    /// `auto_repo` is set (resolved from the current git repo), it is prepended as a
    /// `repo:` filter so bare `hop` scopes to the current repo.
    pub fn initial_query(&self, auto_repo: Option<&str>) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(slug) = auto_repo {
            parts.push(format!("repo:{slug}"));
        }
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
        let mut q = parts.join(" ");
        if !q.is_empty() {
            q.push(' ');
        }
        q
    }

    /// Whether to auto-scope to the current repo: not `--all`, no explicit `--repo`,
    /// and no `repo:` / `-repo:` token already typed in the positional query.
    pub fn wants_auto_repo(&self) -> bool {
        if self.all || self.repo.is_some() {
            return false;
        }
        let has_repo_token = self.query.as_deref().unwrap_or("").split_whitespace().any(|t| {
            let body = t.strip_prefix(['-', '!']).unwrap_or(t);
            body.split_once(':').map(|(k, _)| k == "repo").unwrap_or(false)
        });
        !has_repo_token
    }

    /// Whether the positional query contains a filter keyword token
    /// (`agent:`/`dir:`/`repo:`/`date:`). Used to start in raw search mode when
    /// the user typed DSL on the command line, so the simple toolbar does not
    /// silently drop it.
    pub fn query_has_dsl(&self) -> bool {
        self.query.as_deref().unwrap_or("").split_whitespace().any(|t| {
            let body = t.strip_prefix(['-', '!']).unwrap_or(t);
            body.split_once(':')
                .map(|(k, _)| matches!(k, "agent" | "dir" | "repo" | "date"))
                .unwrap_or(false)
        })
    }

    /// Filter flags the simple toolbar cannot yet represent (`--agent`/`--dir`).
    /// Their presence forces raw mode so the filters survive.
    pub fn has_unsupported_simple_flags(&self) -> bool {
        self.agent.is_some() || self.dir.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli() -> Cli {
        Cli {
            command: None,
            version: false,
            query: None,
            agent: None,
            dir: None,
            repo: None,
            all: false,
            yolo: false,
            rebuild: false,
        }
    }

    #[test]
    fn initial_query_composes_filters() {
        let cli = Cli {
            query: Some("auth".into()),
            agent: Some("claude".into()),
            dir: Some("api".into()),
            repo: Some("hop".into()),
            ..cli()
        };
        assert_eq!(cli.initial_query(None), "agent:claude dir:api repo:hop auth ");
    }

    #[test]
    fn initial_query_prepends_auto_repo() {
        let cli = Cli { query: Some("auth".into()), ..cli() };
        assert_eq!(cli.initial_query(Some("me/hop")), "repo:me/hop auth ");
    }

    #[test]
    fn initial_query_auto_repo_only_has_trailing_space() {
        let c = cli();
        assert_eq!(c.initial_query(Some("me/hop")), "repo:me/hop ");
    }

    #[test]
    fn initial_query_bare_returns_empty() {
        let c = cli();
        assert_eq!(c.initial_query(None), "");
    }

    #[test]
    fn wants_auto_repo_for_bare_and_free_text() {
        assert!(cli().wants_auto_repo());
        assert!(Cli { query: Some("auth".into()), ..cli() }.wants_auto_repo());
    }

    #[test]
    fn wants_auto_repo_suppressed_by_explicit_filters() {
        assert!(!Cli { all: true, ..cli() }.wants_auto_repo());
        assert!(!Cli { repo: Some("other".into()), ..cli() }.wants_auto_repo());
        assert!(!Cli { query: Some("repo:foo bug".into()), ..cli() }.wants_auto_repo());
        assert!(!Cli { query: Some("-repo:vendor".into()), ..cli() }.wants_auto_repo());
    }
}
