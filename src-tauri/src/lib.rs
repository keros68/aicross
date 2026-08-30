mod config;
mod models;
mod pty;
mod resources;
mod sessions;
mod tools;
mod workspace;

use models::{
    AddManualAgentRequest, AddResourceModelRequest, ChangeEntry, ConnectionOverview, DiffResult,
    FileNode, InitialState, ResourceModelKey, ResourceOperation, ResourceStore, SessionEntry,
    Settings, SmokeSelectionRequest, TerminalRequest, ToolDefinition, ToolStatus,
    UpdateResourceModelRequest,
};
use pty::PtyManager;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;
use workspace::SnapshotStore;

struct AppState {
    project: RwLock<PathBuf>,
    recent: RwLock<Vec<String>>,
    settings: RwLock<Settings>,
    tool_definitions: RwLock<Vec<ToolDefinition>>,
    resources: RwLock<ResourceStore>,
    warnings: Vec<String>,
    snapshots: Arc<Mutex<SnapshotStore>>,
    pty: Arc<PtyManager>,
}

#[tauri::command]
fn get_initial_state(state: State<'_, AppState>) -> Result<InitialState, String> {
    initial_state(&state)
}

#[tauri::command]
async fn detect_tools(state: State<'_, AppState>) -> Result<Vec<ToolStatus>, String> {
    let definitions = read_lock(&state.tool_definitions, "读取工具注册表")?.clone();
    run_blocking("检测工具", move || Ok(tools::detect_all(&definitions))).await
}

#[tauri::command]
fn get_resources(state: State<'_, AppState>) -> Result<ResourceStore, String> {
    Ok(read_lock(&state.resources, "读取资源清单")?.clone())
}

