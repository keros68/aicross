// server.rs — 窗口：启动、窗口按钮，以及各页调用的命令。命令的实现体在
// console_cli 与 run_view，与命令行共用。

use std::sync::Mutex;
use tauri::{Manager, State};

/// `aicross open <run_id>` 冷启动随 argv 带进来的 run id。前端挂载后用
/// `take_pending_open` 取一次；热启动走单实例回调的 `open-run-request` 事件，
/// 不经这里。
pub struct AppState {
    pub pending_open: Mutex<Option<String>>,
}

/// 自绘标题栏的三个按钮。
#[tauri::command]
fn window_action(app: tauri::AppHandle, window: tauri::Window, action: String) -> Result<(), String> {
    let result = match action.as_str() {
        "minimize" => window.minimize(),
        "toggle-maximize" => match window.is_maximized() {
            Ok(true) => window.unmaximize(),
            _ => window.maximize(),
        },
        "close" => {
            app.exit(0);
            Ok(())
        }
        other => return Err(format!("未知的窗口操作：{other}")),
    };
    result.map_err(|error| error.to_string())
}

/// 前端首帧画完后调用；窗口创建时是隐藏的，避免先露出空白窗口。
#[tauri::command]
fn show_main_window(app: tauri::AppHandle) {
    bring_main_window_to_front(&app);
}

fn bring_main_window_to_front<R: tauri::Runtime>(app: &impl Manager<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.show();
    let _ = window.set_focus();
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

// ── TASK-46：资源与模式页的窗口命令 ──────────────────────────────────
//
// 返回值与对应命令行的 stdout JSON 同一结构，错误时返回 stderr 那行说明。
// 实现体在 console_cli（与命令行同一份函数），这里只包引擎根目录。
// fix-46-r1 R1：六个命令全部放阻塞线程池（spawn_blocking）——探测与冒烟
// 单次最长 180 秒，快照要拉 where.exe 查 ACP 状态，config 的读写会等资源库
// 锁（冒烟全程持有同一把锁），都不能占窗口主线程。
// TASK-50：命令带主 agent（host）；缺省时按命令行同一套规则推定（单一
// 配置直接用，零或多于一个时报错）。写命令的手选人选 origin 一律为
// manual（同一实现体）。

/// 解析窗口命令的 host 参数：与命令行同一套规则；写命令没有配置时也要
/// 有具体 host。
fn resolve_window_host(
    root: &std::path::Path,
    host: Option<String>,
    for_write: bool,
) -> Result<String, String> {
    let resolved = if for_write {
        crate::console_cli::resolve_host_for_write(root, host.as_deref()).map(Some)
    } else {
        crate::console_cli::resolve_host_or_err(root, host.as_deref())
    };
    match resolved {
        Ok(Some(host)) => Ok(host),
        Ok(None) => Ok(String::new()),
        Err((_, message)) => Err(message),
    }
}

/// 资源与配置快照（`aicross resources` + `aicross config show --host` 同
/// 结构）。host 按命令行同一套规则解析；host 为 null 且一份配置都没有时
/// 返回资源与空配置（资源页在首次探测前就要能读到快照）；多于一份配置
/// 且未给 host 时与命令行一致报错。
#[tauri::command]
async fn config_snapshot(host: Option<String>) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::console_cli::config_snapshot_at(&root, host.as_deref(), &crate::channels::acp_setup::adapter_status_cached())
    })
    .await
    .map_err(|error| format!("读取配置任务失败：{error}"))?
}

/// 检测到的主 agent 清单（`aicross hosts` 同结构）。
#[tauri::command]
async fn hosts_list() -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let home = crate::paths::home()?;
        let env_base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
        let hosts = crate::hosts::list_hosts_at(&root, &home, env_base_url.as_deref())?;
        serde_json::to_value(hosts).map_err(|error| format!("生成主 agent 清单失败：{error}"))
    })
    .await
    .map_err(|error| format!("读取主 agent 任务失败：{error}"))?
}

/// 设定或清除主 agent 的厂商覆盖；vendor 为 auto 表示清除。
#[tauri::command]
async fn host_set_vendor(host: String, vendor: String) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::hosts::set_vendor_override_at(&root, &host, &vendor)
            .map_err(|(_, message)| message)
            .and_then(|info| {
                serde_json::to_value(info).map_err(|error| format!("生成主 agent 条目失败：{error}"))
            })
    })
    .await
    .map_err(|error| format!("写入厂商设定任务失败：{error}"))?
}

