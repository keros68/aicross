//! Shared filesystem path helpers.
//!
//! `home()` resolves the user's home directory. `aicross_root()` is the
//! canonical root for data owned by this fork; callers must never fall back to
//! Coffee CLI's `~/.coffee-cli/` tree.

use std::path::{Path, PathBuf};

/// The user's home directory, or an error string if the OS can't resolve it
/// (no `$HOME` on Linux / profile-load failure on Windows).
pub(crate) fn home() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "No home directory".to_string())
}

/// Root directory for data owned by aicross: `AICROSS_HOME` when set and
/// non-empty, otherwise `~/.aicross`.
pub(crate) fn aicross_root() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("AICROSS_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    Ok(aicross_root_from(&home()?))
}

/// Root directory beneath an explicit home, used by isolated installers and
/// tests without consulting the user's real profile.
pub(crate) fn aicross_root_from(home: &Path) -> PathBuf {
    home.join(".aicross")
}

/// Scratch directory for this test process: `<base>/<pid>`, where base is
/// `$AICROSS_HOME/tests` or `<system temp>/aicross-tests`. The first call
/// deletes sibling directories untouched for over an hour, so leftovers of
/// earlier runs go away on the next run; two overlapping `cargo test`
/// processes are safe as long as neither sits idle for an hour.
#[cfg(test)]
pub(crate) fn test_temp_dir() -> PathBuf {
    use std::sync::OnceLock;
    use std::time::Duration;
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let base = std::env::var_os("AICROSS_HOME")
            .filter(|value| !value.is_empty())
            .map(|home| PathBuf::from(home).join("tests"))
            .unwrap_or_else(|| std::env::temp_dir().join("aicross-tests"));
        let own = base.join(std::process::id().to_string());
        for entry in std::fs::read_dir(&base).into_iter().flatten().flatten() {
            let stale = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|time| time.elapsed().ok())
                .is_some_and(|age| age > Duration::from_secs(3600));
            if stale && entry.path() != own {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        let _ = std::fs::create_dir_all(&own);
        own
    })
    .clone()
}
