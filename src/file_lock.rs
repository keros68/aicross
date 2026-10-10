//! 跨进程互斥：操作系统的文件锁（`std::fs::File::try_lock`，Windows 上是
//! `LockFileEx`，Unix 上是 `flock`），不加依赖。
//!
//! 窗口、命令行、后台 `__run` 各是一个进程，进程内 `Mutex` 挡不住彼此；
//! 读改写同一份文件（资源库、skill 安装目录）的地方都先拿这把锁。锁跟着
//! 文件句柄走：句柄关闭（含进程崩溃）即由系统释放，不需要过期接管与令牌。
//! 锁文件本身常驻、不删除——删除会让「删旧文件」与「另一方锁新文件」交错，
//! 两边各锁一份不同的文件。

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// 等锁时的轮询间隔。
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// 持有中的锁；drop 时关闭句柄，系统随即释放。
#[derive(Debug)]
pub(crate) struct FileLock {
    _file: fs::File,
}

impl FileLock {
    /// 打开（没有就建）`path` 并加独占锁；已被别的句柄锁住时轮询等待，
    /// 最多等 `timeout`，到时返回错误。锁文件所在目录不存在时先建。
    pub(crate) fn acquire(path: &Path, timeout: Duration) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("创建目录 {} 失败：{error}", parent.display()))?;
        }
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| format!("打开锁文件 {} 失败：{error}", path.display()))?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(fs::TryLockError::WouldBlock) => {}
                Err(fs::TryLockError::Error(error)) => {
                    return Err(format!("锁定 {} 失败：{error}", path.display()));
                }
            }
            if started.elapsed() >= timeout {
                return Err(format!(
                    "等待锁 {} 超时（{} 秒）：另一个 aicross 进程正在写入，请稍后重试",
                    path.display(),
                    timeout.as_secs()
                ));
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = crate::paths::test_temp_dir().join(format!("file-lock-{tag}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 两个持有者互斥：第一个持锁期间第二个拿不到，超时返回错误。
    #[test]
    fn file_lock_two_holders_are_exclusive_and_wait_times_out() {
        let dir = temp_dir("exclusive");
        let path = dir.join("x.lock");
        let first = FileLock::acquire(&path, Duration::from_secs(1)).unwrap();
        let started = Instant::now();
        let error = FileLock::acquire(&path, Duration::from_millis(150)).unwrap_err();
        assert!(error.contains("超时"), "{error}");
        assert!(started.elapsed() >= Duration::from_millis(150));
        drop(first);
        let _ = fs::remove_dir_all(dir);
    }

    /// 持有者句柄关闭后另一方立即拿到；锁文件常驻，不被删除。等锁的
    /// 线程在持有者释放后返回。
    #[test]
    fn file_lock_released_when_holder_handle_closes() {
        let dir = temp_dir("release");
        let path = dir.join("x.lock");
        let first = FileLock::acquire(&path, Duration::from_secs(1)).unwrap();
        let waiter_path = path.clone();
        let waiter = std::thread::spawn(move || FileLock::acquire(&waiter_path, Duration::from_secs(10)));
        std::thread::sleep(Duration::from_millis(100));
        assert!(!waiter.is_finished(), "持有期间等待方拿不到锁");
        drop(first);
        let second = waiter.join().unwrap().unwrap();
        assert!(path.is_file(), "锁文件常驻");
        drop(second);
        let third = FileLock::acquire(&path, Duration::ZERO).unwrap();
        drop(third);
        let _ = fs::remove_dir_all(dir);
    }
}
