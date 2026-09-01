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

/// Root directory for data owned by aicross.
pub(crate) fn aicross_root() -> Result<PathBuf, String> {
    Ok(aicross_root_from(&home()?))
}

/// Root directory beneath an explicit home, used by isolated installers and
/// tests without consulting the user's real profile.
pub(crate) fn aicross_root_from(home: &Path) -> PathBuf {
    home.join(".aicross")
}
