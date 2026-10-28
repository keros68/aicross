// ResourcesPage.tsx — 「资源」页（TASK-47；TASK-49 起多轮改版，fix-51-r1 改为 magpie
// 式卡片）：工具行左边上次探测时间、右边重新探测；页顶数字条；每个 agent 一张卡片，
// 卡片头是 agent 图标、名称、版本与小统计，点卡片头展开 / 收起（fix-51-r2，记在
// localStorage），展开后一行一个模型（按 provider 分组，超过 8 条时收起其余），最近
// 一次探测没发现的模型收起在卡片底部，可清理（fix-51-r3，resources_prune，先确认）。
// 数字条「不可用」可点，筛出不可用的模型。单个第三方模型可测试连接。数据取
// config_snapshot 的 resources，操作结束后重新取一次。探测、测试连接、清理同一时间
// 只跑一个。TASK-58：从未探测时顶部空态条与「探测」；探测中按进度事件显示「探测中 2/4」与
// 各卡片转圈；有第三方模型的卡片给「全部测试」（并发 3，可取消），测未验证、验证失败、连接
// 超时的，没有可测的置灰；模式页「去测试」跳来时展开对应卡片。TASK-62：没装的 agent 折叠成
// 末尾一行「未安装 N 个」，展开后每行图标、名称与官网地址（可复制）；未实测的 agent 卡片标题
// 旁标「未实测」。TASK-66：这次没找到、但以前探测到过的 agent 留在主列表并写明原因；未安装
// 的 agent 可「指定路径」，保存后重新探测。

import { useCallback, useEffect, useRef, useState } from 'react';
import { commands, isTauri, type DetectProgress, type ResourceAgentEntry, type ResourceModelEntry, type ResourcesOutput } from '../tauri';
import {
  DETECT_STALE_HINT,
  MODEL_PREVIEW_LIMIT,
  agentExpandable,
  agentHomepage,
  agentStats,
  canSmoke,
  isAgentOpen,
  isListed,
  isUnavailable,
  isUntested,
  notInstalledText,
  readAgentOpen,
  writeAgentOpen,
  detectStatus,
  groupByProvider,
  missingCount,
  missingModelsText,
  modelNote,
  moreModelsText,
  previewModels,
  pruneTitle,
  referencedText,
  resourcesOverview,
  resolvedModelText,
  seatKey,
  splitInstalled,
  splitMissingModels,
  stateLabel,
  stateTone,
} from './config-view';
import { ConfirmDialog } from '../components/common/ConfirmDialog';
import { CopyButton } from './SlashCard';
import { AgentIcon, ProviderIcon } from '../components/common/AgentIcon';
import { Icon } from '../components/common/Icon';
import { StatCards } from '../components/common/StatCards';
import { PageHeader } from '../components/common/PageHeader';
import { pageTabStorage } from './page-tabs';
import { batchSmokeButton, batchSmokeLabel, batchSmokeTargets, detectButtonLabel, versionText } from './newuser-view';
import { runPool } from '../lib/pool';
import './ConfigPages.css';

type Busy =
  | { kind: 'detect' }
  | { kind: 'smoke'; key: string }
  | { kind: 'prune'; agent: string | null }
  | { kind: 'batch'; agent: string }
  | { kind: 'path'; agent: string }
  | null;

/** 「全部测试」的并发数。 */
const BATCH_LIMIT = 3;

function StateText({ state }: { state: string }) {
  const tone = stateTone(state);
  const className = tone === 'alert' ? 'pill alert' : tone === 'muted' ? 'pill muted' : 'cfg-state';
  return <span className={className}>{stateLabel(state)}</span>;
}

/** 一行说明：超出省略，悬停看全文。 */
function NoteLine({ text }: { text: string }) {
  return text === '' ? null : <p className="cfg-note note-line" title={text}>{text}</p>;
}

/** active：导航切到「资源」时为 true。页面常驻不卸载，每次切回重新取快照；
 *  探测或冒烟进行中时不取，由它们结束时取。 */
