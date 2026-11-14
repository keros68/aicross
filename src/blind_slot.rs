//! 盲审固定槽位目录：给会把每次工作目录写进本地历史索引的 CLI（kimi、agy）
//! 复用 `<AICROSS_HOME>/blind/<prefix>-1..8`，跑完再归档回 `runs/<run_id>/<node_id>`。

use crate::file_lock::FileLock;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub(crate) const SLOT_COUNT: usize = 8;
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const SHARING_RETRY_DELAYS_MS: [u64; 6] = [25, 50, 100, 200, 400, 800];

/// 需要固定槽位目录的通道前缀（kimi、agy）。
pub(crate) fn blind_slot_prefix(agent: &str) -> Option<&'static str> {
    match crate::channels::normalize_agent(agent) {
        "kimi" => Some("kimi"),
        "agy" => Some("agy"),
        _ => None,
    }
}

/// 从 `run_root`（生产环境下为 `<AICROSS_HOME>/runs/<run_id>`）推算 `<AICROSS_HOME>/blind`。
pub(crate) fn blind_root_for(run_root: &Path) -> PathBuf {
    if run_root
        .parent()
        .and_then(|parent| parent.file_name())
        .is_some_and(|name| name == "runs")
    {
        if let Some(base) = run_root.parent().and_then(|parent| parent.parent()) {
            return base.join("blind");
        }
    }
    if run_root
        .file_name()
        .is_some_and(|name| name == "runs")
    {
        if let Some(base) = run_root.parent() {
            return base.join("blind");
        }
    }
    if let Ok(base) = crate::dispatch::dispatch_data_root() {
        return base.join("blind");
    }
    run_root.join("blind")
}

/// 持有中的盲审槽位目录与跨进程文件锁；drop 时若未显式归档也会先清空目录再放锁。
#[derive(Debug)]
pub(crate) struct BlindSlotGuard {
    slot_dir: PathBuf,
    _lock: FileLock,
    released: bool,
}

impl BlindSlotGuard {
    pub(crate) fn dir(&self) -> &Path {
        &self.slot_dir
    }

    /// 从 `1..=SLOT_COUNT` 抢占首个空闲槽位；全忙时阻塞等待，受 `timeout` 与 `cancel_key` 限制。
    pub(crate) fn acquire(
        blind_root: &Path,
        prefix: &str,
        timeout: Duration,
        cancel_key: &str,
    ) -> Result<Self, String> {
        Self::acquire_with_cancel(blind_root, prefix, timeout, || {
            crate::dispatch::is_cancelled(cancel_key)
        })
    }

    /// 带自定义取消判定的槽位抢占，供 `resources.rs` 冒烟（允许无 `cancel_key`）与 `acquire` 共用。
    pub(crate) fn acquire_with_cancel(
        blind_root: &Path,
        prefix: &str,
        timeout: Duration,
        mut is_cancelled: impl FnMut() -> bool,
    ) -> Result<Self, String> {
        let started = Instant::now();
        loop {
            if is_cancelled() {
                return Err("派发已取消".into());
            }
            let mut busy = 0;
            let mut clear_error = None;
            for index in 1..=SLOT_COUNT {
                let lock_path = blind_root.join(format!("{prefix}-{index}.lock"));
                let Some(lock) = FileLock::try_acquire(&lock_path)? else {
                    busy += 1;
                    continue;
                };
                let slot_dir = blind_root.join(format!("{prefix}-{index}"));
                // 清不干净的槽位（残留进程还占着文件）不能给盲位置用：放锁，换下一个。
                match clear_dir_contents(&slot_dir) {
                    Ok(()) => {
                        return Ok(Self {
                            slot_dir,
                            _lock: lock,
                            released: false,
                        })
                    }
                    Err(error) => clear_error = Some(error),
                }
            }
            // 没有槽位被别的节点占用，却一个都清不干净：等下去也不会好，直接报原因。
            if busy == 0 {
                if let Some(error) = clear_error {
                    return Err(error);
                }
            }
            if started.elapsed() >= timeout {
                return Err("盲审目录都在使用中".into());
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            std::thread::sleep(POLL_INTERVAL.min(remaining));
        }
    }

    /// 把槽位里的材料与产物归档到 `archive_dir`（即 `runs/<run_id>/<node_id>`），
    /// 随后清空槽位并释放文件锁。即使归档出错仍尽最大努力清空并放锁。
    pub(crate) fn archive_and_release(mut self, archive_dir: &Path) -> Result<(), String> {
        self.released = true;
        let archive_res = archive_slot_contents(&self.slot_dir, archive_dir);
        let clear_res = clear_dir_contents(&self.slot_dir);
        match (archive_res, clear_res) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(archive_err), Ok(())) => Err(archive_err),
            (Ok(()), Err(clear_err)) => Err(format!("清理盲审目录失败：{clear_err}")),
            (Err(archive_err), Err(clear_err)) => {
                Err(format!("{archive_err}；清理盲审目录失败：{clear_err}"))
            }
        }
    }
}

