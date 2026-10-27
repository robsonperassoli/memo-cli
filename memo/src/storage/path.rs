use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Resolve the database file.
///
/// Precedence is the `--db` flag, then `MEMO_DB`, then
/// `$XDG_DATA_HOME/memo/memo.db`, then `~/.local/share/memo/memo.db`.
/// An empty `XDG_DATA_HOME` or `MEMO_DB` is treated as unset. Directories are
/// created when the store is opened, not here.
pub fn resolve_db_path(flag: Option<&Path>) -> Result<PathBuf> {
    resolve_db_path_with(
        flag,
        std::env::var_os("MEMO_DB").as_deref(),
        std::env::var_os("XDG_DATA_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

pub(crate) fn resolve_db_path_with(
    flag: Option<&Path>,
    memo_db: Option<&OsStr>,
    xdg_data_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<PathBuf> {
    if let Some(path) = flag {
        if path.as_os_str().is_empty() {
            return Err(Error::EmptyPath);
        }
        return Ok(path.to_path_buf());
    }

    if let Some(path) = non_empty(memo_db) {
        return Ok(PathBuf::from(path));
    }

    let base = if let Some(xdg) = non_empty(xdg_data_home) {
        PathBuf::from(xdg)
    } else {
        let home = non_empty(home).ok_or(Error::HomeMissing)?;
        PathBuf::from(home).join(".local").join("share")
    };
    Ok(base.join("memo").join("memo.db"))
}

fn non_empty(value: Option<&OsStr>) -> Option<&OsStr> {
    value.filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::Path;

    use super::resolve_db_path_with;

    #[test]
    fn flag_overrides_env_and_xdg() {
        let path = resolve_db_path_with(
            Some(Path::new("/tmp/custom.db")),
            Some(OsStr::new("/tmp/env.db")),
            Some(OsStr::new("/tmp/xdg")),
            Some(OsStr::new("/home/agent")),
        )
        .unwrap();
        assert_eq!(path, Path::new("/tmp/custom.db"));
    }

    #[test]
    fn memo_db_overrides_xdg_and_home() {
        let path = resolve_db_path_with(
            None,
            Some(OsStr::new("/var/memo.db")),
            Some(OsStr::new("/tmp/xdg")),
            Some(OsStr::new("/home/agent")),
        )
        .unwrap();
        assert_eq!(path, Path::new("/var/memo.db"));
    }

    #[test]
    fn empty_memo_db_falls_through_to_xdg() {
        let path = resolve_db_path_with(
            None,
            Some(OsStr::new("")),
            Some(OsStr::new("/tmp/xdg")),
            Some(OsStr::new("/home/agent")),
        )
        .unwrap();
        assert_eq!(path, Path::new("/tmp/xdg/memo/memo.db"));
    }

    #[test]
    fn empty_xdg_uses_home_local_share() {
        let path = resolve_db_path_with(
            None,
            None,
            Some(OsStr::new("")),
            Some(OsStr::new("/home/agent")),
        )
        .unwrap();
        assert_eq!(path, Path::new("/home/agent/.local/share/memo/memo.db"));
    }

    #[test]
    fn missing_home_is_an_error() {
        let err = resolve_db_path_with(None, None, None, None).unwrap_err();
        assert!(matches!(err, crate::Error::HomeMissing));
    }

    #[test]
    fn empty_flag_is_an_error() {
        let err = resolve_db_path_with(Some(Path::new("")), None, None, None).unwrap_err();
        assert!(matches!(err, crate::Error::EmptyPath));
    }
}
