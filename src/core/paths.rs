use std::path::{Path, PathBuf};

pub fn expand_tilde(raw: &str, home: Option<&Path>) -> PathBuf {
    match (raw.strip_prefix('~'), home) {
        (Some(rest), Some(home)) => home.join(rest.trim_start_matches('/')),
        _ => PathBuf::from(raw),
    }
}

/// Whether `program` (a bare name or an absolute path) is installed.
pub fn program_on_path(program: &str) -> bool {
    let candidate = Path::new(program);
    if candidate.is_absolute() {
        return candidate.is_file();
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false)
}

pub fn shorten_home(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home {
        if let Ok(rest) = path.strip_prefix(home) {
            let rest = rest.to_string_lossy();
            return if rest.is_empty() {
                "~".to_string()
            } else {
                format!("~/{rest}")
            };
        }
    }
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_on_path_finds_sh_but_not_nonsense() {
        assert!(program_on_path("sh"));
        assert!(program_on_path("/bin/sh"));
        assert!(!program_on_path("definitely-not-a-program-xyz"));
    }

    #[test]
    fn expands_bare_tilde() {
        assert_eq!(
            expand_tilde("~", Some(Path::new("/home/u"))),
            PathBuf::from("/home/u")
        );
    }

    #[test]
    fn expands_tilde_prefix() {
        assert_eq!(
            expand_tilde("~/.cache/pip", Some(Path::new("/home/u"))),
            PathBuf::from("/home/u/.cache/pip")
        );
    }

    #[test]
    fn leaves_absolute_untouched() {
        assert_eq!(
            expand_tilde("/var/cache", Some(Path::new("/home/u"))),
            PathBuf::from("/var/cache")
        );
    }

    #[test]
    fn without_home_returns_raw() {
        assert_eq!(expand_tilde("~/x", None), PathBuf::from("~/x"));
    }

    #[test]
    fn shortens_home_prefix() {
        assert_eq!(
            shorten_home(Path::new("/home/u/.cache/pip"), Some(Path::new("/home/u"))),
            "~/.cache/pip"
        );
    }

    #[test]
    fn shorten_home_itself_is_tilde() {
        assert_eq!(
            shorten_home(Path::new("/home/u"), Some(Path::new("/home/u"))),
            "~"
        );
    }

    #[test]
    fn shorten_leaves_other_paths() {
        assert_eq!(
            shorten_home(Path::new("/var/cache"), Some(Path::new("/home/u"))),
            "/var/cache"
        );
    }
}