/// 探测全部 agent；真实探测发模型请求，放阻塞线程池跑，不卡窗口主线程。
/// 每个 agent 探测完发一次 `resources-detect-progress` 事件（agent / done /
/// total，走查 #3），前端据此显示探测进度。
#[tauri::command]
async fn resources_detect(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Emitter;
        crate::console_cli::resources_detect_command_with_progress(&root, &|agent, done, total| {
            let _ = app.emit(
                "resources-detect-progress",
                serde_json::json!({ "agent": agent, "done": done, "total": total }),
            );
        })
    })
    .await
    .map_err(|error| format!("探测任务失败：{error}"))?
}

/// 冒烟一个模型（给了 model）或一个 agent 的全部第三方模型；单次最长
/// 180 秒，放阻塞线程池跑，不卡窗口主线程。
#[tauri::command]
async fn resources_smoke(
    agent: String,
    model: Option<String>,
    provider: Option<String>,
) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::console_cli::resources_smoke_command_at(&root, &agent, model.as_deref(), provider.as_deref())
    })
    .await
    .map_err(|error| format!("冒烟任务失败：{error}"))?
}

/// 清理最近一次探测未发现的模型（TASK-52，`aicross resources prune` 同
/// 实现体）；agent 限定只清这一家，缺省清全部。放阻塞线程池跑，不卡
/// 窗口主线程。
#[tauri::command]
async fn resources_prune(agent: Option<String>) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::console_cli::resources_prune_command_at(&root, agent.as_deref())
    })
    .await
    .map_err(|error| format!("清理任务失败：{error}"))?
}

/// stage、index 从 1 开始数（与命令行一致）；seat 是 modes.json 的人选对象。
#[tauri::command]
async fn config_set_default_mode(
    host: Option<String>,
    mode: String,
) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let host = resolve_window_host(&root, host, true)?;
        crate::console_cli::config_set_default_mode_at(&root, &host, &mode)
            .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("写入配置任务失败：{error}"))?
}

/// stage、index 从 1 开始数（与命令行一致）；seat 是 modes.json 的人选对象。
#[tauri::command]
async fn config_set_seat(
    host: Option<String>,
    mode: String,
    stage: u32,
    index: u32,
    seat: crate::orchestration::SeatChoice,
) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let host = resolve_window_host(&root, host, true)?;
        crate::console_cli::config_set_seat_at(&root, &host, &mode, stage as usize, index as usize, seat)
            .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("写入配置任务失败：{error}"))?
}

/// 设定内置模式可调角色的位置数量（`aicross config set-count` 同实现体）；
/// 不可调的模式或越界时报错说明范围。
#[tauri::command]
async fn config_set_count(
    host: Option<String>,
    mode: String,
    count: u32,
) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let host = resolve_window_host(&root, host, true)?;
        crate::console_cli::config_set_count_at(&root, &host, &mode, count)
            .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("写入配置任务失败：{error}"))?
}

/// 设定评审模式某位置的视角（`aicross config set-focus` 同实现体）；
/// index 从 1 起，focus 为空串恢复该位置的默认。返回该模式那一项（同
/// `config show`）。
#[tauri::command]
async fn config_set_focus(
    host: Option<String>,
    mode: String,
    index: u32,
    focus: String,
) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let host = resolve_window_host(&root, host, true)?;
        crate::console_cli::config_set_focus_at(&root, &host, &mode, index as usize, &focus)
            .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("写入配置任务失败：{error}"))?
}

/// 给了 stage 与 index 只清这一个位置；都不给清掉该模式全部人选。
#[tauri::command]
async fn config_clear_seat(
    host: Option<String>,
    mode: String,
    stage: Option<u32>,
    index: Option<u32>,
) -> Result<serde_json::Value, String> {
    if stage.is_some() != index.is_some() {
        return Err("阶段与位置要成对给".into());
    }
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let host = resolve_window_host(&root, host, true)?;
        crate::console_cli::config_clear_seat_at(
            &root,
            &host,
            &mode,
            stage.map(|value| value as usize),
            index.map(|value| value as usize),
        )
        .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("写入配置任务失败：{error}"))?
}

/// 一键推荐（`aicross config recommend` 同实现体）；all 为全部重填，
/// 窗口里需用户确认后才传。
#[tauri::command]
async fn config_recommend(
    host: Option<String>,
    mode: Option<String>,
    all: bool,
) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::recommend::recommend_at(&root, host.as_deref(), mode.as_deref(), all)
            .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("推荐任务失败：{error}"))?
}