#[tauri::command]
async fn detect_resource_agent(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<ResourceOperation, String> {
    let store = read_lock(&state.resources, "读取资源清单")?.clone();
    let definitions = read_lock(&state.tool_definitions, "读取工具注册表")?.clone();
    let settings = read_lock(&state.settings, "读取设置")?.clone();
    let operation = run_blocking("探测资源", move || {
        let operation = resources::detect_one(store, &agent_id, &definitions, &settings)?;
        config::save_resources(&operation.store)?;
        Ok(operation)
    })
    .await?;
    *write_lock(&state.resources, "更新资源清单")? = operation.store.clone();
    Ok(operation)
}

#[tauri::command]
async fn detect_all_resources(
    agent_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<ResourceOperation, String> {
    let store = read_lock(&state.resources, "读取资源清单")?.clone();
    let definitions = read_lock(&state.tool_definitions, "读取工具注册表")?.clone();
    let settings = read_lock(&state.settings, "读取设置")?.clone();
    let operation = run_blocking("探测全部资源", move || {
        let operation = resources::detect_many(store, &agent_ids, &definitions, &settings);
        config::save_resources(&operation.store)?;
        Ok(operation)
    })
    .await?;
    *write_lock(&state.resources, "更新资源清单")? = operation.store.clone();
    Ok(operation)
}

#[tauri::command]
async fn add_resource_model(
    request: AddResourceModelRequest,
    state: State<'_, AppState>,
) -> Result<ResourceStore, String> {
    update_resources(&state, "手动输入模型", move |store| {
        resources::add_model(store, request)
    })
    .await
}

#[tauri::command]
async fn update_resource_model(
    request: UpdateResourceModelRequest,
    state: State<'_, AppState>,
) -> Result<ResourceStore, String> {
    update_resources(&state, "更新模型资源", move |store| {
        resources::update_model(store, request)
    })
    .await
}

#[tauri::command]
async fn remove_resource_model(
    request: ResourceModelKey,
    state: State<'_, AppState>,
) -> Result<ResourceStore, String> {
    update_resources(&state, "移除模型资源", move |store| {
        resources::remove_model(store, request)
    })
    .await
}

#[tauri::command]
async fn add_manual_agent(
    request: AddManualAgentRequest,
    state: State<'_, AppState>,
) -> Result<ResourceStore, String> {
    let definition = resources::manual_agent_definition(request)?;
    let mut store = read_lock(&state.resources, "读取资源清单")?.clone();
    let definitions = run_blocking("手动添加 agent", move || {
        config::add_tool_override(definition)
    })
    .await?;
    resources::ensure_agents(&mut store, &definitions);
    let saved = store.clone();
    run_blocking("保存资源清单", move || config::save_resources(&saved)).await?;
    *write_lock(&state.tool_definitions, "更新工具注册表")? = definitions;
    *write_lock(&state.resources, "更新资源清单")? = store.clone();
    Ok(store)
}

#[tauri::command]
async fn smoke_resource_model(
    request: ResourceModelKey,
    state: State<'_, AppState>,
) -> Result<ResourceStore, String> {
    let store = read_lock(&state.resources, "读取资源清单")?.clone();
    let definitions = read_lock(&state.tool_definitions, "读取工具注册表")?.clone();
    let settings = read_lock(&state.settings, "读取设置")?.clone();
    let updated = run_blocking("模型冒烟", move || {
        let updated = resources::smoke_one(store, &request, &definitions, &settings)?;
        config::save_resources(&updated)?;
        Ok(updated)
    })
    .await?;
    *write_lock(&state.resources, "更新资源清单")? = updated.clone();
    Ok(updated)
}

#[tauri::command]
async fn smoke_selected_resources(
    request: SmokeSelectionRequest,
    state: State<'_, AppState>,
) -> Result<ResourceOperation, String> {
    let store = read_lock(&state.resources, "读取资源清单")?.clone();
    let definitions = read_lock(&state.tool_definitions, "读取工具注册表")?.clone();
    let settings = read_lock(&state.settings, "读取设置")?.clone();
    let operation = run_blocking("所选资源冒烟", move || {
        let operation = resources::smoke_selected(store, &request, &definitions, &settings);
        config::save_resources(&operation.store)?;
        Ok(operation)
    })
    .await?;
    *write_lock(&state.resources, "更新资源清单")? = operation.store.clone();
    Ok(operation)
}

#[tauri::command]
async fn get_connection_overview(state: State<'_, AppState>) -> Result<ConnectionOverview, String> {
    let settings = read_lock(&state.settings, "读取设置")?.clone();
    run_blocking("读取接入资源", move || {
        resources::connection_overview(&settings)
    })
    .await
}

#[tauri::command]
async fn set_project(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<InitialState, String> {
    let recent = read_lock(&state.recent, "读取最近项目")?.clone();
    let (path, snapshots, recent) = run_blocking("打开项目", move || {
        let path = dunce::canonicalize(PathBuf::from(&project_path))
            .map_err(|error| format!("解析项目目录失败：{error}"))?;
        if !path.is_dir() {
            return Err("打开项目失败：目标不是目录".to_string());
        }

        let mut snapshots = SnapshotStore::default();
        snapshots.initialize(&path)?;
        let path_text = path.to_string_lossy().to_string();
        let mut recent = recent;
        recent.retain(|entry| entry != &path_text);
        recent.insert(0, path_text);
        recent.truncate(10);
        config::save_recent(&recent)?;
        Ok((path, snapshots, recent))
    })
    .await?;

    *mutex_lock(&state.snapshots, "更新目录快照")? = snapshots;
    *write_lock(&state.project, "更新项目目录")? = path;
    *write_lock(&state.recent, "更新最近项目")? = recent;
    initial_state(&state)
}

#[tauri::command]
async fn save_settings(settings: Settings, state: State<'_, AppState>) -> Result<Settings, String> {
    let saved = settings.clone();
    run_blocking("保存设置", move || config::save_settings(&saved)).await?;
    *write_lock(&state.settings, "更新设置")? = settings.clone();
    Ok(settings)
}

#[tauri::command]
async fn list_directory(
    relative_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileNode>, String> {
    let project = read_lock(&state.project, "读取项目目录")?.clone();
    run_blocking("列出目录", move || {
        workspace::list_directory(&project, &relative_path)
    })
    .await
}

#[tauri::command]
async fn open_file(
    relative_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project = read_lock(&state.project, "读取项目目录")?.clone();
    let path = run_blocking("检查文件路径", move || {
        workspace::safe_file(&project, &relative_path)
    })
    .await?;

    if requires_open_confirmation(&path) {
        let dialog_app = app.clone();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("该文件")
            .to_string();
        let confirmed = run_blocking("显示打开确认", move || {
            Ok(dialog_app
                .dialog()
                .message(format!("{file_name} 可能执行程序或脚本。是否继续打开？"))
                .title("确认打开")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "继续".to_string(),
                    "取消".to_string(),
                ))
                .blocking_show())
        })
        .await?;
        if !confirmed {
            return Ok(());
        }
    }

    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| format!("打开文件失败：{error}"))
}

#[tauri::command]
async fn refresh_changes(state: State<'_, AppState>) -> Result<Vec<ChangeEntry>, String> {
    let project = read_lock(&state.project, "读取项目目录")?.clone();
    let snapshots = Arc::clone(&state.snapshots);
    run_blocking("刷新修改记录", move || {
        mutex_lock(&snapshots, "读取目录快照")?.refresh(&project)
    })
    .await
}

#[tauri::command]
async fn reset_baseline(state: State<'_, AppState>) -> Result<(), String> {
    let project = read_lock(&state.project, "读取项目目录")?.clone();
    let snapshots = Arc::clone(&state.snapshots);
    run_blocking("重置基线", move || {
        mutex_lock(&snapshots, "读取目录快照")?.reset(&project)
    })
    .await
}

#[tauri::command]
async fn get_file_diff(
    relative_path: String,
    state: State<'_, AppState>,
) -> Result<DiffResult, String> {
    let snapshots = Arc::clone(&state.snapshots);
    run_blocking("生成文件差异", move || {
        mutex_lock(&snapshots, "读取目录快照")?.diff(&relative_path)
    })
    .await
}

#[tauri::command]
async fn list_sessions() -> Result<Vec<SessionEntry>, String> {
    run_blocking("读取会话记录", || Ok(sessions::list_sessions())).await
}

#[tauri::command]
async fn open_terminal(
    request: TerminalRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let tool = read_lock(&state.tool_definitions, "读取工具注册表")?
        .iter()
        .find(|tool| tool.id == request.tool_id)
        .cloned()
        .ok_or_else(|| "启动终端失败：未知工具".to_string())?;
    let project = read_lock(&state.project, "读取项目目录")?.clone();
    let settings = read_lock(&state.settings, "读取设置")?.clone();
    let pty = Arc::clone(&state.pty);
    run_blocking("启动终端", move || {
        pty.open(app, request, &tool, &project, &settings)
    })
    .await
}

#[tauri::command]
async fn write_terminal(
    terminal_id: String,
    data: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pty = Arc::clone(&state.pty);
    run_blocking("写入终端", move || pty.write(&terminal_id, &data)).await
}

#[tauri::command]
fn resize_terminal(
    terminal_id: String,
    rows: u16,
    cols: u16,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.pty.resize(&terminal_id, rows, cols)
}

#[tauri::command]
async fn close_terminal(
    terminal_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pty = Arc::clone(&state.pty);
    run_blocking("关闭终端", move || pty.close(&app, &terminal_id)).await
}

fn initial_state(state: &State<'_, AppState>) -> Result<InitialState, String> {
    Ok(InitialState {
        project_path: read_lock(&state.project, "读取项目目录")?
            .to_string_lossy()
            .to_string(),
        recent_projects: read_lock(&state.recent, "读取最近项目")?.clone(),
        settings: read_lock(&state.settings, "读取设置")?.clone(),
        warnings: state.warnings.clone(),
    })
}

fn read_lock<'a, T>(
    lock: &'a RwLock<T>,
    operation: &str,
) -> Result<RwLockReadGuard<'a, T>, String> {
    lock.read()
        .map_err(|error| format!("{operation}失败：{error}"))
}

