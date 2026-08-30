import { useState } from "react";
import { api } from "../api";
import type { ChangeEntry, DiffResult } from "../types";

const labels = { added: "新增", modified: "修改", deleted: "删除" } as const;

export function Changes({ projectPath }: { projectPath: string }) {
  const [changes, setChanges] = useState<ChangeEntry[]>([]);
  const [diff, setDiff] = useState<DiffResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const refresh = async () => {
    setBusy(true);
    setError("");
    try {
      setChanges(await api.refreshChanges());
      setDiff(null);
    } catch (value) {
      setError(String(value));
    } finally {
      setBusy(false);
    }
  };

  const reset = async () => {
    await api.resetBaseline();
    setChanges([]);
    setDiff(null);
  };

  return (
    <div className="pane-body change-pane" key={projectPath}>
      <div className="action-row pane-actions">
        <button className="button small primary" disabled={busy} onClick={refresh}>
          {busy ? "刷新中" : "刷新"}
        </button>
        <button className="button small" onClick={() => void reset()}>
          重置基线
        </button>
      </div>
      <div className="hint">基线为打开项目时的状态；忽略依赖与构建目录。</div>
      <div className="change-list">
        {changes.length === 0 && <div className="empty-state">暂无修改</div>}
        {changes.map((change) => (
          <button
            key={change.path}
            className="change-row"
            onClick={() => api.diff(change.path).then(setDiff).catch((value) => setError(String(value)))}
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

