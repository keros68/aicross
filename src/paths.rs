//! 路径工具：`home()` 取用户目录，`aicross_root()` 取 aicross 的数据目录。

use std::path::{Path, PathBuf};

/// Replace a prepared file, retrying only transient Windows sharing failures.
pub(crate) fn replace_file_with_retry(source: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    fn replace(source: &Path, target: &Path) -> std::io::Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};
        let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe { MoveFileExW(PCWSTR(source.as_ptr()), PCWSTR(target.as_ptr()), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) }
            .map_err(|error| {
                let hresult = error.code().0 as u32;
                let win32 = if hresult >> 16 == 0x8007 { (hresult & 0xffff) as i32 } else { hresult as i32 };
                std::io::Error::from_raw_os_error(win32)
            })
    }
    #[cfg(not(windows))]
    fn replace(source: &Path, target: &Path) -> std::io::Result<()> { std::fs::rename(source, target) }
    let delays = [25, 50, 100, 200, 400, 800];
    let mut result = replace(source, target);
    for delay in delays {
        match result {
            Ok(()) => return Ok(()),
            Err(error) if matches!(error.raw_os_error(), Some(5 | 32)) => {
                std::thread::sleep(std::time::Duration::from_millis(delay));
                result = replace(source, target);
            }
            Err(error) => return Err(error),
        }
    }
    result
}

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

#[cfg(test)]
mod tests {
    use super::{replace_file_with_retry, PathBuf};
    use std::fs;
    use std::time::{Duration, Instant};

    fn unique_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aicross-replace-{label}-{}-{}", std::process::id(), uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn replace_file_with_retry_overwrites_existing_file() {
        let dir = unique_dir("overwrite");
        let source = dir.join("source");
        let target = dir.join("target");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"old").unwrap();
        replace_file_with_retry(&source, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn replace_file_with_retry_missing_directory_fails_without_retry() {
        let dir = unique_dir("missing");
        let source = dir.join("source");
        let target = dir.join("missing").join("target");
        fs::write(&source, b"new").unwrap();
        let start = Instant::now();
        assert!(replace_file_with_retry(&source, &target).is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn replace_file_with_retry_succeeds_after_shared_handle_closes() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = unique_dir("released");
        let source = dir.join("source");
        let target = dir.join("target");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"old").unwrap();
        let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&target).unwrap();
        let closer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(held);
        });
        replace_file_with_retry(&source, &target).unwrap();
        closer.join().unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn replace_file_with_retry_stops_after_retry_window_when_target_is_locked() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = unique_dir("locked");
        let source = dir.join("source");
        let target = dir.join("target");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"old").unwrap();
        let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&target).unwrap();
        let start = Instant::now();
        let error = replace_file_with_retry(&source, &target).unwrap_err();
        let elapsed = start.elapsed();
        drop(held);
        assert!(elapsed < Duration::from_secs(3));
        assert!(matches!(error.raw_os_error(), Some(5 | 32)));
        let _ = fs::remove_dir_all(dir);
    }
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
