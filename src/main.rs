#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod channels;
mod console_cli;
mod dispatch;
mod engine;
mod git;
mod hosts;
mod orchestration;
mod paths;
mod preset;
mod process;
mod recommend;
mod resources;
mod run_view;
mod server;
mod skill_install;
mod slash;
mod tool_config;
#[cfg(target_os = "windows")]
mod windows_path;

use anyhow::Result;

fn main() -> Result<()> {
    // ── Linux GUI backend selection ─────────────────────────────────────
    // Older WebKit2GTK (≤ 2.44) had a Wayland blank-window bug on
    // Ubuntu 24.04: WebView never paints, so the original workaround
    // unconditionally forced GDK_BACKEND=x11 (XWayland path).
    //
    // On WebKit ≥ 2.46 that workaround backfires badly. Measured on
    // an AMD Lucienne iGPU + WebKit 2.50.4 (Ubuntu 24.04, Wayland
    // session): X11 path makes WebKit's GPU detection silently fail,
    // Skia falls back to CPU software rasterization, and four
    // SkiaCPUWorker threads peg ~19% CPU at idle — fan spins up
    // continuously even with no user input. WebKitWebProcess goes
    // from 47% (X11) down to 8% (native Wayland) just by removing the
    // workaround on this WebKit version, because Skia uses DMABUF +
    // Mesa for GPU paint instead.
    //
    // Strategy: detect installed WebKit minor version from the .so
    // file. ≥ 2.46 → leave GDK_BACKEND unset and let GTK pick the
    // session-native backend (Wayland on Wayland sessions, X11 on
    // X11 sessions). Older / undetectable → keep the safe X11
    // fallback so 22.04 / Debian stable users don't regress.
    //
    // Escape hatch: COFFEE_FORCE_X11=1 forces X11 unconditionally,
    // for users who hit a render bug on a specific driver/compositor
    // combo on the modern path.
    //
    // set_var is `unsafe` in recent Rust because of cross-thread
    // races; we're in single-threaded main() before any thread
    // spawns, so it's safe.
    #[cfg(target_os = "linux")]
    unsafe {
        if std::env::var_os("GDK_BACKEND").is_none() {
            let force_x11 = std::env::var_os("COFFEE_FORCE_X11").is_some();
            let needs_x11 = force_x11 || webkit_minor_version().map_or(true, |m| m < 46);
            if needs_x11 {
                std::env::set_var("GDK_BACKEND", "x11");
            }
        }
    }

    // ── Raise fd soft limit (macOS / Linux) ─────────────────────────────
    // macOS defaults RLIMIT_NOFILE soft to 256; a process spawning many
    // concurrent model CLIs (each with sockets + pipes) hits EMFILE fast.
    // Raise the soft limit to min(hard, 1<<20) once at startup — never
    // exceeds the hard limit, best-effort (errors ignored). Windows has
    // no equivalent.
    #[cfg(unix)]
    unsafe {
        let mut rlim: libc::rlimit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) == 0 {
            let target = std::cmp::min(rlim.rlim_max, (1 << 20) as libc::rlim_t);
            if target > rlim.rlim_cur {
                let _ = libc::setrlimit(
                    libc::RLIMIT_NOFILE,
                    &libc::rlimit {
                        rlim_cur: target,
                        rlim_max: rlim.rlim_max,
                    },
                );
            }
        }
    }

    // ── PATH inheritance fix (macOS / Linux) ────────────────────────────
    // GUI apps on macOS / Linux launched from Dock / Finder / .desktop
    // entries get a minimal PATH (typically /usr/bin:/bin:/usr/sbin:/sbin)
    // — they do NOT source the user's interactive shell rc files. So tools
    // installed via Homebrew, nvm, volta, asdf, npm-global, cargo, bun,
    // ~/.local/bin, etc. are invisible to every Command::new() in the
    // process. Symptom: resource detection reports 未安装 even though
    // `claude` / `codex` / `kimi` / `pi` are clearly installed.
    //
    // Fix: ask the user's login shell for its real PATH ONCE at startup
    // and replace the process PATH. Every downstream subprocess
    // (resource detection, model CLI spawns) inherits this and resolves
    // binaries the same way the user's terminal would.
    //
    // We use `-ilc` (interactive + login) so both .zprofile/.bash_profile
    // AND .zshrc/.bashrc are sourced — matches what the user sees when
    // they open a fresh terminal window.
    #[cfg(not(target_os = "windows"))]
    unsafe {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let basename = std::path::Path::new(&shell)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        // fish prints $PATH space-separated, not colon-separated. Use its
        // string-join builtin to emit the same format as POSIX shells.
        let cmd_str = if basename == "fish" {
            "string join : -- $PATH"
        } else {
            "printf '%s' \"$PATH\""
        };
        if let Ok(out) = std::process::Command::new(&shell)
            .args(["-ilc", cmd_str])
            .output()
        {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                // Sanity guard: a real PATH always contains ':'. If the
                // shell rc errored out and we got garbage / empty, keep
                // whatever PATH the OS gave us rather than nuking it.
                if !path.is_empty() && path.contains(':') {
                    std::env::set_var("PATH", path);
                }
            }
        }
    }

    // ── PATH inheritance fix (Windows) ──────────────────────────────────
    // Windows has no login-shell rc to source, but the same class of problem
    // exists as on macOS/Linux: a GUI-launched process can inherit a PATH
    // that is MISSING the per-user dirs where CLI agents install — npm global
    // (%APPDATA%\npm), pnpm, bun, cargo, AND scoop shims, volta, nvm-windows,
    // mise, the Anthropic native installer in ~/.local/bin, … The old fix
    // hardcoded 5 dirs — a guessing treadmill (every new install method needed
    // another entry, and we still missed scoop/volta/nvm). The registry is
    // the source of truth a fresh `cmd.exe` sees, so we read HKCU + HKLM
    // `Path` (auto-expanded) and merge those dirs into the process PATH.
    // Append-only + existence-gated: it can only make more tools resolvable,
    // never removes or shadows anything — a harmless no-op when PATH is
    // already complete.
    #[cfg(target_os = "windows")]
    {
        windows_path::hydrate();
    }

    // CLI subcommand dispatch — short-circuit GUI launch when invoked
    // with a known subcommand (`aicross dispatch/status/adopt/cancel/
    // resources/config/__run`). This is opt-in; double-clicking the
    // executable still gets the GUI (no argv).
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = console_cli::run_if_subcommand(&args) {
        std::process::exit(code);
    }
    // `aicross open <run_id>` 不短路：校验参数与 run 存在性后照常起窗口，
    // 请求随 AppState::pending_open 走；窗口已开时由单实例插件转发（server.rs）。
    // 打开动作只读，不派发、不改文件。退出码：0 已打开 / 2 参数错 /
    // 3 窗口起不来（引擎根不可用、其它读取错误同按 3）；只有 run 文件
    // 不存在按 1 找不到 run（fix-37-r1 R1-7）。
    let pending_open = match run_view::parse_open_args(&args) {
        run_view::OpenParse::NotOpen => None,
        run_view::OpenParse::Invalid(message) => {
            console_cli::attach_parent_console();
            eprintln!("{message}");
            std::process::exit(2);
        }
        run_view::OpenParse::Valid(run_id) => match paths::aicross_root() {
            Ok(root) => match run_view::open_run_exists(&root, &run_id) {
                Ok(true) => Some(run_id),
                Ok(false) => {
                    console_cli::attach_parent_console();
                    eprintln!("找不到 run：{run_id}");
                    std::process::exit(1);
                }
                Err(message) => {
                    console_cli::attach_parent_console();
                    eprintln!("{message}");
                    std::process::exit(3);
                }
            },
            Err(_) => {
                console_cli::attach_parent_console();
                eprintln!("引擎不可用");
                std::process::exit(3);
            }
        },
    };
    // R1-5（fix-37-r1）：宿主里调 `aicross open` 不能挂到窗口关闭——冷启动
    // 以分离方式另起窗口进程（argv 仍是 `open <run_id>`，被拉起的进程照常走
    // pending_open），自己立即退出 0；拉起失败按退出码 3。OPEN_WINDOW_ENV
    // 区分拉起方与被拉起方，避免无限接力。
    if let Some(run_id) = &pending_open {
        if std::env::var_os(run_view::OPEN_WINDOW_ENV).is_none() {
            match run_view::spawn_open_window(run_id) {
                Ok(()) => std::process::exit(0),
                Err(message) => {
                    console_cli::attach_parent_console();
                    eprintln!("{message}");
                    std::process::exit(3);
                }
            }
        } else {
            // 被拉起的窗口进程：清掉标记，不往下传。
            std::env::remove_var(run_view::OPEN_WINDOW_ENV);
        }
    }

    // Default: launch the GUI (首页「最近的派发」，或 `open` 带来的查看页)。
    // 冷启动 `aicross open` 起不来窗口时退出 3；其余路径保持原行为
    // （走到这里的 open 只剩被拉起的窗口进程）。
    let open_requested = pending_open.is_some();
    match server::start_ui(pending_open) {
        Ok(()) => Ok(()),
        Err(error) => {
            if open_requested {
                console_cli::attach_parent_console();
                eprintln!("窗口启动失败：{error}");
                std::process::exit(3);
            }
            Err(error)
        }
    }
}

