use crate::core::CleanMode;
use rustix::fs::{Access, CWD};
use std::path::Path;

/// Whether the current user may clean `path` the way `mode` requires.
/// Emptying a directory needs write+search on it; removing a file or a whole
/// directory needs write+search on its parent. Errors count as "no".
pub fn can_clean(path: &Path, mode: CleanMode) -> bool {
    let dir_to_modify = if path.is_dir() && mode == CleanMode::Contents {
        path
    } else {
        match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => path,
        }
    };
    rustix::fs::accessat(
        CWD,
        dir_to_modify,
        Access::WRITE_OK | Access::EXEC_OK,
        rustix::fs::AtFlags::empty(),
    )
    .is_ok()
}

/// Tests skip permission-bit checks when run as root, who bypasses them.
#[cfg(test)]
pub fn is_root() -> bool {
    rustix::process::geteuid().is_root()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn writable_tempdir_is_cleanable_in_both_modes() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("cache");
        fs::create_dir(&inner).unwrap();
        assert!(can_clean(&inner, CleanMode::Contents));
        assert!(can_clean(&inner, CleanMode::Dir));
        let file = dir.path().join("f");
        fs::write(&file, b"x").unwrap();
        assert!(can_clean(&file, CleanMode::Contents));
    }

    #[test]
    fn system_dir_owned_by_root_is_locked_for_a_normal_user() {
        if is_root() {
            return;
        }
        assert!(!can_clean(Path::new("/usr/lib/"), CleanMode::Contents));
        assert!(!can_clean(Path::new("/usr/lib"), CleanMode::Dir));
    }

    #[test]
    fn read_only_dir_is_locked_for_contents_but_dir_mode_checks_parent() {
        if is_root() {
            return; // root bypasses mode bits
        }
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("cache");
        fs::create_dir(&inner).unwrap();
        fs::set_permissions(&inner, fs::Permissions::from_mode(0o555)).unwrap();
        assert!(!can_clean(&inner, CleanMode::Contents));
        assert!(can_clean(&inner, CleanMode::Dir));
        fs::set_permissions(&inner, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