export function ResourcesPage({ active, host, refreshKey, onRefreshDone, onDetected, onGoModes, focusAgent }: {
  active: boolean;
  /** 模式页「去测试」跳来时要展开并滚到的 agent；seq 每次跳转递增。 */
  focusAgent: { agent: string; seq: number } | null;
  /** 清理后有人选引用了被删的模型时，提示条里「去模式页」。 */
  onGoModes: () => void;
  /** 顶栏「刷新」的计数，变化时重新读取。 */
  refreshKey: number;
  /** 刷新触发的读取结束后回报（key 为本次 refreshKey），顶栏据此停止转圈。 */
  onRefreshDone: (key: number, ok: boolean) => void;
  /** 当前主 agent；资源不分主 agent，只用来满足 config_snapshot 的参数，空串时不带。 */
  host: string;
  /** 探测结束后通知外壳重读主 agent 列表。 */
  onDetected: () => void;
}) {
  const [resources, setResources] = useState<ResourcesOutput | null>(null);
  // 取到资源的时刻，用来判断上次探测是否超过 7 天。
  const [loadedAt, setLoadedAt] = useState(0);
  // 卡片折叠记忆（agent id → 是否展开），记在 localStorage；没记过的按默认（有不可用模型的展开）。
  const [savedOpen, setSavedOpen] = useState<Record<string, boolean>>(() => readAgentOpen(pageTabStorage()));
  // 「不可用」筛选：点数字条那一格或工具行的分段切换。
  const [filter, setFilter] = useState<'unavailable' | null>(null);
  // 模型超过 8 条时点了「展开其余」的 agent。
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  // 展开了「未发现」模型的 agent。
  const [showMissing, setShowMissing] = useState<Record<string, boolean>>({});
  const [loadError, setLoadError] = useState('');
  const [busy, setBusyState] = useState<Busy>(null);
  const busyRef = useRef<Busy>(null);
  const setBusy = (next: Busy) => {
    busyRef.current = next;
    setBusyState(next);
  };
  const [headError, setHeadError] = useState('');
  const [rowErrors, setRowErrors] = useState<Record<string, string>>({});
  // 清理确认框：要清的 agent（null 为全部）与条数。
  const [pruneAsk, setPruneAsk] = useState<{ agent: string | null; count: number } | null>(null);
  const closePrune = useCallback(() => setPruneAsk(null), []);
  // 清理后仍被人选引用的已删模型条数；0 不提示。
  const [referenced, setReferenced] = useState(0);
  // 探测进度（拿不到进度事件时为 null）与已探测完的 agent。
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [detectedAgents, setDetectedAgents] = useState<Set<string>>(() => new Set());
  // 全部测试：进度与取消标记。
  const [batch, setBatch] = useState<{ agent: string; done: number; total: number } | null>(null);
  const batchStop = useRef(false);
  // 「未安装 N 个」是否展开。
  const [showNotInstalled, setShowNotInstalled] = useState(false);
  // 正在编辑指定路径的 agent 与输入内容（TASK-66）。
  const [pathDraft, setPathDraft] = useState<Record<string, string>>({});

  // 快照请求序号：首次读取、刷新、探测 / 测试连接 / 清理后的重读各自取号，落地前核对
  // 是最新一次，过期的结果丢弃，避免旧结果盖掉新结果（同模式页）。
  const snapshotSeq = useRef(0);

  const reload = useCallback(async () => {
    const seq = ++snapshotSeq.current;
    try {
      const snapshot = await commands.configSnapshot(host === '' ? null : host);
      if (seq !== snapshotSeq.current) return;
      setResources(snapshot.resources);
      setLoadedAt(Date.now());
      setLoadError('');
    } catch (err) {
      if (seq === snapshotSeq.current) setLoadError(String(err));
    }
  }, [host]);

  useEffect(() => {
    if (!active) return;
    // 不在窗口里、或探测 / 测试连接 / 清理进行中（它们结束时自己会重读）：刷新直接算完成。
    if (!isTauri || busyRef.current !== null) {
      if (refreshKey > 0) onRefreshDone(refreshKey, true);
      return;
    }
    let cancelled = false;
    const seq = ++snapshotSeq.current;
    commands.configSnapshot(host === '' ? null : host)
      .then((snapshot) => {
        if (refreshKey > 0) onRefreshDone(refreshKey, true);
        if (cancelled || seq !== snapshotSeq.current) return;
        setResources(snapshot.resources);
        setLoadedAt(Date.now());
        setLoadError('');
      })
      .catch((err) => {
        if (refreshKey > 0) onRefreshDone(refreshKey, false);
        if (!cancelled && seq === snapshotSeq.current) setLoadError(String(err));
      });
    return () => { cancelled = true; };
  }, [active, host, refreshKey, onRefreshDone]);

  // 模式页「去测试」跳来：展开该 agent 卡片、退出筛选，并滚到它。第一次跳来时资源可能
  // 还没读到、卡片还不存在：记下待滚动的 agent，数据到了再滚一次，滚到后清掉。
  const pendingScroll = useRef<string | null>(null);
  const scrollToPending = useCallback(() => {
    const agent = pendingScroll.current;
    if (agent === null) return;
    const card = document.getElementById(`agent-card-${agent}`);
    if (!card) return;
    card.scrollIntoView({ block: 'start', behavior: 'smooth' });
    pendingScroll.current = null;
  }, []);

  useEffect(() => {
    if (!focusAgent) return;
    setFilter(null);
    setSavedOpen((current) => {
      const next = { ...current, [focusAgent.agent]: true };
      writeAgentOpen(pageTabStorage(), next);
      return next;
    });
    pendingScroll.current = focusAgent.agent;
    const frame = window.requestAnimationFrame(scrollToPending);
    return () => window.cancelAnimationFrame(frame);
  }, [focusAgent, scrollToPending]);

  // 「去测试」的目标已不在主列表（未安装）时，每次跳转展开一次折叠行，再滚到对应行。
  const handledFocusSeq = useRef(-1);
  useEffect(() => {
    if (!focusAgent || resources === null || handledFocusSeq.current === focusAgent.seq) return;
    if (!resources.agents.some((agent) => agent.id === focusAgent.agent && !isListed(agent))) return;
    handledFocusSeq.current = focusAgent.seq;
    setShowNotInstalled(true);
    const frame = window.requestAnimationFrame(scrollToPending);
    return () => window.cancelAnimationFrame(frame);
  }, [focusAgent, resources, scrollToPending]);

  useEffect(() => {
    if (resources === null || pendingScroll.current === null) return;
    const frame = window.requestAnimationFrame(scrollToPending);
    return () => window.cancelAnimationFrame(frame);
  }, [resources, scrollToPending]);

  if (!isTauri) {
    return (
      <div className="page">
        <div className="page-body"><p className="cfg-empty">仅在 aicross 窗口中可用</p></div>
      </div>
    );
  }

  const detect = async () => {
    if (busyRef.current !== null) return;
    setBusy({ kind: 'detect' });
    setHeadError('');
    setProgress(null);
    setDetectedAgents(new Set());
    // 进度事件：每个 agent 探测完发一次，收到就重读一次快照让该卡片更新。浏览器预览里
    // 拿不到事件，按钮退回「探测中…」。
    let unlisten: (() => void) | undefined;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten = await listen<DetectProgress>('resources-detect-progress', (event) => {
        setProgress({ done: event.payload.done, total: event.payload.total });
        setDetectedAgents((current) => new Set(current).add(event.payload.agent));
        void reload();
      });
    } catch { /* 事件不可用 */ }
    try {
      await commands.resourcesDetect();
      setRowErrors({});
    } catch (err) {
      setHeadError(String(err));
    }
    unlisten?.();
    await reload();
    onDetected();
    setProgress(null);
    setBusy(null);
  };

  /** 逐条测试该 agent 下待测的第三方模型，并发 3；取消后不再发新请求。失败的条目保持
   *  各自状态（重读后显示），不弹窗。 */
  const smokeAll = async (agentId: string, targets: ResourceModelEntry[]) => {
    if (busyRef.current !== null || targets.length === 0) return;
    batchStop.current = false;
    setBusy({ kind: 'batch', agent: agentId });
    setBatch({ agent: agentId, done: 0, total: targets.length });
    await runPool(
      targets,
      BATCH_LIMIT,
      (model) => commands.resourcesSmoke(model.agent, model.id, model.provider || undefined),
      {
        shouldStop: () => batchStop.current,
        onProgress: (done) => setBatch({ agent: agentId, done, total: targets.length }),
      },
    );
    await reload();
    setBatch(null);
    setBusy(null);
  };

  const smoke = async (model: ResourceModelEntry) => {
    const key = seatKey(model.agent, model.provider, model.id);
    setBusy({ kind: 'smoke', key });
    setRowErrors((current) => {
      const next = { ...current };
      delete next[key];
      return next;
    });
    try {
      await commands.resourcesSmoke(model.agent, model.id, model.provider || undefined);
    } catch (err) {
      setRowErrors((current) => ({ ...current, [key]: String(err) }));
    }
    await reload();
    setBusy(null);
  };

  /** 记下或清除（value 为 null）指定路径，成功后重新探测（TASK-66）。 */
  const savePath = async (agentId: string, value: string | null) => {
    if (busyRef.current !== null) return;
    const key = `path:${agentId}`;
    setBusy({ kind: 'path', agent: agentId });
    setRowErrors((current) => {
      const next = { ...current };
      delete next[key];
      return next;
    });
    try {
      await commands.resourcesSetPath(agentId, value);
    } catch (err) {
      setRowErrors((current) => ({ ...current, [key]: String(err) }));
      setBusy(null);
      return;
    }
    setPathDraft((current) => {
      const next = { ...current };
      delete next[agentId];
      return next;
    });
    setBusy(null);
    await detect();
  };

  const openPathEditor = (agent: ResourceAgentEntry) =>
    setPathDraft((current) => ({ ...current, [agent.id]: agent.custom_path ?? '' }));

  /** 「指定路径」按钮：没指定、也没在编辑时给出。 */
  const pathButton = (agent: ResourceAgentEntry) =>
    (agent.custom_path ?? '') === '' && pathDraft[agent.id] === undefined && (
      <button type="button" className="btn small" disabled={busy !== null} onClick={() => openPathEditor(agent)}>
        指定路径
      </button>
    );

  /** 指定路径的编辑行或当前路径行，以及保存失败的说明。 */
  const pathPanel = (agent: ResourceAgentEntry) => {
    const draft = pathDraft[agent.id];
    const current = agent.custom_path ?? '';
    const error = rowErrors[`path:${agent.id}`] ?? '';
    const saving = busy?.kind === 'path' && busy.agent === agent.id;
    return (
      <>
        {draft !== undefined ? (
          <div className="card-row path-row">
            <input
              className="path-input cfg-mono"
              value={draft}
              placeholder="可执行文件的完整路径"
              aria-label={`${agent.name || agent.id} 的可执行文件路径`}
              autoFocus
              onChange={(event) => setPathDraft((all) => ({ ...all, [agent.id]: event.target.value }))}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && draft.trim() !== '') void savePath(agent.id, draft.trim());
              }}
            />
            <button
              type="button"
              className="btn small primary"
              disabled={busy !== null || draft.trim() === ''}
              onClick={() => void savePath(agent.id, draft.trim())}
            >
              {saving ? '保存中…' : '保存'}
            </button>
            <button
              type="button"
              className="btn small"
              disabled={saving}
              onClick={() => setPathDraft((all) => {
                const next = { ...all };
                delete next[agent.id];
                return next;
              })}
            >
              取消
            </button>
          </div>
        ) : current !== '' && (
          <div className="card-row path-row">
            <span className="cfg-dim">指定路径</span>
            <span className="cfg-mono homepage-text" title={current}>{current}</span>
            <span className="cfg-spacer" />
            <button type="button" className="btn small" disabled={busy !== null} onClick={() => openPathEditor(agent)}>修改</button>
            <button type="button" className="btn small" disabled={busy !== null} onClick={() => void savePath(agent.id, null)}>
              {saving ? '清除中…' : '清除'}
            </button>
          </div>
        )}
        {error !== '' && <p className="cfg-error path-error">{error}</p>}
      </>
    );
  };

  const prune = async (agent: string | null) => {
    setPruneAsk(null);
    if (busyRef.current !== null) return;
    setBusy({ kind: 'prune', agent });
    setHeadError('');
    setReferenced(0);
    try {
      const result = await commands.resourcesPrune(agent);
      setReferenced(Array.isArray(result.referenced) ? result.referenced.length : 0);
    } catch (err) {
      setReferenced(0);
      setHeadError(String(err));
    }
    await reload();
    setBusy(null);
  };

  const allAgents = resources?.agents ?? [];
  const { installed: agents, notInstalled } = splitInstalled(allAgents);
  const missingTotal = missingCount(resources);
  const detected = detectStatus(resources?.detected_at, loadedAt);
  const detecting = busy?.kind === 'detect';
  const everDetected = (resources?.detected_at ?? '').trim() !== '';
  const detectLabel = detectButtonLabel({ detecting, progress, everDetected });

  const modelRow = (model: ResourceModelEntry, showProvider: boolean, agentState: string) => {
    const key = seatKey(model.agent, model.provider, model.id);
    const resolved = resolvedModelText(model);
    const smoking = busy?.kind === 'smoke' && busy.key === key;
    return (
      <div key={key} className="card-row model-row">
        <div className="model-row-main">
          <span className="cfg-mono model-row-id">{model.id}</span>
          {showProvider && model.provider !== '' && <span className="cfg-mono cfg-dim">{model.provider}</span>}
          {resolved !== '' && (
            <span className="cfg-dim">实际模型 <span className="cfg-mono">{resolved}</span></span>
          )}
          <span className="cfg-spacer" />
          <StateText state={model.state} />
          {canSmoke(model) && (
            <button
              type="button"
              className="icon-btn"
              title="测试连接"
              aria-label="测试连接"
              disabled={busy !== null}
              onClick={() => void smoke(model)}
            >
              <Icon name="play" />
              {smoking && '测试中…'}
            </button>
          )}
        </div>
        <NoteLine text={modelNote(model, agentState)} />
        {rowErrors[key] && <p className="cfg-error">{rowErrors[key]}</p>}
      </div>
    );
  };

  const overview = resourcesOverview(resources);
  const unavailableCount = agents.flatMap((agent) => agent.models).filter(isUnavailable).length;
  // 不可用数变成 0 后筛选自动失效。
  const filtering = filter === 'unavailable' && unavailableCount > 0;

  const toggleAgent = (agentId: string, open: boolean) => {
    setSavedOpen((current) => {
      const next = { ...current, [agentId]: !open };
      writeAgentOpen(pageTabStorage(), next);
      return next;
    });
  };

  return (
    <div className="page">
      <PageHeader
        left={resources !== null && (
          <>
            <span className="cfg-detected">{detected.label}</span>
            {unavailableCount > 0 && (
              <div className="segmented" role="tablist" aria-label="按状态筛选">
                <button
                  type="button"
                  role="tab"
                  aria-selected={!filtering}
                  className={`segmented-item${!filtering ? ' selected' : ''}`}
                  onClick={() => setFilter(null)}
                >
                  全部
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={filtering}
                  className={`segmented-item${filtering ? ' selected' : ''}`}
                  onClick={() => setFilter('unavailable')}
                >
                  不可用
                </button>
              </div>
            )}
          </>
        )}
        actions={(
          <>
            {missingTotal > 0 && (
              <button
                type="button"
                className="btn"
                disabled={busy !== null}
                onClick={() => setPruneAsk({ agent: null, count: missingTotal })}
              >
                <Icon name="trash" />
                {busy?.kind === 'prune' && busy.agent === null ? '清理中…' : `清理未发现（${missingTotal}）`}
              </button>
            )}
            <button type="button" className="btn primary" disabled={busy !== null} onClick={() => void detect()}>
              <Icon name="refresh" />
              {detectLabel}
            </button>
          </>
        )}
      />
      <div className="page-body">
        {resources !== null && !everDetected && !detecting && (
          <div className="cfg-banner empty-banner">
            <Icon name="server" />
            <span className="cfg-banner-text">探测本机 agent 与模型</span>
            <button type="button" className="btn primary" disabled={busy !== null} onClick={() => void detect()}>
              探测
            </button>
          </div>
        )}
        {resources !== null && everDetected && detected.stale && (
          <div className="cfg-banner">
            <Icon name="alert" />
            <span className="cfg-banner-text">{DETECT_STALE_HINT}</span>
            <button type="button" className="btn" disabled={busy !== null} onClick={() => void detect()}>重新探测</button>
          </div>
        )}
        {referenced > 0 && (
          <div className="cfg-banner alert">
            <Icon name="alert" />
            <span className="cfg-banner-text">{referencedText(referenced)}</span>
            <button type="button" className="btn primary" onClick={onGoModes}>去模式页</button>
            <button type="button" className="icon-btn" title="关闭" aria-label="关闭" onClick={() => setReferenced(0)}>
              <Icon name="close" />
            </button>
          </div>
        )}
        {headError !== '' && <p className="cfg-error cfg-head-error">{headError}</p>}
        {loadError !== '' && <p className="cfg-error cfg-head-error">{loadError}</p>}
        {resources === null && loadError === '' && <p className="cfg-empty">读取中…</p>}
        {resources !== null && (
          <StatCards
            items={overview}
            activeFilter={filtering ? 'unavailable' : null}
            onFilter={(next) => setFilter(next === 'unavailable' ? 'unavailable' : null)}
          />
        )}
        {resources !== null && allAgents.length === 0 && <p className="cfg-empty">暂无资源记录</p>}
        <div className="card-stack">
          {agents.map((agent) => {
            const stats = (
              <span className="card-sub">
                {agentStats(agent).map((part, index) => (
                  <span key={index} className={part.alert ? 'cfg-alert-text' : undefined}>
                    {index > 0 && ' · '}{part.text}
                  </span>
                ))}
              </span>
            );

            // 筛选「不可用」：只列这个 agent 的不可用模型（含未发现），没有就不出卡片。
            if (filtering) {
              const bad = groupByProvider(agent.models.filter(isUnavailable)).flatMap((group) => group.models);
              if (bad.length === 0) return null;
              return (
                <section key={agent.id} className="card">
                  <div className="card-row card-head">
                    <AgentIcon agent={agent.id} size={24} />
                    <span className="card-title">{agent.name || agent.id}</span>
                    {isUntested(agent) && <span className="pill muted">未实测</span>}
                    {versionText(agent.version) !== '' && <span className="pill mono">{versionText(agent.version)}</span>}
                    <span className="cfg-spacer" />
                    {stats}
                  </div>
                  {bad.map((model) => modelRow(model, true, agent.state))}
                </section>
              );
            }

            const expandable = agentExpandable(agent);
            const open = expandable && isAgentOpen(agent, savedOpen);
            const { listed, missing } = splitMissingModels(agent.models);
            const showAll = expanded[agent.id] === true;
            const { shown, hidden } = previewModels(listed, showAll);
            const groups = groupByProvider(shown);
            // 只有原生模型时不出小组标题；有 provider 时按组列出，行内不再重复 provider。
            const grouped = groups.length > 1 || (groups[0]?.provider ?? '') !== '';
            const missingOpen = showMissing[agent.id] === true;
            const version = versionText(agent.version);
            // 探测中、这个 agent 还没探测完：卡片头转圈。
            const pending = detecting && !detectedAgents.has(agent.id);
            const targets = batchSmokeTargets(agent);
            const batchButton = batchSmokeButton(agent);
            const running = batch && batch.agent === agent.id ? batch : null;
            const headContent = (
              <>
                <AgentIcon agent={agent.id} size={24} />
                <span className="card-title">{agent.name || agent.id}</span>
                {isUntested(agent) && <span className="pill muted">未实测</span>}
                {version !== '' && <span className="pill mono">{version}</span>}
                {pending && <span className="spin" aria-label="探测中"><Icon name="refresh" size={14} /></span>}
                <span className="cfg-spacer" />
                {stats}
              </>
            );
            return (
              <section key={agent.id} id={`agent-card-${agent.id}`} className="card">
                <div className="card-row card-head card-head-split">
                  {expandable ? (
                    <button
                      type="button"
                      className="card-toggle-inner"
                      aria-expanded={open}
                      onClick={() => toggleAgent(agent.id, open)}
                    >
                      {headContent}
                      <Icon name={open ? 'chevron-up' : 'chevron-down'} size={16} />
                    </button>
                  ) : (
                    <div className="card-toggle-inner static">{headContent}</div>
                  )}
                  {(batchButton.show || running !== null) && (
                    <span className="batch-actions">
                      <button
                        type="button"
                        className="btn small"
                        disabled={running === null && (busy !== null || batchButton.reason !== '')}
                        title={running === null && batchButton.reason !== '' ? batchButton.reason : undefined}
                        aria-busy={running !== null}
                        onClick={() => { if (!running) void smokeAll(agent.id, targets); }}
                      >
                        <Icon name="play" size={14} />
                        {batchSmokeLabel(running)}
                      </button>
                      {running && (
                        <button
                          type="button"
                          className="icon-btn"
                          title="取消"
                          aria-label="取消"
                          onClick={() => { batchStop.current = true; }}
                        >
                          <Icon name="close" />
                        </button>
                      )}
                    </span>
                  )}
                </div>
                {open && (
                  <>
                    {(agent.state !== '可用' || agent.note.trim() !== '') && (
                      <div className="card-row agent-state-row">
                        {agent.state !== '可用' && <StateText state={agent.state} />}
                        <NoteLine text={agent.note.trim()} />
                        {agent.state === '未安装' && (
                          <>
                            <span className="cfg-spacer" />
                            {pathButton(agent)}
                          </>
                        )}
                      </div>
                    )}
                    {pathPanel(agent)}
                    {!grouped && shown.map((model) => modelRow(model, true, agent.state))}
                    {grouped && groups.map((group) => (
                      <div key={group.provider}>
                        <div className="card-row card-group-title">
                          <ProviderIcon provider={group.provider} />
                          <span className="cfg-mono">{group.label}</span>
                        </div>
                        {group.models.map((model) => modelRow(model, false, agent.state))}
                      </div>
                    ))}
                    {(hidden > 0 || showAll) && listed.length > MODEL_PREVIEW_LIMIT && (
                      <button
                        type="button"
                        className="card-row card-more"
                        aria-expanded={showAll}
                        onClick={() => setExpanded((current) => ({ ...current, [agent.id]: !showAll }))}
                      >
                        {showAll ? '收起' : moreModelsText(hidden)}
                        <Icon name={showAll ? 'chevron-up' : 'chevron-down'} size={14} />
                      </button>
                    )}
                    {missing.length > 0 && (
                      <>
                        <div className="card-row missing-row">
                          <button
                            type="button"
                            className="missing-toggle"
                            aria-expanded={missingOpen}
                            onClick={() => setShowMissing((current) => ({ ...current, [agent.id]: !missingOpen }))}
                          >
                            {missingModelsText(missing.length)}
                            <Icon name={missingOpen ? 'chevron-up' : 'chevron-down'} size={14} />
                          </button>
                          <button
                            type="button"
                            className="btn small"
                            disabled={busy !== null}
                            onClick={() => setPruneAsk({ agent: agent.id, count: missing.length })}
                          >
                            {busy?.kind === 'prune' && busy.agent === agent.id ? '清理中…' : '清理'}
                          </button>
                        </div>
                        {missingOpen && missing.map((model) => (
                          <div key={seatKey(model.agent, model.provider, model.id)} className="card-row model-row missing">
                            <div className="model-row-main">
                              <span className="cfg-mono model-row-id">{model.id}</span>
                              {model.provider !== '' && <span className="cfg-mono">{model.provider}</span>}
                              <span className="cfg-spacer" />
                              <span className="cfg-state">{stateLabel(model.state)}</span>
                            </div>
                          </div>
                        ))}
                      </>
                    )}
                  </>
                )}
              </section>
            );
          })}
          {!filtering && notInstalled.length > 0 && (
            <section className="card">
              <div className="card-row card-head card-head-split">
                <button
                  type="button"
                  className="card-toggle-inner"
                  aria-expanded={showNotInstalled}
                  onClick={() => setShowNotInstalled((current) => !current)}
                >
                  <span className="card-title">{notInstalledText(notInstalled.length)}</span>
                  <span className="cfg-spacer" />
                  <Icon name={showNotInstalled ? 'chevron-up' : 'chevron-down'} size={16} />
                </button>
              </div>
              {showNotInstalled && notInstalled.map((agent) => {
                const homepage = agentHomepage(agent);
                return (
                  <div key={agent.id} id={`agent-card-${agent.id}`}>
                    <div className="card-row">
                      <AgentIcon agent={agent.id} size={20} />
                      <span>{agent.name || agent.id}</span>
                      <span className="cfg-spacer" />
                      {homepage !== '' && (
                        <>
                          <span className="cfg-mono cfg-dim homepage-text" title="官网">{homepage}</span>
                          <CopyButton text={homepage} icon />
                        </>
                      )}
                      {pathButton(agent)}
                    </div>
                    {pathPanel(agent)}
                  </div>
                );
              })}
            </section>
          )}
        </div>
      </div>
      {pruneAsk && (
        <ConfirmDialog
          title={pruneTitle(pruneAsk.count)}
          confirmLabel="清理"
          onConfirm={() => void prune(pruneAsk.agent)}
          onCancel={closePrune}
        />
      )}
    </div>
  );
}
