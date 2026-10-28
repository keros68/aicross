//! 窗口启动前整理进程环境。
//!
//! 从开始菜单、Dock 或桌面图标启动时，进程拿到的 PATH 往往比终端里少，
//! npm、pnpm、scoop、Homebrew 等装的 agent CLI 会被探测成「未安装」。
//! 这里在启动时补齐一次，之后探测与派发拉起的子进程都继承它。
//!
//! Windows 的系统代理只写在注册表里，浏览器和 IDE 会用，命令行程序
//! （agy 等）只认 HTTP(S)_PROXY 环境变量。没设这些变量时，把已开启的
//! 系统代理转成环境变量；想直连时把 HTTPS_PROXY 设成空值即可跳过。

/// 在 main() 一开始、尚未起任何线程时调用。
pub(crate) fn prepare() {
    #[cfg(target_os = "windows")]
    {
        win::extend_path_from_registry();
        win::adopt_system_proxy();
    }

    #[cfg(not(target_os = "windows"))]
    unix::adopt_login_shell_path();

    // 个别 Linux 显卡驱动与合成器组合下 WebKitGTK 在 Wayland 上白屏，
    // 设 AICROSS_FORCE_X11=1 改走 X11。
    #[cfg(target_os = "linux")]
    if std::env::var_os("GDK_BACKEND").is_none() && std::env::var_os("AICROSS_FORCE_X11").is_some() {
        // 单线程阶段，没有并发读环境变量的线程。
        std::env::set_var("GDK_BACKEND", "x11");
    }
}

/// 把 `extra` 里 `current` 还没有的目录按顺序追加到末尾。比较时忽略大小写
/// 和结尾的 `\`；已有的条目原样保留，不删除、不调整顺序。
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
fn append_missing(current: &str, extra: &[String]) -> String {
    let key = |entry: &str| entry.trim().trim_end_matches('\\').to_lowercase();
    let mut seen: std::collections::HashSet<String> = current
        .split(';')
        .map(key)
        .filter(|entry| !entry.is_empty())
        .collect();
    let mut result = current.to_string();
    for dir in extra {
        let dir_key = key(dir);
        if dir_key.is_empty() || !seen.insert(dir_key) {
            continue;
        }
        if !result.is_empty() {
            result.push(';');
        }
        result.push_str(dir);
    }
    result
}

/// 注册表 `ProxyServer` 转成代理地址。值可能是 `host:port`、带协议的地址，
/// 或 `http=…;https=…;socks=…` 分协议写法；分协议时依次取 https、http、socks。
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
fn proxy_url(server: &str) -> Option<String> {
    let server = server.trim();
    if server.is_empty() {
        return None;
    }
    let with_scheme = |address: &str, scheme: &str| {
        let address = address.trim();
        if address.is_empty() {
            None
        } else if address.contains("://") {
            Some(address.to_string())
        } else {
            Some(format!("{scheme}://{address}"))
        }
    };
    if !server.contains('=') {
        return with_scheme(server.split(';').next().unwrap_or(""), "http");
    }
    let entries: Vec<(String, &str)> = server
        .split(';')
        .filter_map(|entry| entry.split_once('='))
        .map(|(kind, address)| (kind.trim().to_lowercase(), address))
        .collect();
    let pick = |kind: &str| entries.iter().find(|(name, _)| name == kind).map(|(_, address)| *address);
    pick("https")
        .and_then(|address| with_scheme(address, "http"))
        .or_else(|| pick("http").and_then(|address| with_scheme(address, "http")))
        .or_else(|| pick("socks").and_then(|address| with_scheme(address, "socks5")))
}

/// 注册表 `ProxyOverride`（分号分隔，`<local>` 表示本机）转成 NO_PROXY；
/// 本机地址总是直连。
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
fn no_proxy(override_list: &str) -> String {
    let mut hosts = vec!["localhost".to_string(), "127.0.0.1".to_string(), "::1".to_string()];
    for entry in override_list.split(';').map(str::trim) {
        if entry.is_empty() || entry.eq_ignore_ascii_case("<local>") {
            continue;
        }
        if !hosts.iter().any(|host| host.eq_ignore_ascii_case(entry)) {
            hosts.push(entry.to_string());
        }
    }
    hosts.join(",")
}

#[cfg(target_os = "windows")]
mod win {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::Path;

    use ::windows::core::{w, PCWSTR};
    use ::windows::Win32::System::Registry::{
        RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_EXPAND_SZ,
        RRF_RT_REG_SZ,
    };

    const INTERNET_SETTINGS: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");

    /// 系统代理已开启、且进程里没有任何代理环境变量时，写入
    /// HTTP_PROXY / HTTPS_PROXY，NO_PROXY 未设时一并写入。
    pub(super) fn adopt_system_proxy() {
        // Windows 环境变量不分大小写，查大写即可；设成空值也算用户已自行决定。
        if ["HTTPS_PROXY", "HTTP_PROXY", "ALL_PROXY"].iter().any(|name| std::env::var_os(name).is_some()) {
            return;
        }
        if read_dword(HKEY_CURRENT_USER, INTERNET_SETTINGS, w!("ProxyEnable")) != Some(1) {
            return;
        }
        let Some(url) = read_string(HKEY_CURRENT_USER, INTERNET_SETTINGS, w!("ProxyServer"))
            .as_deref()
            .and_then(super::proxy_url)
        else {
            return;
        };
        // 单线程阶段，没有并发读环境变量的线程。
        std::env::set_var("HTTP_PROXY", &url);
        std::env::set_var("HTTPS_PROXY", &url);
        if std::env::var_os("NO_PROXY").is_none() {
            let overrides = read_string(HKEY_CURRENT_USER, INTERNET_SETTINGS, w!("ProxyOverride")).unwrap_or_default();
            std::env::set_var("NO_PROXY", super::no_proxy(&overrides));
        }
    }

