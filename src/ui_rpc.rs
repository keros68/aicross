// ui_rpc.rs — 原生 macOS SwiftUI（Liquid Glass）面板的 JSON 桥接入口（`aicross __ui <cmd> [json]`），
// 与命令行及 Tauri 窗口复用同一份 console_cli / hosts / recommend / slash / preset / skill_install / track / run_view 实现体。

use serde_json::{json, Map, Value};
use std::path::Path;

fn resolve_window_host(
    root: &Path,
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

pub fn handle_ui_rpc(method: &str, payload: &Value) -> Result<Value, String> {
    match method {
        "app_version" => Ok(Value::String(env!("CARGO_PKG_VERSION").to_string())),

        "list_runs" => {
            let items = match crate::paths::aicross_root() {
                Ok(root) => crate::run_view::list_runs_at(&root),
                Err(_) => Vec::new(),
            };
            serde_json::to_value(items).map_err(|e| format!("序列化派发列表失败：{e}"))
        }

        "load_run_view" => {
            let run_id = payload.get("runId").and_then(Value::as_str).unwrap_or_default().to_string();
            let view = crate::run_view::load_run_view(run_id)?;
            serde_json::to_value(view).map_err(|e| format!("序列化查看页失败：{e}"))
        }

        "adopt_run_node" => {
            let run_id = payload.get("runId").and_then(Value::as_str).unwrap_or_default().to_string();
            let node_id = payload.get("nodeId").and_then(Value::as_str).unwrap_or_default().to_string();
            let view = crate::run_view::adopt_run_node(run_id, node_id)?;
            serde_json::to_value(view).map_err(|e| format!("序列化采纳结果失败：{e}"))
        }

        "config_snapshot" => {
            let root = crate::paths::aicross_root()?;
            let host = payload.get("host").and_then(Value::as_str);
            crate::console_cli::config_snapshot_at(
                &root,
                host,
                &crate::channels::acp_setup::adapter_status_cached(),
            )
        }

        "hosts_list" => {
            let root = crate::paths::aicross_root()?;
            let home = crate::paths::home()?;
            let env_base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
            let hosts = crate::hosts::list_hosts_at(&root, &home, env_base_url.as_deref())?;
            serde_json::to_value(hosts).map_err(|e| format!("生成主 agent 清单失败：{e}"))
        }

        "host_set_vendor" => {
            let root = crate::paths::aicross_root()?;
            let host = payload.get("host").and_then(Value::as_str).unwrap_or_default();
            let vendor = payload.get("vendor").and_then(Value::as_str).unwrap_or("auto");
            let info = crate::hosts::set_vendor_override_at(&root, host, vendor).map_err(|(_, m)| m)?;
            serde_json::to_value(info).map_err(|e| format!("生成主 agent 条目失败：{e}"))
        }

        "resources_detect" => {
            let root = crate::paths::aicross_root()?;
            crate::console_cli::resources_detect_command_with_progress(&root, &|agent, done, total| {
                eprintln!(
                    "AICROSS_PROGRESS:{}",
                    json!({ "agent": agent, "done": done, "total": total })
                );
            })
        }

        "resources_smoke" => {
            let root = crate::paths::aicross_root()?;
            let agent = payload.get("agent").and_then(Value::as_str).unwrap_or_default();
            let model = payload.get("model").and_then(Value::as_str);
            let provider = payload.get("provider").and_then(Value::as_str);
            crate::console_cli::resources_smoke_command_at(&root, agent, model, provider)
        }

        "resources_prune" => {
            let root = crate::paths::aicross_root()?;
            let agent = payload.get("agent").and_then(Value::as_str);
            crate::console_cli::resources_prune_command_at(&root, agent)
        }

        "resources_set_path" => {
            let root = crate::paths::aicross_root()?;
            let agent = payload.get("agent").and_then(Value::as_str).unwrap_or_default();
            let path = payload.get("path").and_then(Value::as_str);
            crate::console_cli::resources_set_path_command_at(&root, agent, path)
        }

        "model_track" => {
            let root = crate::paths::aicross_root()?;
            let store = crate::resources::get_resources_at(&crate::console_cli::store_file(&root)).ok();
            crate::track::stats_value(&root, None, store.as_ref(), chrono::Local::now().timestamp_millis())
        }

        "model_note_set" => {
            let root = crate::paths::aicross_root()?;
            let model = payload.get("model").and_then(Value::as_str).unwrap_or_default();
            let tags: Vec<String> = payload
                .get("tags")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let note = payload.get("note").and_then(Value::as_str).unwrap_or_default();
            let key = crate::track::parse_model_key(model)?;
            let saved = crate::track::set_note(&root, &key, Some(&tags.join(",")), Some(note))?;
            Ok(crate::track::note_value(&key, &saved))
        }

        "config_set_default_mode" => {
            let root = crate::paths::aicross_root()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                true,
            )?;
            let mode = payload.get("mode").and_then(Value::as_str).unwrap_or_default();
            crate::console_cli::config_set_default_mode_at(&root, &host, mode).map_err(|(_, m)| m)
        }

        "config_set_seat" => {
            let root = crate::paths::aicross_root()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                true,
            )?;
            let mode = payload.get("mode").and_then(Value::as_str).unwrap_or_default();
            let stage = payload.get("stage").and_then(Value::as_u64).unwrap_or(1) as usize;
            let index = payload.get("index").and_then(Value::as_u64).unwrap_or(1) as usize;
            let seat_val = payload.get("seat").cloned().unwrap_or(Value::Null);
            let seat: crate::orchestration::SeatChoice =
                serde_json::from_value(seat_val).map_err(|e| format!("人选格式无效：{e}"))?;
            crate::console_cli::config_set_seat_at(&root, &host, mode, stage, index, seat)
                .map_err(|(_, m)| m)
        }

        "config_set_count" => {
            let root = crate::paths::aicross_root()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                true,
            )?;
            let mode = payload.get("mode").and_then(Value::as_str).unwrap_or_default();
            let count = payload.get("count").and_then(Value::as_u64).unwrap_or(1) as u32;
            crate::console_cli::config_set_count_at(&root, &host, mode, count).map_err(|(_, m)| m)
        }

        "config_set_focus" => {
            let root = crate::paths::aicross_root()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                true,
            )?;
            let mode = payload.get("mode").and_then(Value::as_str).unwrap_or_default();
            let index = payload.get("index").and_then(Value::as_u64).unwrap_or(1) as usize;
            let focus = payload.get("focus").and_then(Value::as_str).unwrap_or_default();
            crate::console_cli::config_set_focus_at(&root, &host, mode, index, focus)
                .map_err(|(_, m)| m)
        }

        "config_clear_seat" => {
            let root = crate::paths::aicross_root()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                true,
            )?;
            let mode = payload.get("mode").and_then(Value::as_str).unwrap_or_default();
            let stage = payload.get("stage").and_then(Value::as_u64).map(|v| v as usize);
            let index = payload.get("index").and_then(Value::as_u64).map(|v| v as usize);
            crate::console_cli::config_clear_seat_at(&root, &host, mode, stage, index)
                .map_err(|(_, m)| m)
        }

        "config_recommend" => {
            let root = crate::paths::aicross_root()?;
            let host = payload.get("host").and_then(Value::as_str);
            let mode = payload.get("mode").and_then(Value::as_str);
            let all = payload.get("all").and_then(Value::as_bool).unwrap_or(false);
            crate::recommend::recommend_at(&root, host, mode, all).map_err(|(_, m)| m)
        }

        "dispatch_settings" => {
            let root = crate::paths::aicross_root()?;
            let acp = crate::channels::acp_setup::adapter_status_cached();
            let dispatch = crate::engine::load_settings(&root);
            Ok(json!({
                "dispatch": dispatch,
                "path": root.join("dispatch-settings.json").to_string_lossy(),
                "acp": acp,
            }))
        }

        "dispatch_save" => {
            let root = crate::paths::aicross_root()?;
            let defer_acp = payload
                .get("deferAcpInstall")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let patch: Map<String, Value> = payload
                .get("patch")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let ops = if defer_acp {
                crate::console_cli::AcpOps::status_only()
            } else {
                crate::console_cli::AcpOps::production()
            };
            let outcome = crate::console_cli::config_set_at(&root, patch, &ops)
                .map_err(|(_, m)| m)?;
            let mut value = json!({
                "dispatch": &outcome.dispatch,
                "note": if defer_acp { None } else { outcome.note },
            });
            if let Some(reports) = &outcome.acp {
                value["acp"] = serde_json::to_value(reports)
                    .map_err(|e| format!("生成适配器状态失败：{e}"))?;
            }
            Ok(value)
        }

        "acp_install" => {
            let reports = crate::channels::acp_setup::install_missing_blocking();
            Ok(json!({
                "acp": reports,
            }))
        }

        "slash_list" => {
            let root = crate::paths::aicross_root()?;
            let home = crate::paths::home()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                true,
            )?;
            crate::slash::slash_list_at(&root, &home, &host).map_err(|(_, m)| m)
        }

        "slash_add" => {
            let root = crate::paths::aicross_root()?;
            let home = crate::paths::home()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                true,
            )?;
            let name = payload.get("name").and_then(Value::as_str).unwrap_or_default();
            let mode = payload.get("mode").and_then(Value::as_str).unwrap_or_default();
            let note = payload.get("note").and_then(Value::as_str).unwrap_or_default();
            crate::slash::slash_add_at(&root, &home, &host, name, mode, note).map_err(|(_, m)| m)
        }

        "slash_add_all" => {
            let root = crate::paths::aicross_root()?;
            let home = crate::paths::home()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                true,
            )?;
            let prefix = payload
                .get("prefix")
                .and_then(Value::as_str)
                .unwrap_or(crate::slash::DEFAULT_PREFIX);
            crate::slash::slash_add_all_at(&root, &home, &host, prefix).map_err(|(_, m)| m)
        }

        "slash_remove" => {
            let root = crate::paths::aicross_root()?;
            let home = crate::paths::home()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                true,
            )?;
            let name = payload.get("name").and_then(Value::as_str).unwrap_or_default();
            crate::slash::slash_remove_at(&root, &home, &host, name).map_err(|(_, m)| m)
        }

        "preset_save" => {
            let root = crate::paths::aicross_root()?;
            let host = resolve_window_host(
                &root,
                payload.get("host").and_then(Value::as_str).map(String::from),
                false,
            )?;
            let preset_val = payload.get("preset").cloned().unwrap_or(Value::Null);
            let input: crate::preset::PresetInput =
                serde_json::from_value(preset_val).map_err(|e| format!("预设对象无法解析：{e}"))?;
            crate::preset::preset_save_at(
                &root,
                if host.is_empty() { None } else { Some(host.as_str()) },
                &input,
            )
            .map_err(|(_, m)| m)
        }

        "preset_remove" => {
            let root = crate::paths::aicross_root()?;
            let home = crate::paths::home()?;
            let id = payload.get("id").and_then(Value::as_str).unwrap_or_default();
            crate::preset::preset_remove_at(&root, &home, id).map_err(|(_, m)| m)
        }

        "skill_status" => {
            let home = crate::paths::home()?;
            let project = payload.get("project").and_then(Value::as_str).map(std::path::PathBuf::from);
            crate::skill_install::status_at(&home, project.as_deref()).map_err(|(_, m)| m)
        }

        "skill_install" => {
            let home = crate::paths::home()?;
            let host = payload.get("host").and_then(Value::as_str).unwrap_or_default();
            let scope = payload.get("scope").and_then(Value::as_str).unwrap_or("global");
            let project = payload.get("project").and_then(Value::as_str).map(std::path::PathBuf::from);
            crate::skill_install::install_at(&home, host, scope, project.as_deref())
                .map_err(|(_, m)| m)
        }

        "skill_upgrade" => {
            let home = crate::paths::home()?;
            let host = payload.get("host").and_then(Value::as_str).unwrap_or_default();
            let path = payload.get("path").and_then(Value::as_str).unwrap_or_default();
            crate::skill_install::upgrade_at(&home, host, Path::new(path)).map_err(|(_, m)| m)
        }

        "skill_uninstall" => {
            let home = crate::paths::home()?;
            let host = payload.get("host").and_then(Value::as_str).unwrap_or_default();
            let scope = payload.get("scope").and_then(Value::as_str).unwrap_or("global");
            let project = payload.get("project").and_then(Value::as_str).map(std::path::PathBuf::from);
            crate::skill_install::uninstall_at(&home, host, scope, project.as_deref())
                .map_err(|(_, m)| m)
        }

        other => Err(format!("未知的界面桥接命令：{other}")),
    }
}

#[cfg(not(feature = "tauri-ui"))]
pub fn start_native_macos_ui(pending_open: Option<String>) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("/usr/bin/open");
    cmd.arg("-a").arg("/Applications/aicross.app");
    if let Some(run_id) = pending_open {
        cmd.arg("--args").arg("--open-run").arg(run_id);
    }
    let _ = cmd.status();
    Ok(())
}
