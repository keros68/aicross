// TASK-37 查看器第一版：一次 run 一页，只做「看 + 采纳」；重派与放弃仍在
// 宿主对话里由 skill 调命令行。数据经 load_run_view 读 run 文件与图文件
// （只读）；run 运行中每 2 秒重取一次，终态停止。

import { useEffect, useState } from 'react';
import { commands, type RunView, type RunViewNode } from '../tauri';
import { ConsoleMarkdown } from '../components/center/ConsoleMarkdown';
import { vendorColor } from '../graph/vendor';
import {
  acceptanceVerdict,
  acceptanceVerdictLabel,
  canAdopt,
  cardSubtitle,
  cardTitle,
  fallbackText,
  formatDuration,
  formatStartedAt,
  formatTokens,
  groupNodesByStage,
  identityMismatch,
  nodeStatusLabel,
  nodeStatusTone,
  pollIntervalMs,
  quotaResetText,
  reportedModelLabel,
  runElapsedMs,
  runStatusLabel,
  stageColumnCount,
} from './run-view';
import './RunViewer.css';

interface Props {
  runId: string;
  onClose: () => void;
}

export function RunViewer({ runId, onClose }: Props) {
  const [view, setView] = useState<RunView | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [adoptBusyNodeId, setAdoptBusyNodeId] = useState('');
  const [adoptError, setAdoptError] = useState('');
  // 总耗时随秒走：运行中每秒重画一次，终态钉住。
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    const tick = async () => {
      try {
        const next = await commands.loadRunView(runId);
        if (cancelled) return;
        setView(next);
        setNotFound(false);
        const delay = pollIntervalMs(next.status);
        if (delay !== null) timer = window.setTimeout(tick, delay);
      } catch {
        if (cancelled) return;
        setView(null);
        setNotFound(true);
      }
    };
    setView(null);
    setNotFound(false);
    void tick();
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [runId]);

  useEffect(() => {
    if (view?.status !== 'running') return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [view?.status, view?.runId]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.isComposing) {
        event.preventDefault();
        event.stopPropagation();
        onClose();
      }
    };
    document.addEventListener('keydown', onKey, true);
    return () => document.removeEventListener('keydown', onKey, true);
  }, [onClose]);

  const adopt = async (nodeId: string) => {
    if (adoptBusyNodeId !== '') return;
    setAdoptBusyNodeId(nodeId);
    setAdoptError('');
    try {
      setView(await commands.adoptRunNode(runId, nodeId));
    } catch (error) {
      setAdoptError(String(error));
    } finally {
      setAdoptBusyNodeId('');
    }
  };

  if (notFound || view === null) {
    return (
      <div className="run-viewer" role="dialog" aria-label="查看派发">
        <div className="run-viewer-missing">
          {notFound
            ? <><div className="run-viewer-missing-text">找不到这次派发</div><div className="run-viewer-missing-id">{runId}</div></>
            : <div className="run-viewer-missing-text">正在读取这次派发…</div>}
          <button type="button" onClick={onClose}>关闭</button>
        </div>
      </div>
    );
  }

  const groups = groupNodesByStage(view);
  return (
    <div className="run-viewer" role="dialog" aria-label="查看派发">
      <header className="run-viewer-head">
        <div className="run-viewer-head-main">
          <div className="run-viewer-title">
            {view.presetName !== '' && <span className="run-viewer-preset">{view.presetName}</span>}
            <span className={`run-viewer-status run-viewer-status-${view.status}`}>{runStatusLabel(view.status)}</span>
          </div>
          <div className="run-viewer-meta">
            <span>开始时间 <span className="run-viewer-mono">{formatStartedAt(view.startedAt)}</span></span>
            <span>总耗时 <span className="run-viewer-mono">{formatDuration(runElapsedMs(view, now))}</span></span>
            <span className="run-viewer-mono">run {view.runId}</span>
          </div>
        </div>
        <button type="button" onClick={onClose}>关闭</button>
      </header>
      <div className="run-viewer-body">
        {view.reason !== '' && <div className="run-viewer-reason">{runStatusLabel(view.status)}原因：{view.reason}</div>}
        {view.leftover.length > 0 && (
          <section className="run-viewer-leftover">
            <div className="run-viewer-section-title">遗留</div>
            <ol>{view.leftover.map((item, index) => <li key={index}>{item}</li>)}</ol>
          </section>
        )}
        {groups.map((group) => (
          <section className="run-viewer-stage" key={group.stage}>
            <div className="run-viewer-section-title">{group.title}</div>
            <div
              className="run-viewer-cards"
              style={{ gridTemplateColumns: `repeat(${stageColumnCount(group.nodes.length)}, minmax(280px, 1fr))` }}
            >
              {group.nodes.map((node) => (
                <RunCard
                  key={node.nodeId}
                  node={node}
                  view={view}
                  busy={adoptBusyNodeId === node.nodeId}
                  onAdopt={() => void adopt(node.nodeId)}
                />
              ))}
            </div>
          </section>
        ))}
        {adoptError !== '' && <div className="run-viewer-error">{adoptError}</div>}
      </div>
    </div>
  );
}

