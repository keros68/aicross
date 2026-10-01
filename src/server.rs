// server.rs — 窗口外壳（TASK-40 下线其余界面后的最小形态）。
//
// 窗口只承载三样东西：自绘标题栏（最小化 / 最大化 / 关闭、拖动）与缩放边条、
// 首页「最近的派发」、TASK-37 的查看页。其余旧界面（终端、文件树、控制台对话、
// 对话图、编排、接入页、设置页）已删除，功能由命令行承接：
// 派发 `aicross dispatch/status/adopt/cancel`，资源 `aicross resources`，
// 配置 `aicross config`，查看 `aicross open`。

use std::sync::Mutex;
use tauri::{Manager, State};

/// `aicross open <run_id>` 冷启动随 argv 带进来的 run id。前端挂载后用
/// `take_pending_open` 取一次；热启动走单实例回调的 `open-run-request` 事件，
/// 不经这里。
pub struct AppState {
    pub pending_open: Mutex<Option<String>>,
}

#[tauri::command]
fn window_minimize(window: tauri::Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn window_maximize(window: tauri::Window) {
    let is_max = window.is_maximized().unwrap_or(false);
    if is_max { let _ = window.unmaximize(); } else { let _ = window.maximize(); }
}

#[tauri::command]
fn window_close(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn show_main_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// TASK-37：取走冷启动携带的查看页请求（`aicross open <run_id>` 的 run id），
/// 只取一次；热启动经 `open-run-request` 事件。
#[tauri::command]
fn take_pending_open(state: State<'_, AppState>) -> Option<String> {
    state.pending_open.lock().ok()?.take()
}

/// TASK-40 首页「最近的派发」：读 run 目录，按更新时间倒序取最近 50 条。
/// 引擎根目录不可确定时给空列表，首页按「还没有派发记录」处理。
#[tauri::command]
fn list_runs() -> Vec<crate::run_view::RunListItem> {
    match crate::paths::aicross_root() {
        Ok(root) => crate::run_view::list_runs_at(&root),
        Err(_) => Vec::new(),
    }
}

pub fn start_ui(pending_open: Option<String>) -> anyhow::Result<()> {
    let builder = tauri::Builder::default();

    // Single-instance plugin MUST be the first plugin registered (per
    // Tauri docs) so its argv-forwarding hook runs before any other
    // plugin's init touches state. A second process forwards its argv to
    // this callback and exits; the first process refocuses the window.
    //
    // Release-only: in debug builds we skip the lock so a dev `cargo tauri
    // dev` window can run side-by-side with an installed production build.
    // Both builds otherwise share the bundle identifier, and the lock
    // would silently redirect the dev launch to the production process.
    //
    // `aicross open <run_id>` 的热启动转发：第二个进程已校验参数与 run
    // 存在性，这里把 run id 发给前端切到查看页。事件带递增序号
    // （fix-37-r1 R1-9）；emit 要求载荷 Clone——这段只在发布构建编译
    // （R1-1：开发构建不编它，漏了 derive 会 `cargo check` 过、--release 失败）。
    #[cfg(not(debug_assertions))]
    // 热启动 open 事件的递增序号（fix-37-r1 R1-9）。
    let open_request_seq = std::sync::atomic::AtomicU32::new(0);
    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(move |app, args, _cwd| {
        use tauri::Emitter;
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
        let open_seq = open_request_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if let crate::run_view::OpenParse::Valid(run_id) = crate::run_view::parse_open_args(&args) {
            #[derive(Clone, serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct OpenRunRequest<'a> {
                run_id: &'a str,
                seq: u32,
            }
            let _ = app.emit(
                "open-run-request",
                OpenRunRequest {
                    run_id: &run_id,
                    seq: open_seq,
                },
            );
        }
    }));

    builder
        .manage(AppState {
            pending_open: Mutex::new(pending_open),
        })
        .invoke_handler(tauri::generate_handler![
            window_minimize,
            window_maximize,
            window_close,
            show_main_window,
            take_pending_open,
            list_runs,
            crate::run_view::load_run_view,
            crate::run_view::adopt_run_node,
        ])
        .setup(|app| {
            // ── Bulletproof window-reveal fallback ──────────────────
            // The window is created with `visible: false` so the user
            // never sees the platform's chrome flash — main.tsx
            // invokes `show_main_window` after the first paint via
            // double-RAF and the window appears already-themed.
            //
            // BUT: if the WebView never paints (Gatekeeper rejection
            // on adhoc-signed macOS bundles, WebKit2GTK Wayland blank
            // window on Ubuntu 24.04, or any JS error before
            // ReactDOM mount), the `invoke` never fires and the
            // window stays hidden forever. Force a reveal after 3s
            // as a safety net. Healthy startups call show_main_window
            // in ~50ms, well before this fires.
            {
                use tauri::Manager;
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    if let Some(window) = handle.get_webview_window("main") {
                        if !window.is_visible().unwrap_or(false) {
                            eprintln!(
                                "[main-window] frontend never called show_main_window after 3s — forcing reveal (likely WebView render failure)"
                            );
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                });
            }

            // Force square corners + no shadow on the borderless window.
            // Windows 11's DWM rounds borderless windows by default and adds
            // a subtle drop-shadow; both create the visible "edge ring" we
            // want gone for the flat look.
            #[cfg(target_os = "windows")]
            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_shadow(false);
                    if let Ok(hwnd) = window.hwnd() {
                        unsafe {
                            use windows::Win32::Foundation::HWND;
                            use windows::Win32::Graphics::Dwm::{
                                DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE,
                                DWMWCP_DONOTROUND,
                            };
                            let pref: i32 = DWMWCP_DONOTROUND.0;
                            let _ = DwmSetWindowAttribute(
                                HWND(hwnd.0 as *mut _),
                                DWMWA_WINDOW_CORNER_PREFERENCE,
                                &pref as *const _ as *const _,
                                std::mem::size_of_val(&pref) as u32,
                            );
                        }
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // On macOS a reflexive Cmd+W / red traffic-light click should
            // hide the main window instead of quitting the app (standard
            // Mac convention). The app stays alive; clicking the Dock icon
            // restores it via the Reopen event in `.run()` below.
            // Cmd+Q still terminates normally because NSApplication
            // terminate does not dispatch CloseRequested, so this prevent
            // is never reached on a real quit.
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (window, event);
            }
        })
        .build(tauri::generate_context!())
        .map_err(|e| anyhow::anyhow!("Error while building tauri application: {}", e))?
        .run(|_app_handle, event| {
            // ── macOS: restore the hidden main window on Dock click ──────
            // Pairs with the CloseRequested handler in `.on_window_event`:
            // Cmd+W hides the main window rather than quitting, so on
            // applicationShouldHandleReopen (Dock icon click) we must bring
            // it back ourselves - macOS does not auto-unhide a hidden window
            // on reopen.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = &event {
                use tauri::Manager;
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }

            // ACP 池里可能还挂着引擎拉起的适配器子进程，退出时全部终止，
            // 不留孤儿进程。
            if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                crate::channels::acp::shutdown_all();
            }
        });

    Ok(())
}