/// 短命令清单（`aicross slash list` 同结构）。三个短命令都需要具体
/// host（文件写在家目录下，按 host 分路径），按写命令同一套规则解析。
#[tauri::command]
async fn slash_list(host: Option<String>) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let home = crate::paths::home()?;
        let host = resolve_window_host(&root, host, true)?;
        crate::slash::slash_list_at(&root, &home, &host).map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("读取短命令任务失败：{error}"))?
}

/// 添加或更新短命令（`aicross slash add` 同实现体）；note 可缺省。
#[tauri::command]
async fn slash_add(
    host: Option<String>,
    name: String,
    mode: String,
    note: Option<String>,
) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let home = crate::paths::home()?;
        let host = resolve_window_host(&root, host, true)?;
        crate::slash::slash_add_at(&root, &home, &host, &name, &mode, note.as_deref().unwrap_or_default())
            .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("写入短命令任务失败：{error}"))?
}

/// 一键生成内置模式的短命令（`aicross slash add-all` 同实现体）；prefix
/// 缺省 ax-。已有该模式命令的模式跳过，单个失败不中断，连同原因记入
/// 输出的 skipped。
#[tauri::command]
async fn slash_add_all(
    host: Option<String>,
    prefix: Option<String>,
) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let home = crate::paths::home()?;
        let host = resolve_window_host(&root, host, true)?;
        crate::slash::slash_add_all_at(
            &root,
            &home,
            &host,
            prefix.as_deref().unwrap_or(crate::slash::DEFAULT_PREFIX),
        )
        .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("写入短命令任务失败：{error}"))?
}

/// 移除短命令（`aicross slash remove` 同实现体，只删带标记的文件）。
#[tauri::command]
async fn slash_remove(host: Option<String>, name: String) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let home = crate::paths::home()?;
        let host = resolve_window_host(&root, host, true)?;
        crate::slash::slash_remove_at(&root, &home, &host, &name).map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("移除短命令任务失败：{error}"))?
}

/// 保存自定义模式（`aicross preset save` 同实现体）；preset 是一个预设
/// 对象，格式同命令行的 --file。返回保存后的预设与 modes（同 config show），
/// host 按命令行同一套规则解析。
#[tauri::command]
async fn preset_save(
    host: Option<String>,
    preset: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let host = resolve_window_host(&root, host, false)?;
        let input: crate::preset::PresetInput = serde_json::from_value(preset)
            .map_err(|error| format!("预设对象无法解析：{error}"))?;
        crate::preset::preset_save_at(
            &root,
            if host.is_empty() { None } else { Some(host.as_str()) },
            &input,
        )
        .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("保存模式任务失败：{error}"))?
}

/// 删除自定义模式（`aicross preset remove` 同实现体）；同时清理各主
/// agent 下该模式的人选、指向它的默认模式与短命令定义、对应的带标记
/// 文件。返回删除结果与受影响项列表。
#[tauri::command]
async fn preset_remove(id: String) -> Result<serde_json::Value, String> {
    let root = crate::paths::aicross_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let home = crate::paths::home()?;
        crate::preset::preset_remove_at(&root, &home, &id).map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("移除模式任务失败：{error}"))?
}

/// skill 安装状态；project 是项目目录（缺省只看全局）。
#[tauri::command]
async fn skill_status(project: Option<String>) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<serde_json::Value, String> {
        let home = crate::paths::home()?;
        let project = project.map(std::path::PathBuf::from);
        crate::skill_install::status_at(&home, project.as_deref())
            .map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| format!("读取 skill 状态任务失败：{error}"))?
}

/// 装 skill（scope：global | project）。
#[tauri::command]
async fn skill_install(
    host: String,
    scope: String,
    project: Option<String>,
) -> Result<serde_json::Value, String> {
    skill_change(host, scope, project, true).await
}

/// 卸 skill（只删我们装的那份）。
#[tauri::command]
async fn skill_uninstall(
    host: String,
    scope: String,
    project: Option<String>,
) -> Result<serde_json::Value, String> {
    skill_change(host, scope, project, false).await
}

