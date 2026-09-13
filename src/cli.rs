use crate::tui::theme::Theme;
use clap::Parser;
use std::path::PathBuf;

/// A blazingly fast, terminal-based system cleaner.
#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[command(name = "tidytui", bin_name = "tidytui", version, about)]
pub struct Cli {
    /// Path to a definitions.yaml (overrides the default search locations)
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Start in dry-run mode (default; nothing is deleted)
    #[arg(short = 'n', long, conflicts_with = "danger")]
    pub dry_run: bool,

    /// Start in danger mode: cleaning really deletes files
    #[arg(long)]
    pub danger: bool,

    /// Scan and print what would be cleaned, then exit (no TUI)
    #[arg(short, long)]
    pub list: bool,

    /// With --list, print machine-readable JSON instead of a table
    #[arg(long, requires = "list")]
    pub json: bool,

    /// Pre-select these definition group ids (comma-separated), e.g. dev_pip,user_trash
    #[arg(short, long, value_delimiter = ',', value_name = "ID")]
    pub select: Vec<String>,

    /// Clean the --select'ed groups without the TUI (dry-run unless --danger)
    #[arg(short, long, requires = "select")]
    pub yes: bool,

    /// Colour theme: default, nord, gruvbox, dracula or mono (press t to cycle)
    #[arg(
        long,
        env = "TIDYTUI_THEME",
        default_value = "default",
        value_parser = clap::builder::PossibleValuesParser::new(Theme::names())
    )]
    pub theme: String,

    /// Disable mouse support (lets the terminal select text instead)
    #[arg(long)]
    pub no_mouse: bool,
}

impl Cli {
    /// Whether the run starts in dry-run mode; only `--danger` turns it off.
    pub fn dry_run(&self) -> bool {
        !self.danger
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("tidytui").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_are_safe() {
        let cli = parse(&[]).unwrap();
        assert!(cli.dry_run());
        assert!(!cli.list && !cli.json && !cli.yes);
        assert!(cli.select.is_empty());
        assert_eq!(cli.config, None);
    }

    #[test]
    fn danger_disables_dry_run_and_conflicts_with_explicit_dry_run() {
        assert!(!parse(&["--danger"]).unwrap().dry_run());
        assert!(parse(&["--danger", "--dry-run"]).is_err());
    }

    #[test]
    fn theme_is_validated_against_known_names() {
        assert_eq!(parse(&["--theme", "nord"]).unwrap().theme, "nord");
        assert!(parse(&["--theme", "solarized"]).is_err());
        assert_eq!(parse(&[]).unwrap().theme, "default");
        assert!(!parse(&[]).unwrap().no_mouse);
    }

    #[test]
    fn select_splits_on_commas() {
        let cli = parse(&["-s", "dev_pip,user_trash", "--select", "x"]).unwrap();
        assert_eq!(cli.select, vec!["dev_pip", "user_trash", "x"]);
    }

    #[test]
    fn json_requires_list_and_yes_requires_select() {
        assert!(parse(&["--json"]).is_err());
        assert!(parse(&["--list", "--json"]).is_ok());
        assert!(parse(&["--yes"]).is_err());
        assert!(parse(&["--yes", "--select", "a"]).is_ok());
    }

    #[test]
    fn config_path_is_parsed() {
        let cli = parse(&["-c", "/tmp/defs.yaml"]).unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/defs.yaml")));
    }
}
