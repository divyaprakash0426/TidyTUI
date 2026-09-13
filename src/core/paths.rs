use std::path::{Path, PathBuf};

pub fn expand_tilde(raw: &str, home: Option<&Path>) -> PathBuf {
    match (raw.strip_prefix('~'), home) {
        (Some(rest), Some(home)) => home.join(rest.trim_start_matches('/')),
        _ => PathBuf::from(raw),
    }
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
