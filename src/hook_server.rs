// Coffee CLI Hook Server
//
// Loopback TCP listener that accepts one JSON line per connection
// from each tool's forwarder script:
//
//   - scripts/aicross-hook.py            — Claude Code stdin hooks
//   - scripts/aicross-codex-notify.py    — Codex `notify` argv-tail
//   - scripts/aicross-opencode-plugin.js — OpenCode plugin events
//   - `<exe> __kimi-hook` (hook_forwarder.rs) — Kimi Code [[hooks]] events
//
// Single payload kind:
//
//   - **Status** payload (status field present):
//       `{tab_id, tool, status, event}` → emit `agent-status` event
//       to the frontend's tab indicators.
//
// File-edit attribution per AI tool was removed in v2.7.x —
// ChangesBoard is now sourced from a folder snapshot diff
// (`compute_folder_stats` Tauri command, tool-agnostic by design).
// `path` / `action` / `cwd` fields are kept in the wire payload for
// backward compat with installed hook scripts; they are ignored.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Wire payload received from a forwarder script. Serde ignores
/// unknown fields by default, so stale forwarder scripts left over
/// from v2.6.x that still include `path` / `action` / `cwd` are
/// accepted gracefully — those fields are simply discarded.
#[derive(Debug, Clone, Deserialize)]
pub struct HookPayload {
    pub tab_id: String,
    pub tool: String,
    /// "idle" | "working" | "wait_input" — drives the tab dot.
    pub status: Option<String>,
    /// Hook event name (Claude: PostToolUse / Notification / Stop;
    /// Codex: agent-turn-complete; OpenCode: session.status / etc.).
    pub event: Option<String>,
}

/// Frontend payload for the `agent-status` Tauri event — unchanged
/// shape from v2.6.x so existing TS subscribers keep working.
#[derive(Debug, Clone, Serialize)]
pub struct AgentStatusEvent {
    pub tab_id: String,
    pub tool: String,
    pub status: String,
    pub event: String,
}

/// Bind a loopback TCP listener on an OS-assigned port, return the port, and
/// hand the listener off to an async accept loop.
pub fn start(app: AppHandle) -> anyhow::Result<u16> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    std_listener.set_nonblocking(true)?;
    let port = std_listener.local_addr()?.port();
    let token = crate::console_api::token_hex();
    if let Err(error) = crate::console_api::write_manifest(port, &token) {
        eprintln!("[hook-server] 控制台通道不可用：{}", error);
    }
    eprintln!("[hook-server] listening on 127.0.0.1:{}", port);

    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::from_std(std_listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[hook-server] from_std failed: {}", e);
                return;
            }
        };
        loop {
            match listener.accept().await {
                Ok((socket, _)) => {
                    let app = app.clone();
                    let token = token.clone();
                    tauri::async_runtime::spawn(async move {
                        handle_conn(app, token, socket).await;
                    });
                }
                Err(e) => {
                    eprintln!("[hook-server] accept error: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    });

    Ok(port)
}

async fn handle_conn(app: AppHandle, token: String, socket: tokio::net::TcpStream) {
    let mut reader = BufReader::new(socket);
    let mut line = String::new();
    if let Err(e) = reader.read_line(&mut line).await {
        eprintln!("[hook-server] read error: {}", e);
        return;
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    if ["GET ", "POST ", "HEAD ", "OPTIONS "].iter().any(|prefix| trimmed.starts_with(prefix)) {
        handle_http(app, &token, trimmed, reader).await;
        return;
    }
    match serde_json::from_str::<HookPayload>(trimmed) {
        Ok(payload) => dispatch(&app, payload),
        Err(e) => {
            eprintln!("[hook-server] bad JSON ({}): {}", e, trimmed);
        }
    }
    let _ = reader.into_inner().write_all(b"{}\n").await;
}

async fn handle_http(
    app: AppHandle,
    token: &str,
    request_line: &str,
    mut reader: BufReader<tokio::net::TcpStream>,
) {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    let mut authorization = None;
    let mut origin = None;
    let mut fetch_site = None;
    let mut content_length = 0_usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await.is_err() || header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("authorization") {
                authorization = Some(value.trim().to_string());
            } else if name.eq_ignore_ascii_case("origin") {
                origin = Some(value.trim().to_string());
            } else if name.eq_ignore_ascii_case("sec-fetch-site") {
                fetch_site = Some(value.trim().to_string());
            } else if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(usize::MAX);
            }
        }
    }
    let early = early_http_response(token, authorization.as_deref(), origin.as_deref(), fetch_site.as_deref());
    let response = if let Some(response) = early {
        response
    } else if content_length > 4 * 1024 * 1024 {
        crate::console_api::ApiResponse {
            status: 413,
            body: serde_json::json!({ "error": "请求正文过大" }).to_string(),
        }
    } else {
        let mut body = vec![0_u8; content_length];
        let read = tokio::io::AsyncReadExt::read_exact(&mut reader, &mut body).await;
        if read.is_err() {
            crate::console_api::ApiResponse {
                status: 400,
                body: serde_json::json!({ "error": "请求正文不完整" }).to_string(),
            }
        } else {
            crate::console_api::route(
                &crate::console_api::LiveBackend::new(app),
                token,
                authorization.as_deref(),
                method,
                path,
                &body,
            )
        }
    };
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    let wire = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.status,
        reason,
        response.body.as_bytes().len(),
        response.body,
    );
    let _ = reader.into_inner().write_all(wire.as_bytes()).await;
}

fn early_http_response(token: &str, authorization: Option<&str>, origin: Option<&str>, fetch_site: Option<&str>) -> Option<crate::console_api::ApiResponse> {
    if origin.is_some() || fetch_site.is_some_and(|value| value.eq_ignore_ascii_case("cross-site")) {
        return Some(crate::console_api::ApiResponse { status: 403, body: serde_json::json!({ "error": "不接受跨站请求" }).to_string() });
    }
    if !crate::console_api::authorized(token, authorization) {
        return Some(crate::console_api::ApiResponse { status: 401, body: serde_json::json!({ "error": "令牌无效" }).to_string() });
    }
    None
}

/// Translate a hook payload into a Tauri event. Only the `status`
/// path remains — see file-level doc for why per-tool file-edit
/// attribution was removed.
fn dispatch(app: &AppHandle, payload: HookPayload) {
    if let Some(status) = payload.status.as_deref() {
        let evt = AgentStatusEvent {
            tab_id: payload.tab_id.clone(),
            tool: payload.tool.clone(),
            status: status.to_string(),
            event: payload.event.clone().unwrap_or_default(),
        };
        eprintln!(
            "[hook-server] {} {} → {}",
            evt.tool, evt.event, evt.status
        );
        let _ = app.emit("agent-status", &evt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_and_cross_site_are_403() {
        assert_eq!(early_http_response("x", Some("Bearer x"), Some("https://example.test"), None).unwrap().status, 403);
        assert_eq!(early_http_response("x", Some("Bearer x"), None, Some("cross-site")).unwrap().status, 403);
    }

    #[test]
    fn unauthorized_is_decided_before_body_read() {
        let response = early_http_response("secret", Some("Bearer wrong"), None, None).unwrap();
        assert_eq!(response.status, 401);
    }

    #[test]
    fn head_and_options_are_http_methods() {
        for line in ["HEAD /v1/runs/x HTTP/1.1", "OPTIONS /v1/dispatch HTTP/1.1"] {
            assert!(["GET ", "POST ", "HEAD ", "OPTIONS "].iter().any(|prefix| line.starts_with(prefix)));
        }
    }
}