fn write_lock<'a, T>(
    lock: &'a RwLock<T>,
    operation: &str,
) -> Result<RwLockWriteGuard<'a, T>, String> {
    lock.write()
        .map_err(|error| format!("{operation}失败：{error}"))
}

fn mutex_lock<'a, T>(lock: &'a Mutex<T>, operation: &str) -> Result<MutexGuard<'a, T>, String> {
    lock.lock()
        .map_err(|error| format!("{operation}失败：{error}"))
}

async fn run_blocking<T, F>(operation: &'static str, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| format!("{operation}线程失败：{error}"))?
}

async fn update_resources<F>(
    state: &State<'_, AppState>,
    operation: &'static str,
    update: F,
) -> Result<ResourceStore, String>
where
    F: FnOnce(ResourceStore) -> Result<ResourceStore, String> + Send + 'static,
{
    let store = read_lock(&state.resources, "读取资源清单")?.clone();
    let updated = run_blocking(operation, move || {
        let updated = update(store)?;
        config::save_resources(&updated)?;
        Ok(updated)
    })
    .await?;
    *write_lock(&state.resources, "更新资源清单")? = updated.clone();
    Ok(updated)
}

fn requires_open_confirmation(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "exe" | "bat" | "cmd" | "ps1" | "msi"
            )
        })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let current = std::env::current_dir()
                .map_err(|error| std::io::Error::other(format!("读取当前目录失败：{error}")))?;
            let project = dunce::canonicalize(current)
                .map_err(|error| std::io::Error::other(format!("解析当前目录失败：{error}")))?;
            let mut snapshots = SnapshotStore::default();
            snapshots
                .initialize(&project)
                .map_err(|error| std::io::Error::other(format!("创建目录快照失败：{error}")))?;
            let recent = config::load_recent().map_err(std::io::Error::other)?;
            let settings = config::load_settings().map_err(std::io::Error::other)?;
            let tools = config::load_tools()
                .map_err(|error| std::io::Error::other(format!("读取工具注册表失败：{error}")))?;
            let mut resources = config::load_resources()
                .map_err(|error| std::io::Error::other(format!("读取资源清单失败：{error}")))?;
            resources::ensure_agents(&mut resources.value, &tools.value);
            let warnings = [
                recent.warning,
                settings.warning,
                tools.warning,
                resources.warning,
            ]
            .into_iter()
            .flatten()
            .collect();
            app.manage(AppState {
                project: RwLock::new(project),
                recent: RwLock::new(recent.value),
                settings: RwLock::new(settings.value),
                tool_definitions: RwLock::new(tools.value),
                resources: RwLock::new(resources.value),
                warnings,
                snapshots: Arc::new(Mutex::new(snapshots)),
                pty: Arc::new(PtyManager::default()),
            });
            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("aicross")
                    .inner_size(1400.0, 850.0)
                    .min_inner_size(760.0, 560.0);
            #[cfg(debug_assertions)]
            let window = window.data_directory(
                app.path()
                    .app_local_data_dir()
                    .map_err(|error| {
                        std::io::Error::other(format!("确定 WebView 数据目录失败：{error}"))
                    })?
                    .join("webview-debug"),
            );
            window.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_initial_state,
            detect_tools,
            get_resources,
            detect_resource_agent,
            detect_all_resources,
            add_resource_model,
            update_resource_model,
            remove_resource_model,
            add_manual_agent,
            smoke_resource_model,
            smoke_selected_resources,
            get_connection_overview,
            set_project,
            save_settings,
            list_directory,
            open_file,
            refresh_changes,
            reset_baseline,
            get_file_diff,
            list_sessions,
            open_terminal,
            write_terminal,
            resize_terminal,
            close_terminal
        ])
        .run(tauri::generate_context!())
        .expect("aicross failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_extensions_require_confirmation_case_insensitively() {
        for path in ["tool.exe", "script.BAT", "run.cmd", "setup.ps1", "pkg.msi"] {
            assert!(requires_open_confirmation(Path::new(path)), "{path}");
        }
        assert!(!requires_open_confirmation(Path::new("notes.txt")));
    }
}
