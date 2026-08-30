import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { ChangeEntry, DiffResult } from "../types";

const labels = { added: "新增", modified: "修改", deleted: "删除" } as const;

export function Changes({ projectPath }: { projectPath: string }) {
  const [changes, setChanges] = useState<ChangeEntry[]>([]);
  const [diff, setDiff] = useState<DiffResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const projectVersion = useRef(0);

  useEffect(() => {
    projectVersion.current += 1;
    setChanges([]);
    setDiff(null);
    setBusy(false);
    setError("");
  }, [projectPath]);

  const refresh = async () => {
    const version = projectVersion.current;
    setBusy(true);
    setError("");
    try {
      const value = await api.refreshChanges();
      if (version !== projectVersion.current) return;
      setChanges(value);
      setDiff(null);
    } catch (value) {
      if (version !== projectVersion.current) return;
      setError(String(value));
    } finally {
      if (version === projectVersion.current) setBusy(false);
    }
  };

  const reset = async () => {
    const version = projectVersion.current;
    setBusy(true);
    setError("");
    try {
      await api.resetBaseline();
      if (version !== projectVersion.current) return;
      setChanges([]);
      setDiff(null);
    } catch (value) {
      if (version !== projectVersion.current) return;
      setError(String(value));
    } finally {
      if (version === projectVersion.current) setBusy(false);
    }
  };

  const showDiff = async (path: string) => {
    const version = projectVersion.current;
    try {
      const value = await api.diff(path);
      if (version !== projectVersion.current) return;
      setDiff(value);
      setError("");
    } catch (value) {
      if (version !== projectVersion.current) return;
      setError(String(value));
    }
  };

  return (
    <div className="pane-body change-pane">
      <div className="action-row pane-actions">
        <button className="button small primary" disabled={busy} onClick={refresh}>
          {busy ? "刷新中" : "刷新"}
        </button>
        <button className="button small" disabled={busy} onClick={() => void reset()}>
          重置基线
        </button>
      </div>
      <div className="hint">基线为打开项目时的状态。已忽略 .git / node_modules / target / dist</div>
      <div className="change-list">
        {changes.length === 0 && <div className="empty-state">暂无修改</div>}
        {changes.map((change) => (
          <button
            key={change.path}
            className="change-row"
            onClick={() => void showDiff(change.path)}
          >
            <span className={`change-badge ${change.kind}`}>{labels[change.kind]}</span>
            <span className="change-path mono">{change.path}</span>
          </button>
        ))}
      </div>
      {diff && (
        <div className="diff-panel">
          <div className="diff-title mono">{diff.path}</div>
          <pre>{diff.text || "文件内容无行级变化"}</pre>
        </div>
      )}
      {error && <div className="error-line">{error}</div>}
    </div>
  );
}