/// Query the installed WebKit2GTK 4.1 minor version via dlopen +
/// `webkit_get_minor_version()` — WebKit's public C API. Returns
/// e.g. `Some(50)` for WebKit 2.50.x, `Some(46)` for 2.46.x, or
/// `None` if WebKit isn't installed or the symbol can't be resolved.
///
/// We deliberately do NOT parse the `.so` filename: the soversion
/// suffix uses libtool's `current.revision.age` triplet which has
/// no fixed relationship to WebKit's `MAJOR.MINOR.PATCH` (e.g. on
/// Ubuntu 24.04 WebKit 2.50.4 ships as `.so.0.19.7`).
///
/// `dlopen` / `dlsym` are exposed by libc on every glibc system
/// (and merged into libc proper since glibc 2.34), so no extra
/// link flags are required.
#[cfg(target_os = "linux")]
fn webkit_minor_version() -> Option<u32> {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_uint, c_void};

    extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
    }
    const RTLD_LAZY: c_int = 1;
    const RTLD_LOCAL: c_int = 0;

    let lib = CString::new("libwebkit2gtk-4.1.so.0").ok()?;
    let sym = CString::new("webkit_get_minor_version").ok()?;

    unsafe {
        let handle = dlopen(lib.as_ptr(), RTLD_LAZY | RTLD_LOCAL);
        if handle.is_null() {
            return None;
        }
        let func_ptr = dlsym(handle, sym.as_ptr());
        if func_ptr.is_null() {
            dlclose(handle);
            return None;
        }
        let func: extern "C" fn() -> c_uint = std::mem::transmute(func_ptr);
        let minor = func();
        dlclose(handle);
        Some(minor)
    }
}