    fn read_dword(root: HKEY, subkey: PCWSTR, name: PCWSTR) -> Option<u32> {
        let mut value = 0u32;
        let mut bytes = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            RegGetValueW(root, subkey, name, RRF_RT_REG_DWORD, None, Some((&mut value as *mut u32).cast()), Some(&mut bytes))
        };
        status.is_ok().then_some(value)
    }

    /// 注册表里系统与用户两级 Path 是新开命令行看到的 PATH。把其中存在、
    /// 而当前进程 PATH 缺的目录追加进来。
    pub(super) fn extend_path_from_registry() {
        let system = read_string(
            HKEY_LOCAL_MACHINE,
            w!("SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment"),
            w!("Path"),
        );
        let user = read_string(HKEY_CURRENT_USER, w!("Environment"), w!("Path"));
        let dirs: Vec<String> = [system, user]
            .into_iter()
            .flatten()
            .flat_map(|value| value.split(';').map(str::trim).map(str::to_string).collect::<Vec<_>>())
            .filter(|dir| !dir.is_empty() && Path::new(dir).is_dir())
            .collect();
        if dirs.is_empty() {
            return;
        }
        let current = std::env::var("PATH").unwrap_or_default();
        let merged = super::append_missing(&current, &dirs);
        if merged != current {
            // 单线程阶段，没有并发读环境变量的线程。
            std::env::set_var("PATH", merged);
        }
    }

    /// 读 `subkey` 下的字符串值；REG_EXPAND_SZ 里的 %VAR% 由系统展开。
    fn read_string(root: HKEY, subkey: PCWSTR, name: PCWSTR) -> Option<String> {
        let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
        let mut buffer = vec![0u16; 4096];
        // 缓冲不够时系统回填所需字节数，按它扩一次再读；两次都失败就放弃。
        for _ in 0..2 {
            let mut bytes = (buffer.len() * 2) as u32;
            let status = unsafe {
                RegGetValueW(
                    root,
                    subkey,
                    name,
                    flags,
                    None,
                    Some(buffer.as_mut_ptr().cast()),
                    Some(&mut bytes),
                )
            };
            let units = bytes as usize / 2;
            if status.is_ok() {
                let text = &buffer[..units.min(buffer.len())];
                let end = text.iter().position(|&unit| unit == 0).unwrap_or(text.len());
                return Some(OsString::from_wide(&text[..end]).to_string_lossy().into_owned());
            }
            if units <= buffer.len() {
                return None;
            }
            buffer = vec![0u16; units];
        }
        None
    }
}

#[cfg(not(target_os = "windows"))]
mod unix {
    /// 让用户的登录 shell 跑一次 `env`，取输出里的 PATH 行替换进程 PATH。
    /// 交互加登录模式会读 .zprofile、.zshrc 等，与新开终端一致；shell 报错
    /// 或输出里没有像样的 PATH 时保持原值。
    pub(super) fn adopt_login_shell_path() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let Ok(output) = std::process::Command::new(&shell).args(["-l", "-i", "-c", "env"]).output() else {
            return;
        };
        if !output.status.success() {
            return;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        if let Some(path) = text.lines().filter_map(|line| line.strip_prefix("PATH=")).last() {
            if path.contains(':') {
                // 单线程阶段，没有并发读环境变量的线程。
                std::env::set_var("PATH", path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{append_missing, no_proxy, proxy_url};

    #[test]
    fn proxy_url_from_registry_formats() {
        assert_eq!(proxy_url("127.0.0.1:10808").as_deref(), Some("http://127.0.0.1:10808"));
        assert_eq!(proxy_url(" http://127.0.0.1:7890 ").as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(
            proxy_url("http=127.0.0.1:7890;https=127.0.0.1:7891;socks=127.0.0.1:7892").as_deref(),
            Some("http://127.0.0.1:7891")
        );
        assert_eq!(proxy_url("ftp=1.2.3.4:21;HTTP=127.0.0.1:7890").as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(proxy_url("socks=127.0.0.1:1080").as_deref(), Some("socks5://127.0.0.1:1080"));
        assert_eq!(proxy_url(""), None);
        assert_eq!(proxy_url("ftp=1.2.3.4:21"), None);
    }

    #[test]
    fn no_proxy_keeps_local_and_overrides() {
        assert_eq!(no_proxy(""), "localhost,127.0.0.1,::1");
        assert_eq!(
            no_proxy("<local>;*.lan; 10.*;localhost;"),
            "localhost,127.0.0.1,::1,*.lan,10.*"
        );
    }

    fn dirs(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn appends_only_missing_dirs_in_order() {
        assert_eq!(
            append_missing(r"C:\Windows;C:\Tools\", &dirs(&[r"c:\tools", r"D:\npm", r"D:\npm\", r"E:\bin"])),
            r"C:\Windows;C:\Tools\;D:\npm;E:\bin"
        );
    }

    #[test]
    fn empty_inputs() {
        assert_eq!(append_missing("", &dirs(&[r"D:\npm"])), r"D:\npm");
        assert_eq!(append_missing(r"C:\Windows", &[]), r"C:\Windows");
        assert_eq!(append_missing(r"C:\Windows;", &dirs(&["", "  "])), r"C:\Windows;");
    }
}
