mod config;
mod models;
mod pty;
mod sessions;
mod tools;
mod workspace;

use models::{
    ChangeEntry, DiffResult, FileNode, InitialState, SessionEntry, Settings, TerminalRequest,
    ToolDefinition, ToolStatus,
};
use pty::PtyManager;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use workspace::SnapshotStore;

struct AppState {
    project: RwLock<PathBuf>,
    recent: RwLock<Vec<String>>,
    settings: RwLock<Settings>,
    tool_definitions: RwLock<Vec<ToolDefinition>>,
    snapshots: Mutex<SnapshotStore>,
    pty: PtyManager,
}

#[tauri::command]
fn get_initial_state(state: State<'_, AppState>) -> Result<InitialState, String> {
    initial_state(&state)
}

#[tauri::command]
fn detect_tools(state: State<'_, AppState>) -> Result<Vec<ToolStatus>, String> {
    let definitions = state
        .tool_definitions
        .read()
        .map_err(|error| error.to_string())?;
    Ok(tools::detect_all(&definitions))
}

#[tauri::command]
fn set_project(project_path: String, state: State<'_, AppState>) -> Result<InitialState, String> {
    let path = PathBuf::from(&project_path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !path.is_dir() {
        return Err("目标不是目录".to_string());
    }

    state
        .snapshots
        .lock()
        .map_err(|error| error.to_string())?
        .initialize(&path)?;
    *state.project.write().map_err(|error| error.to_string())? = path.clone();

    let path_text = path.to_string_lossy().to_string();
    let mut recent = state.recent.write().map_err(|error| error.to_string())?;
    recent.retain(|entry| entry != &path_text);
    recent.insert(0, path_text);
    recent.truncate(10);
    config::save_recent(&recent)?;
    drop(recent);
    initial_state(&state)
}

#[tauri::command]
fn save_settings(settings: Settings, state: State<'_, AppState>) -> Result<Settings, String> {
    config::save_settings(&settings)?;
    *state.settings.write().map_err(|error| error.to_string())? = settings.clone();
    Ok(settings)
}

#[tauri::command]
fn list_directory(
    relative_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileNode>, String> {
    let project = state.project.read().map_err(|error| error.to_string())?;
    workspace::list_directory(&project, &relative_path)
}

#[tauri::command]
fn open_file(
    relative_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project = state.project.read().map_err(|error| error.to_string())?;
    let path = workspace::safe_file(&project, &relative_path)?;
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn refresh_changes(state: State<'_, AppState>) -> Result<Vec<ChangeEntry>, String> {
    let project = state.project.read().map_err(|error| error.to_string())?;
    state
        .snapshots
        .lock()
        .map_err(|error| error.to_string())?
        .refresh(&project)
}

#[tauri::command]
fn reset_baseline(state: State<'_, AppState>) -> Result<(), String> {
    let project = state.project.read().map_err(|error| error.to_string())?;
    state
        .snapshots
        .lock()
        .map_err(|error| error.to_string())?
        .reset(&project)
}

#[tauri::command]
fn get_file_diff(relative_path: String, state: State<'_, AppState>) -> Result<DiffResult, String> {
    state
        .snapshots
        .lock()
        .map_err(|error| error.to_string())?
        .diff(&relative_path)
}

#[tauri::command]
fn list_sessions() -> Vec<SessionEntry> {
    sessions::list_sessions()
}

#[tauri::command]
fn open_terminal(
    request: TerminalRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let definitions = state
        .tool_definitions
        .read()
        .map_err(|error| error.to_string())?;
    let tool = definitions
        .iter()
        .find(|tool| tool.id == request.tool_id)
        .ok_or_else(|| "未知工具".to_string())?;
    let project = state.project.read().map_err(|error| error.to_string())?;
    let settings = state.settings.read().map_err(|error| error.to_string())?;
    state.pty.open(app, request, tool, &project, &settings)
}

#[tauri::command]
fn write_terminal(
    terminal_id: String,
    data: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.pty.write(&terminal_id, &data)
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
fn close_terminal(
    terminal_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.pty.close(&app, &terminal_id)
}

fn initial_state(state: &State<'_, AppState>) -> Result<InitialState, String> {
    Ok(InitialState {
        project_path: state
            .project
            .read()
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .to_string(),
        recent_projects: state
            .recent
            .read()
            .map_err(|error| error.to_string())?
            .clone(),
        settings: state
            .settings
            .read()
            .map_err(|error| error.to_string())?
            .clone(),
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
            let project = current
                .canonicalize()
                .map_err(|error| std::io::Error::other(format!("解析当前目录失败：{error}")))?;
            let mut snapshots = SnapshotStore::default();
            snapshots
                .initialize(&project)
                .map_err(|error| std::io::Error::other(format!("创建目录快照失败：{error}")))?;
            let debug_data_directory = project.join(".aicross-webview");
            app.manage(AppState {
                project: RwLock::new(project),
                recent: RwLock::new(config::load_recent()),
                settings: RwLock::new(config::load_settings()),
                tool_definitions: RwLock::new(config::load_tools().map_err(|error| {
                    std::io::Error::other(format!("读取工具注册表失败：{error}"))
                })?),
                snapshots: Mutex::new(snapshots),
                pty: PtyManager::default(),
            });
            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("aicross")
                    .inner_size(1400.0, 850.0)
                    .min_inner_size(760.0, 560.0);
            #[cfg(debug_assertions)]
            let window = window.data_directory(debug_data_directory);
            window.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_initial_state,
            detect_tools,
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