async fn skill_change(
    host: String,
    scope: String,
    project: Option<String>,
    install: bool,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<serde_json::Value, String> {
        let home = crate::paths::home()?;
        let project = project.map(std::path::PathBuf::from);
        let outcome = if install {
            crate::skill_install::install_at(&home, &host, &scope, project.as_deref())
        } else {
            crate::skill_install::uninstall_at(&home, &host, &scope, project.as_deref())
        };
        outcome.map_err(|(_, message)| message)
    })
    .await
    .map_err(|error| if install {
        format!("skill 安装任务失败：{error}")
    } else {
        format!("卸载任务失败：{error}")
    })?
}

/// 弹出系统选目录对话框（TASK-51 的项目范围安装用）；取消返回 null。
/// 只在 Rust 侧调用（tauri-plugin-dialog），前端不装 npm 包。对话框在
/// 阻塞线程上弹出，不占窗口主线程。
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        Ok(app
            .dialog()
            .file()
            .blocking_pick_folder()
            .and_then(|file| file.into_path().ok())
            .map(|path| path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|error| format!("打开目录选择失败：{error}"))?
}
pub fn start_ui(pending_open: Option<String>) -> anyhow::Result<()> {
    let builder = tauri::Builder::default();

    // 单实例只在发布构建启用，开发窗口可以与已安装的版本同时开。插件要求
    // 第一个注册。第二个进程把命令行转给这里后退出：这里把已开的窗口带到
    // 前台；命令行是 `open <run_id>` 时再发 `open-run-request` 让前端进查看页，
    // seq 逐次加一，前端据此区分两次相同 run id 的请求。
    #[cfg(not(debug_assertions))]
    let builder = {
        let seq = std::sync::atomic::AtomicU32::new(0);
        builder.plugin(tauri_plugin_single_instance::init(move |app, args, _cwd| {
            use tauri::Emitter;
            bring_main_window_to_front(app);
            let seq = seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if let crate::run_view::OpenParse::Valid(run_id) = crate::run_view::parse_open_args(&args) {
                let _ = app.emit("open-run-request", serde_json::json!({ "runId": run_id, "seq": seq }));
            }
        }))
    };

    let builder = builder.plugin(tauri_plugin_dialog::init());

    builder
        .manage(AppState {
            pending_open: Mutex::new(pending_open),
        })
        .invoke_handler(tauri::generate_handler![
            window_action,
            show_main_window,
            take_pending_open,
            list_runs,
            config_snapshot,
            resources_detect,
            resources_smoke,
            resources_prune,
            config_set_default_mode,
            config_set_seat,
            config_set_count,
            config_set_focus,
            config_clear_seat,
            hosts_list,
            host_set_vendor,
            config_recommend,
            slash_list,
            slash_add,
            slash_add_all,
            slash_remove,
            preset_save,
            preset_remove,
            skill_status,
            skill_install,
            skill_uninstall,
            pick_folder,
            crate::run_view::load_run_view,
            crate::run_view::adopt_run_node,
        ])
        .setup(|app| {
            // 前端出错画不出首帧时 show_main_window 不会被调用，3 秒后照样显示窗口。
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(3));
                let hidden = handle
                    .get_webview_window("main")
                    .is_some_and(|window| !window.is_visible().unwrap_or(true));
                if hidden {
                    eprintln!("窗口 3 秒内未显示，前端可能加载失败，改为直接显示");
                    bring_main_window_to_front(&handle);
                }
            });

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                square_corners(&window);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // macOS 习惯：关窗只隐藏，程序留在 Dock；Cmd+Q 照常退出。
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (window, event);
        })
        .build(tauri::generate_context!())
        .map_err(|error| anyhow::anyhow!("窗口初始化失败：{error}"))?
        .run(|app, event| match event {
            // macOS 点 Dock 图标时把隐藏的窗口显示回来。
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => bring_main_window_to_front(app),
            // 退出时终止 ACP 池里引擎拉起的适配器子进程。
            tauri::RunEvent::ExitRequested { .. } => {
                let _ = app;
                crate::channels::acp::shutdown_all();
            }
            _ => {}
        });

    Ok(())
}

/// Windows 11 默认给无边框窗口加圆角和阴影，这里改成直角、去掉阴影。
#[cfg(target_os = "windows")]
fn square_corners(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND};

    let _ = window.set_shadow(false);
    let Ok(handle) = window.hwnd() else {
        return;
    };
    let preference = DWMWCP_DONOTROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            HWND(handle.0 as _),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::addr_of!(preference).cast(),
            std::mem::size_of_val(&preference) as u32,
        );
    }
}