impl Drop for BlindSlotGuard {
    fn drop(&mut self) {
        if !self.released {
            let _ = clear_dir_contents(&self.slot_dir);
        }
    }
}

fn is_transient_windows_lock(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(5 | 32))
}

fn retry_on_sharing<T>(mut op: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut result = op();
    for delay in SHARING_RETRY_DELAYS_MS {
        match result {
            Ok(value) => return Ok(value),
            Err(ref error) if is_transient_windows_lock(error) => {
                std::thread::sleep(Duration::from_millis(delay));
                result = op();
            }
            Err(_) => return result,
        }
    }
    result
}

fn clear_readonly_recursive(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if meta.file_type().is_symlink() {
        return;
    }
    let mut perms = meta.permissions();
    if perms.readonly() {
        perms.set_readonly(false);
        let _ = fs::set_permissions(path, perms);
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path).into_iter().flatten().flatten() {
            clear_readonly_recursive(&entry.path());
        }
    }
}

fn remove_path_with_retry(path: &Path) -> Result<(), String> {
    let remove_once = || -> std::io::Result<()> {
        let meta = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        if meta.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
    };
    match retry_on_sharing(remove_once) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            // 只读属性在 Windows 上也会报 AccessDenied（OS error 5），去掉后再删一次。
            clear_readonly_recursive(path);
            retry_on_sharing(remove_once)
                .map_err(|error| format!("删除 {} 失败：{first_error}（重试后：{error}）", path.display()))
        }
    }
}

/// 确保 `dir` 存在且内部为空：清理上一轮崩溃遗留或本轮用完后的残留。
pub(crate) fn clear_dir_contents(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|error| format!("创建目录 {} 失败：{error}", dir.display()))?;
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("读取目录 {} 失败：{error}", dir.display()))?;
    let mut errors = Vec::new();
    for entry in entries.flatten() {
        if let Err(error) = remove_path_with_retry(&entry.path()) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

fn copy_entry_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(src)
        .map_err(|error| format!("读取 {} 元数据失败：{error}", src.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!("不支持归档符号链接 {}", src.display()));
    }
    if meta.is_dir() {
        fs::create_dir_all(dst)
            .map_err(|error| format!("创建归档目录 {} 失败：{error}", dst.display()))?;
        let entries = fs::read_dir(src)
            .map_err(|error| format!("读取目录 {} 失败：{error}", src.display()))?;
        for entry in entries.flatten() {
            copy_entry_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("创建归档父目录 {} 失败：{error}", parent.display()))?;
        }
        retry_on_sharing(|| fs::copy(src, dst).map(|_| ()))
            .map_err(|error| format!("复制 {} 到 {} 失败：{error}", src.display(), dst.display()))
    }
}

fn move_entry_to_archive(src: &Path, dst: &Path) -> Result<(), String> {
    if fs::symlink_metadata(dst).is_ok() {
        remove_path_with_retry(dst)?;
    }
    match retry_on_sharing(|| fs::rename(src, dst)) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            // rename 遇到占用重试仍失败（或跨盘）时兜底：先完整拷贝到 archive，再删源。
            copy_entry_recursive(src, dst)
                .map_err(|copy_err| format!("归档移动 {} 失败（{rename_error}），拷贝兜底也失败：{copy_err}", src.display()))?;
            remove_path_with_retry(src)
        }
    }
}

