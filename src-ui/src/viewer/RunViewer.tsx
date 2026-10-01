// TASK-37 查看器第一版：一次 run 一页，只做「看 + 采纳」；重派与放弃仍在
// 宿主对话里由 skill 调命令行。数据经 load_run_view 读 run 文件与图文件
// （只读）；run 运行中每 2 秒重取一次，终态停止。修正轮 R1（fix-37-r1）：
// 轮询出错保留旧数据继续重试（R1-7）；轮询与采纳的响应按请求序号丢弃过期
// 结果（R1-8）；refreshSeq 变化时重取（R1-9）；挂载后焦点落在「关闭」、
// Tab 不出查看页、Esc 只关最上层（R1-6）。

import { useEffect, useRef, useState } from 'react';
import { commands, type RunView, type RunViewNode } from '../tauri';
import { ConsoleMarkdown } from '../components/center/ConsoleMarkdown';
import { vendorColor } from '../graph/vendor';
import {
  RUN_VIEW_POLL_INTERVAL_MS,
  acceptanceVerdict,
  acceptanceVerdictLabel,
  adoptAction,
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
  runStatusTone,
  stageColumnCount,
} from './run-view';
import './RunViewer.css';

interface Props {
  runId: string;
  /** 热启动事件的递增序号（R1-9）：同一个 run 再次 open 时靠它触发重取。 */
  refreshSeq: number;
  onClose: () => void;
}

/** Tab 焦点陷阱用：查看页内可聚焦的元素。 */
const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input',
  'select',
  'textarea',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

export function RunViewer({ runId, refreshSeq, onClose }: Props) {
  const [view, setView] = useState<RunView | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [adoptBusyNodeId, setAdoptBusyNodeId] = useState('');
  const [adoptError, setAdoptError] = useState('');
  // 总耗时随秒走：运行中每秒重画一次，终态钉住。
  const [now, setNow] = useState(() => Date.now());
  // R1-8：请求序号。轮询与采纳发起时递增，响应落地前核对，不是最新请求的
  // 结果一律丢弃。
  const requestSeq = useRef(0);
  // R1-7：是否已拿到过数据。拿到过后再出错保留旧数据继续重试，只有首次
  // 读取失败才显示「找不到这次派发」。
  const hasData = useRef(false);
  // 最近一次已知状态：出错重试只对运行中的 run 有意义，终态 run 不另起
  // 轮询循环。
  const lastStatus = useRef<string | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const closeRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    const tick = async () => {
      const seq = ++requestSeq.current;
      try {
        const next = await commands.loadRunView(runId);
        // 过期响应（采纳刚接过序号）不落地，但链不停：运行中的 run 照常排
        // 下一次重取，采纳后的下一次重取会带回最新状态。
        const stale = cancelled || seq !== requestSeq.current;
        if (!stale) {
          hasData.current = true;
          lastStatus.current = next.status;
          setView(next);
          setNotFound(false);
        }
        const delay = pollIntervalMs(next.status);
        if (delay !== null && !cancelled) timer = window.setTimeout(tick, delay);
      } catch {
        if (cancelled || seq !== requestSeq.current) return;
        if (!hasData.current) {
          setNotFound(true);
          return;
        }
        // 已有数据：保留旧数据，运行中的 run 按间隔重试，终态不再起循环。
        if (lastStatus.current === 'running') {
          timer = window.setTimeout(tick, RUN_VIEW_POLL_INTERVAL_MS);
        }
      }
    };
    void tick();
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [runId, refreshSeq]);

  useEffect(() => {
    if (view?.status !== 'running') return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [view?.status, view?.runId]);

  // R1-6：挂载后焦点落在「关闭」按钮；查看页打开期间缩放边条抬到查看页
  // 之上（样式见 RunViewer.css），标题栏不在查看页覆盖范围内。读取态与
  // 正文态切换时「关闭」换成了另一个元素，重取焦点。
  const showMissing = notFound || view === null;
  useEffect(() => {
    closeRef.current?.focus();
    document.documentElement.classList.add('run-viewer-open');
    return () => document.documentElement.classList.remove('run-viewer-open');
  }, [showMissing]);

  useEffect(() => {
    const trapTab = (event: KeyboardEvent) => {
      const root = rootRef.current;
      if (!root) return;
      const focusable = Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
        .filter((element) => element.offsetParent !== null);
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      if (!root.contains(active)) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
        return;
      }
      if (event.shiftKey && active === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Tab') {
        trapTab(event);
        return;
      }
      if (event.key === 'Escape' && !event.isComposing) {
        // Esc 只关最上层：视口中心命中的不是查看页，说明上面还有层，不抢。
        const hit = document.elementFromPoint(
          window.innerWidth / 2,
          window.innerHeight / 2,
        );
        if (!hit || !rootRef.current?.contains(hit)) return;
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
    const seq = ++requestSeq.current;
    setAdoptBusyNodeId(nodeId);
    setAdoptError('');
    try {
      const next = await commands.adoptRunNode(runId, nodeId);
      if (seq !== requestSeq.current) return;
      setView(next);
    } catch (error) {
      if (seq !== requestSeq.current) return;
      setAdoptError(String(error));
    } finally {
      setAdoptBusyNodeId('');
    }
  };

  if (showMissing) {
    return (
      <div className="run-viewer" role="dialog" aria-label="查看派发" ref={rootRef}>
        <div className="run-viewer-missing">
          {notFound
            ? <><div className="run-viewer-missing-text">找不到这次派发</div><div className="run-viewer-missing-id">{runId}</div></>
            : <div className="run-viewer-missing-text">正在读取这次派发…</div>}
          <button type="button" ref={closeRef} onClick={onClose}>关闭</button>
        </div>
      </div>
    );
  }

  const groups = groupNodesByStage(view);
  const tone = runStatusTone(view.status);
  return (
    <div className="run-viewer" role="dialog" aria-label="查看派发" ref={rootRef}>
      <header className="run-viewer-head">
        <div className="run-viewer-head-main">
          <div className="run-viewer-title">
            {view.presetName !== '' && <span className="run-viewer-preset">{view.presetName}</span>}
            <span className={`run-viewer-status${tone === 'plain' ? '' : tone === 'pill' ? ' run-viewer-pill run-viewer-status-running' : ' run-viewer-pill alert'}`}>
              {runStatusLabel(view.status)}
            </span>
          </div>
          <div className="run-viewer-meta">
            <span>开始时间 <span className="run-viewer-mono">{formatStartedAt(view.startedAt)}</span></span>
            <span>总耗时 <span className="run-viewer-mono">{formatDuration(runElapsedMs(view, now))}</span></span>
            <span className="run-viewer-mono">run {view.runId}</span>
          </div>
        </div>
        <button type="button" ref={closeRef} onClick={onClose}>关闭</button>
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
  const action = adoptAction(node);
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
            {identityMismatch(node) && <span className="run-viewer-mismatch">真身不符</span>}
          </dd>
        </div>
        <div>
          <dt>token</dt>
          <dd>
            {'输入 '}<span className="run-viewer-mono">{formatTokens(node.tokensIn)}</span>
            {' / 输出 '}<span className="run-viewer-mono">{formatTokens(node.tokensOut)}</span>
            {' / 缓存 '}<span className="run-viewer-mono">{formatTokens(node.tokensCache)}</span>
          </dd>
        </div>
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
      {action !== null && (
        <div className="run-viewer-card-actions">
          {action.clickable
            ? <button type="button" disabled={busy} onClick={onAdopt}>{action.label}</button>
            : <span className="run-viewer-adopted">{action.label}</span>}
        </div>
      )}
    </section>
  );
}
