import { useEffect, useState } from "react";
import { api } from "../api";
import type { SessionEntry } from "../types";

export function Sessions() {
  const [sessions, setSessions] = useState<SessionEntry[]>([]);
  const [selected, setSelected] = useState<SessionEntry | null>(null);
  const [error, setError] = useState("");

  const load = () => api.sessions().then(setSessions).catch((value) => setError(String(value)));
  useEffect(() => {
    void load();
  }, []);

  return (
    <div className="pane-body sessions-pane">
      <div className="action-row pane-actions">
        <button className="button small" onClick={load}>刷新</button>
      </div>
      <div className="session-list">
        {sessions.length === 0 && <div className="empty-state">暂无记录</div>}
        {sessions.map((session) => (
          <button className="session-row" key={`${session.tool}-${session.id}`} onClick={() => setSelected(session)}>
            <span className="session-head">
              <span className="tool-tag">{session.tool}</span>
              <time>{formatTime(session.timestamp)}</time>
            </span>
            <span className="session-message">{session.firstMessage}</span>
          </button>
        ))}
      </div>
      {selected && (
        <dl className="metadata-card">
          <dt>时间</dt><dd>{formatTime(selected.timestamp)}</dd>
          <dt>工具</dt><dd>{selected.tool}</dd>
          <dt>模型</dt><dd>{selected.model}</dd>
          <dt>项目目录</dt><dd className="mono path-wrap">{selected.projectPath}</dd>
          <dt>首条消息</dt><dd>{selected.firstMessage}</dd>
        </dl>
      )}
      {error && <div className="error-line">{error}</div>}
    </div>
  );
}

function formatTime(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? value : date.toLocaleString();
}