fn archive_slot_contents(slot_dir: &Path, archive_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(archive_dir)
        .map_err(|error| format!("创建归档目录 {} 失败：{error}", archive_dir.display()))?;
    if !slot_dir.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(slot_dir)
        .map_err(|error| format!("读取盲审槽位 {} 失败：{error}", slot_dir.display()))?;
    let mut errors = Vec::new();
    for entry in entries.flatten() {
        let src = entry.path();
        let dst = archive_dir.join(entry.file_name());
        if let Err(error) = move_entry_to_archive(&src, &dst) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn unique_dir(tag: &str) -> PathBuf {
        let dir = crate::paths::test_temp_dir().join(format!("blind-slot-{tag}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn blind_root_for_resolves_runs_layout_and_direct_runs_dir() {
        let base = unique_dir("layout");
        assert_eq!(blind_root_for(&base.join("runs").join("run-123")), base.join("blind"));
        assert_eq!(blind_root_for(&base.join("runs")), base.join("blind"));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn acquire_clears_crash_leftovers_before_handing_out_slot() {
        let base = unique_dir("crash-clean");
        let blind_root = base.join("blind");
        let slot1 = blind_root.join("kimi-1");
        fs::create_dir_all(slot1.join("sub")).unwrap();
        fs::write(slot1.join("stale.txt"), b"leftover").unwrap();
        fs::write(slot1.join("sub").join("deep.txt"), b"deep").unwrap();

        let guard = BlindSlotGuard::acquire(&blind_root, "kimi", Duration::from_secs(1), "none").unwrap();
        assert_eq!(guard.dir(), slot1);
        assert!(fs::read_dir(guard.dir()).unwrap().next().is_none(), "接手槽位前必须先清光上一轮残留");
        drop(guard);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn slot_that_cannot_be_cleared_is_skipped_and_all_unclearable_fails_at_once() {
        let base = unique_dir("unclearable");
        let blind_root = base.join("blind");
        fs::create_dir_all(&blind_root).unwrap();
        // 槽位路径被一个普通文件占住：清空必然失败。
        fs::write(blind_root.join("kimi-1"), b"not a dir").unwrap();

        let guard = BlindSlotGuard::acquire(&blind_root, "kimi", Duration::from_secs(1), "none").unwrap();
        assert_eq!(guard.dir(), blind_root.join("kimi-2"));
        assert!(
            FileLock::try_acquire(&blind_root.join("kimi-1.lock")).unwrap().is_some(),
            "跳过的槽位要放锁"
        );
        drop(guard);

        for index in 2..=SLOT_COUNT {
            let _ = fs::remove_dir_all(blind_root.join(format!("kimi-{index}")));
            fs::write(blind_root.join(format!("kimi-{index}")), b"not a dir").unwrap();
        }
        let started = Instant::now();
        let err = BlindSlotGuard::acquire(&blind_root, "kimi", Duration::from_secs(30), "none").unwrap_err();
        assert!(err.contains("创建目录"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(5));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn nine_concurrent_attempts_assign_eight_distinct_slots_and_times_out_ninth() {
        let base = unique_dir("eight-slots");
        let blind_root = base.join("blind");
        let mut guards = Vec::new();
        let mut dirs = std::collections::HashSet::new();
        for _ in 0..SLOT_COUNT {
            let guard = BlindSlotGuard::acquire(&blind_root, "kimi", Duration::from_secs(1), "k-all").unwrap();
            assert!(dirs.insert(guard.dir().to_path_buf()));
            guards.push(guard);
        }
        assert_eq!(dirs.len(), SLOT_COUNT);

        let started = Instant::now();
        let err = BlindSlotGuard::acquire(&blind_root, "kimi", Duration::from_millis(120), "k-ninth").unwrap_err();
        assert_eq!(err, "盲审目录都在使用中");
        assert!(started.elapsed() >= Duration::from_millis(100));

        // 释放其中 1 个后，新请求应立刻拿到该槽位并归档清理。
        let released = guards.pop().unwrap();
        let expected = released.dir().to_path_buf();
        fs::write(expected.join("hello.txt"), b"hi").unwrap();
        let archive = base.join("archive");
        released.archive_and_release(&archive).unwrap();
        assert_eq!(fs::read(archive.join("hello.txt")).unwrap(), b"hi");
        assert!(fs::read_dir(&expected).unwrap().next().is_none());

        let reused = BlindSlotGuard::acquire(&blind_root, "kimi", Duration::from_millis(200), "k-reuse").unwrap();
        assert_eq!(reused.dir(), expected);
        drop(reused);
        drop(guards);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn waiting_for_full_slots_stops_immediately_when_cancelled() {
        let base = unique_dir("cancel");
        let blind_root = base.join("blind");
        let _guards: Vec<_> = (0..SLOT_COUNT)
            .map(|_| BlindSlotGuard::acquire(&blind_root, "kimi", Duration::from_secs(1), "hold").unwrap())
            .collect();

        let key = format!("slot-cancel-{}", uuid::Uuid::new_v4());
        let cancel_key = key.clone();
        let blind_root_clone = blind_root.clone();
        let barrier = Arc::new(Barrier::new(2));
        let barrier_bg = barrier.clone();
        let handle = std::thread::spawn(move || {
            barrier_bg.wait();
            BlindSlotGuard::acquire(&blind_root_clone, "kimi", Duration::from_secs(30), &cancel_key)
        });
        barrier.wait();
        std::thread::sleep(Duration::from_millis(80));
        crate::dispatch::mark_cancelled(&key);
        let started = Instant::now();
        let err = handle.join().unwrap().unwrap_err();
        assert_eq!(err, "派发已取消");
        assert!(started.elapsed() < Duration::from_secs(2));
        crate::dispatch::clear_cancelled(&key);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn agy_uses_blind_slots_and_non_slot_agents_stay_none() {
        assert_eq!(blind_slot_prefix("kimi"), Some("kimi"));
        assert_eq!(blind_slot_prefix("agy"), Some("agy"));
        assert_eq!(blind_slot_prefix("claude"), None);
        assert_eq!(blind_slot_prefix("codex"), None);
        assert_eq!(blind_slot_prefix("pi"), None);
    }
}