function RunCard({ node, view, busy, onAdopt }: {
  node: RunViewNode;
  view: RunView;
  busy: boolean;
  onAdopt: () => void;
}) {
  const verdict = acceptanceVerdict(node);
  const worktree = node.artifacts?.worktree ?? '';
  return (
    <section className={`run-viewer-card${node.adopted ? ' adopted' : ''}`}>
      <div className="run-viewer-card-head">
        <span className="run-viewer-vendor-dot" style={{ backgroundColor: vendorColor(node.agent) }} />
        <div className="run-viewer-card-titles">
          <div className="run-viewer-card-title">{cardTitle(node, view)}</div>
          <div className="run-viewer-card-subtitle">{cardSubtitle(node)}</div>
        </div>
        <span className={`run-viewer-pill ${nodeStatusTone(node) === 'alert' ? 'alert' : ''}`}>{nodeStatusLabel(node)}</span>
      </div>
      <dl className="run-viewer-card-facts">
        <div><dt>请求模型</dt><dd className="run-viewer-mono">{node.model || '—'}</dd></div>
        <div>
          <dt>实际模型</dt>
          <dd className="run-viewer-mono">
            {reportedModelLabel(node)}
            {identityMismatch(node) && <span className="run-viewer-mismatch">与请求不符</span>}
          </dd>
        </div>
        <div><dt>token</dt><dd className="run-viewer-mono">
          输入 {formatTokens(node.tokensIn)} / 输出 {formatTokens(node.tokensOut)} / 缓存 {formatTokens(node.tokensCache)}
        </dd></div>
        <div><dt>耗时</dt><dd className="run-viewer-mono">{formatDuration(node.ms)}</dd></div>
        {verdict !== null && <div><dt>结论</dt><dd>{acceptanceVerdictLabel(verdict)}</dd></div>}
      </dl>
      {fallbackText(node) !== '' && <div className="run-viewer-card-note">{fallbackText(node)}</div>}
      {quotaResetText(node) !== '' && <div className="run-viewer-card-note">{quotaResetText(node)}</div>}
      {node.skippedReason !== '' && <div className="run-viewer-card-note">已跳过：{node.skippedReason}</div>}
      {node.text !== '' && <div className="run-viewer-card-text"><ConsoleMarkdown text={node.text} /></div>}
      {node.artifacts && (
        <details className="run-viewer-artifacts">
          <summary>产物 · {node.artifacts.changedFileCount} 个文件</summary>
          {worktree !== '' && <div className="run-viewer-worktree">采纳前请手动合并：{worktree}</div>}
          <pre>{node.artifacts.diff || '无文件改动'}</pre>
        </details>
      )}
      {canAdopt(node) && (
        <div className="run-viewer-card-actions">
          <button type="button" disabled={busy} onClick={onAdopt}>
            {node.adopted ? '取消采纳' : '采纳'}
          </button>
          {node.adopted && <span className="run-viewer-card-note">已采纳</span>}
        </div>
      )}
    </section>
  );
}
