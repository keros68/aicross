// 中栏「对话图」tab（TASK-02 C1）：上半会话图（按深度分层、左→右生长），
// 下半 1–3 列聊天面板。draft 节点经 G1/G2/G3 派发，结果先写回 Reply，
// 再由 commitGraph 持久化。
// 文案直接中文硬编码（见 REPORT-02：接入 11 语言 i18n 字典成本过高）。

import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useAppState, type TerminalSession } from '../../../store/app-state';
import { commands, isTauri, type GraphMeta } from '../../../tauri';
import type { DispatchableModel, Graph, GraphNode, Preset } from '../../../graph/types';
import { newGraph } from '../../../graph/types';
import { contextOf } from '../../../graph/context';
import { childrenOf, stamp } from '../../../graph/stamp';
import { findModel, validateNode } from '../../../graph/validate';
import { latestLeaf, layersOf, normalizeNodeKinds, pathTo, sameBranch } from '../../../graph/layout';
import { appendTurn, defaultTurnTarget, fillRoot, parseSend, removeDraftNode } from '../../../graph/mutate';
import { commitGraph, presetNameOf, recordStamp } from '../../../graph/persist';
import { vendorColor } from '../../../graph/vendor';
import { nodeLabel } from '../../../graph/label';
import { NodePanel } from '../../right/orchestration/NodePanel';
import { applyReconcile, applyRoleDefaults, dispatchableNodeIds, EMPTY_RECONCILE_UI, fallbackNoticeOf, nodeStatusLabel, parseAdvise, presetWithRoleDefaults, RECONCILE_UNKNOWN_TEXT, reconcileUiAfter, reconcileUiFor, redraftNode, resetPendingToDraft, type ReconcileSummary, type ReconcileUi } from '../../../graph/dispatch';
import { DispatchOverlay } from './DispatchOverlay';
import { appendMention, INSERT_MENTION_EVENT, openResourcesPage, RESOURCES_CHANGED_EVENT } from '../../../resources/mention';
import { groupPickableModels, modelKey, modelOptionLabel } from '../../../resources/model-picker';
import { stampMenuShouldClose } from '../../../graph/stamp-menu';
import { requestOrchestrationPanel } from '../../../lib/open-orchestration';
import { FirstRunGuide } from './FirstRunGuide';
import { shouldShowFirstRun, type FirstRunStepKey } from './first-run';
import './GraphTab.css';

// 节点卡固定尺寸，图区横向滚动；不做缩放、不做拖拽（任务书硬约束）。
const NODE_W = 168;
const NODE_H = 56;
const GAP_X = 64;
const GAP_Y = 40; // 留出高亮节点下方「分叉」「盖预设」按钮的位置，不压下一行
const ACTIONS_H = 32; // 节点下方按钮行高度，加进画布高度，末行按钮不被底边裁掉
// 「盖预设」菜单（portal + fixed）夹进视口用的尺寸，和 .graph-stamp-menu 的
// min-width / 最小可读高度对齐；菜单本身仍按内容自适应。
const STAMP_MENU_W = 140;
const STAMP_MENU_MIN_H = 120;

export function GraphTab({ session }: { session: TerminalSession }) {
  const { state, dispatch } = useAppState();
  const graphId = session.toolData ?? '';
  const graph = graphId !== '' ? state.graphs[graphId] : undefined;

  const [models, setModels] = useState<DispatchableModel[]>([]);
  const [modelsError, setModelsError] = useState<string | null>(() =>
    isTauri ? null : '未在应用内运行，无法读取资源',
  );
  const [presets, setPresets] = useState<Preset[]>([]);
  const [metas, setMetas] = useState<GraphMeta[] | null>(() => (isTauri ? null : []));
  const [metasError, setMetasError] = useState(false);
  const [columns, setColumns] = useState<string[]>([]);
  const [stampFor, setStampFor] = useState<string | null>(null);
  // 「盖预设」菜单改走 portal：它原来是 .graph-canvas-inner 里的 absolute 元素，
  // 而 .graph-canvas 是 max-height:44% + overflow:auto 的滚动容器，菜单一超过画布
  // 下边沿就被裁掉（下半聊天面板并没有「压住」它，是画布自己把它切了）。
  // 这里记下触发按钮的视口坐标，菜单挂到 document.body 上按 fixed 定位。
  const [stampAt, setStampAt] = useState<{ left: number; top: number } | null>(null);
  const [panelNode, setPanelNode] = useState<string | null>(null);
  const [ctxOpen, setCtxOpen] = useState<Record<number, boolean>>({});
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [failedId, setFailedId] = useState<string | null>(null);
  const [dispatchOpen, setDispatchOpen] = useState(false);
  // 资源是否已读过一轮。models 初值是 []，没有这个标记会把「还没读」
  // 误判成「一个资源都没有」，首启引导就会闪一下。
  const [modelsRead, setModelsRead] = useState(false);
  const [unfinishedRun, setUnfinishedRun] = useState<{ runId: string; nodeIds: string[] } | null>(null);
  // 崩溃后对账判「未知」的节点（TASK-06-1）：G1 标「待核对」，默认不勾选派发。
  // 这份状态属于某一张图，带 graphId 存；换图后 reconcileUiFor 过滤掉（R1-1）。
  const [reconcile, setReconcile] = useState<ReconcileUi>(EMPTY_RECONCILE_UI);
  // 节点卡按过「重派」的节点（TASK-06-2）：单写者的那些在 G1 带 rollbackBefore。
  const [retryNodes, setRetryNodes] = useState<string[]>([]);
  const [roles, setRoles] = useState<any>(null);

  // 左栏「资源」tab 点模型时往主面板（列 0）输入框插入 @agent/模型（TASK-07）。
  // 纯追加监听：既有 drafts/发送逻辑不变。
  useEffect(() => {
    const onInsert = (event: Event) => {
      const text = (event as CustomEvent<{ text?: string }>).detail?.text;
      if (!text) return;
      setDrafts((d) => ({ ...d, 0: appendMention(d[0] ?? '', text) }));
    };
    window.addEventListener(INSERT_MENTION_EVENT, onInsert);
    return () => window.removeEventListener(INSERT_MENTION_EVENT, onInsert);
  }, []);

  useEffect(() => {
    if (isTauri) commands.loadRoles().then(setRoles).catch(() => {});
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    const reload = () => commands.listPresets().then((f) => setPresets(f.presets)).catch(() => {});
    window.addEventListener('aicross:presets-changed', reload);
    window.addEventListener(RESOURCES_CHANGED_EVENT, reload);
    return () => {
      window.removeEventListener('aicross:presets-changed', reload);
      window.removeEventListener(RESOURCES_CHANGED_EVENT, reload);
    };
  }, []);

  // 接入页写盘后重拉可派发模型（TASK-07 修正轮 R1）：@ 弹层与节点面板
  // 下拉都吃这份 models state。纯追加监听，不改既有加载逻辑。
  useEffect(() => {
    if (!isTauri) return;
    let dead = false;
    const reload = () => {
      commands
        .listDispatchableModels()
        .then((r) => {
          if (dead) return;
          setModels(r.models);
          setModelsError(r.error);
        })
        .catch(() => {
          if (!dead) setModelsError('读取资源失败');
        });
    };
    window.addEventListener(RESOURCES_CHANGED_EVENT, reload);
    return () => {
      dead = true;
      window.removeEventListener(RESOURCES_CHANGED_EVENT, reload);
    };
  }, []);

  // 可派发模型与预设（命令幂等便宜；右栏 OrchestrationPanel 也各自拉取）
  useEffect(() => {
    if (!isTauri) return;
    let dead = false;
    commands
      .listDispatchableModels()
      .then((r) => {
        if (dead) return;
        setModels(r.models);
        setModelsError(r.error);
        setModelsRead(true);
      })
      .catch(() => {
        if (!dead) setModelsError('读取资源失败');
      });
    commands
      .listPresets()
      .then((f) => {
        if (!dead) setPresets(f.presets);
      })
      .catch(() => {});
    return () => {
      dead = true;
    };
  }, []);

  useEffect(() => {
    if (!graph || !isTauri || !session.folderPath) return;
    commands.inspectDispatchState(session.folderPath)
      .then((state) => setUnfinishedRun(state.unfinished
        ? { runId: state.runId, nodeIds: state.activeNodeIds }
        : null))
      .catch(() => setUnfinishedRun(null));
  }, [graph?.id, session.folderPath]);

  // toolData 有图 id 但全局没载入 → 从 ~/.aicross/graphs/ 读
  useEffect(() => {
    if (graphId === '' || graph || !isTauri) return;
    let dead = false;
    commands
      .loadGraph(graphId)
      .then((g) => {
        if (dead) return;
        if (g) dispatch({ type: 'SET_GRAPH', graph: normalizeNodeKinds(g) });
        else setFailedId(graphId);
      })
      .catch(() => {
        if (!dead) setFailedId(graphId);
      });
    return () => {
      dead = true;
    };
  }, [graphId, graph, dispatch]);
  const loadFailed = failedId === graphId;

  // 开始视图（toolData 为空）：列已有图
  useEffect(() => {
    if (graphId !== '' || !isTauri) return;
    let dead = false;
    commands
      .listGraphs()
      .then((list) => {
        if (!dead) setMetas(list);
      })
      .catch(() => {
        // 列表读不出来 ≠ 一张图都没有。metas 仍置 [] 让列表区停止转圈，
        // 但 metasError 会把首启引导的 graphCount 变成「不知道」，
        // 免得在图状态未知时弹引导（R1 修正）。
        if (!dead) {
          setMetas([]);
          setMetasError(true);
        }
      });
    return () => {
      dead = true;
    };
  }, [graphId]);

  // 列末端随图变化修剪；初次载入落到最新叶节点（渲染期调整，不走 effect）
  const [prevGraph, setPrevGraph] = useState<Graph | undefined>(undefined);
  if (graph !== prevGraph) {
    setPrevGraph(graph);
    if (graph) {
      setColumns((cols) => {
        const valid = cols.filter((id) => graph.nodes[id]);
        if (valid.length === 0) return [latestLeaf(graph)];
        return valid.length === cols.length ? cols : valid;
      });
    }
  }

  const commit = (g: Graph) => commitGraph(dispatch, g);

  // 「继续」先对账（TASK-06-1）：completed 直接收回留痕正文与产物，incomplete
  // 回 draft 重跑，unknown 回 draft 并标「待核对」。对账命令未接通（后端还没接
  // 上这条命令）或返回 exists:false 时，退回原有的「pending 全部重置为 draft」。
  const continueUnfinished = async () => {
    if (!graph) return;
    if (unfinishedRun?.runId) {
      await Promise.all(unfinishedRun.nodeIds.map((nodeId) =>
        commands.cancelDispatchNode(unfinishedRun.runId, nodeId).catch(() => false)));
    }
    let summary: ReconcileSummary | null = null;
    try {
      summary = await commands.reconcileDispatchState(session.folderPath ?? '');
    } catch {
      summary = null;
    }
    commit(applyReconcile(resetPendingToDraft(graph), summary));
    setReconcile(reconcileUiAfter(graph.id, summary));
    setUnfinishedRun(null);
    setDispatchOpen(true);
  };

  const abandonUnfinished = () => {
    if (!graph) return;
    const nodes = Object.fromEntries(Object.entries(graph.nodes).map(([id, node]) => [id,
      node.reply?.status === 'pending'
        ? { ...node, reply: { ...node.reply, status: 'failed' as const, text: '上次派发已放弃' } }
        : node,
    ]));
    commit({ ...graph, nodes });
    commands.abandonDispatchState(session.folderPath ?? '').catch(() => {});
    setUnfinishedRun(null);
    setReconcile(EMPTY_RECONCILE_UI);
  };

  // 换图后这份状态不跟着走（R1-1）：id 对不上就当空的。
  const reconcileUi = reconcileUiFor(reconcile, graphId);

  const focusNodeId =
    state.graphFocus?.graphId === graphId ? state.graphFocus.nodeId : null;

  // 高亮节点变化时把节点卡连同它下方的「分叉」「盖预设」按钮一起滚入视野；
  // 只滚节点卡的话，末行节点的按钮会留在图区底边之外
  const canvasRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!focusNodeId) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    canvas.querySelector('.graph-node.focused')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    canvas.querySelector('.graph-node-actions')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [focusNodeId]);

  // 「盖预设」菜单按打开那一刻的按钮坐标固定在视口上，画布一滚动或窗口一改尺寸，
  // 它就会和按钮脱节 —— 这种情况直接收起菜单（和点别处收起是同一种结果）。
  // scroll 用捕获阶段，因为滚的是画布不是 window。
  useEffect(() => {
    if (stampFor === null) return;
    const close = () => {
      setStampFor(null);
      setStampAt(null);
    };
    window.addEventListener('scroll', close, true);
    window.addEventListener('resize', close);
    return () => {
      window.removeEventListener('scroll', close, true);
      window.removeEventListener('resize', close);
    };
  }, [stampFor]);

  useEffect(() => {
    if (stampFor === null) return;
    const close = () => closeStamp();
    const onKey = (event: KeyboardEvent) => { if (stampMenuShouldClose(event)) close(); };
    const onPointer = (event: MouseEvent) => { if (stampMenuShouldClose(event)) close(); };
    window.addEventListener('keydown', onKey);
    window.addEventListener('mousedown', onPointer, true);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('mousedown', onPointer, true);
    };
  }, [stampFor]);

  const openGraph = (id: string) => {
    if (!isTauri) return;
    commands
      .loadGraph(id)
      .then((g) => {
        if (!g) return;
        dispatch({ type: 'SET_GRAPH', graph: normalizeNodeKinds(g) });
        dispatch({ type: 'SET_TERMINAL_TOOL', id: session.id, tool: 'graph', toolData: id });
      })
      .catch((err) => console.warn('[graph] 打开失败', err));
  };

  const createGraph = () => {
    const g = newGraph();
    dispatch({ type: 'SET_GRAPH', graph: g });
    commands.saveGraph(g).catch((err) => console.warn('[graph] 保存失败', err));
    dispatch({ type: 'SET_TERMINAL_TOOL', id: session.id, tool: 'graph', toolData: g.id });
    dispatch({ type: 'SET_GRAPH_FOCUS', focus: { graphId: g.id, nodeId: g.root } });
  };

  // 同一分支只保留一个面板：列末端与新节点在同一条祖先链上即同分支
  const onSameBranch = (a: string, b: string) => (graph ? sameBranch(graph, a, b) : a === b);

  // 双击：主面板（第 1 列）对齐到该节点；其它列里同分支的面板收掉
  const alignMain = (nodeId: string) =>
    setColumns((cols) => [nodeId, ...cols.slice(1).filter((c) => !onSameBranch(c, nodeId))]);

  // Ctrl+双击：固定到第 2/3 列。同分支的列被替换；满 3 列时后开的替换最后一列。
  const pinColumn = (nodeId: string) =>
    setColumns((cols) => {
      const idx = cols.findIndex((c) => onSameBranch(c, nodeId));
      if (idx >= 0) return cols.map((c, i) => (i === idx ? nodeId : c));
      if (cols.length < 3) return [...cols, nodeId];
      return [...cols.slice(0, 2), nodeId];
    });

  const forkFrom = (nodeId: string) => {
    if (!graph) return;
    const { graph: g2, nodeId: child } = appendTurn(graph, nodeId, '');
    commit(g2);
    dispatch({ type: 'SET_GRAPH_FOCUS', focus: { graphId: graph.id, nodeId: child } });
    setPanelNode(child); // 弹出节点面板填 Target
  };

  function closeStamp() {
    setStampFor(null);
    setStampAt(null);
  }

  const applyStamp = (nodeId: string, preset: Preset) => {
    if (!graph) return;
    const before = new Set(Object.keys(graph.nodes));
    // 擂台跳过角色默认填充：三路目标留空，G1 确认屏逐家选（不然会填成同一家）
    const effective = roles ? presetWithRoleDefaults(preset, roles) : preset;
    const g2 = stamp(graph, nodeId, effective);
    recordStamp(
      graph.id,
      Object.keys(g2.nodes).filter((id) => !before.has(id)),
      preset.name,
    );
    commit(g2);
    closeStamp();
  };

  const sendInColumn = (colIdx: number) => {
    if (!graph) return;
    const raw = (drafts[colIdx] ?? '').trim();
    if (raw === '') return;
    const endId = columns[colIdx];
    const endNode = endId ? graph.nodes[endId] : undefined;
    if (!endNode) return;
    // /advise @agent/model 问题 → 顾问节点（盲、只读、只要方案不要执行）
    const advise = parseAdvise(raw, models);
    if (advise) {
      if (advise.question === '') return;
      setDrafts((d) => ({ ...d, [colIdx]: '' }));
      const { graph: g2, nodeId } = appendTurn(graph, endId, advise.question, advise.target);
      commit(g2);
      setColumns((cols) => cols.map((c, i) => (i === colIdx ? nodeId : c)));
      dispatch({ type: 'SET_GRAPH_FOCUS', focus: { graphId: graph.id, nodeId } });
      return;
    }
    const { text, target } = parseSend(raw, models);
    if (text === '') return;
    setDrafts((d) => ({ ...d, [colIdx]: '' }));
    // 列末端是根且根消息为空 → 用户的第一句填进根节点（标题取首行前 20 字）
    if (endId === graph.root && endNode.user.text === '' && endNode.user.attachments.length === 0) {
      commit(fillRoot(graph, text));
      return;
    }
    // 没写 @ 沿用列末端 Target；根节点没有 Target 时用「执行」默认值
    const finalTarget = target ?? endNode.target ?? (roles ? applyRoleDefaults(defaultTurnTarget(), roles) : defaultTurnTarget());
    const { graph: g2, nodeId } = appendTurn(graph, endId, text, finalTarget);
    commit(g2);
    setColumns((cols) => cols.map((c, i) => (i === colIdx ? nodeId : c)));
    dispatch({ type: 'SET_GRAPH_FOCUS', focus: { graphId: graph.id, nodeId } });
  };

  // ── 布局：按深度分层，同层竖排并相对最高层垂直居中 ──────────────────────
  const layout = useMemo(() => {
    if (!graph) return null;
    const layers = layersOf(graph);
    const maxLen = Math.max(...layers.map((l) => l.length), 1);
    const height = maxLen * NODE_H + (maxLen - 1) * GAP_Y;
    const pos = new Map<string, { x: number; y: number }>();
    layers.forEach((ids, depth) => {
      const layerH = ids.length * NODE_H + (ids.length - 1) * GAP_Y;
      const offY = (height - layerH) / 2;
      ids.forEach((id, i) =>
        pos.set(id, { x: depth * (NODE_W + GAP_X), y: offY + i * (NODE_H + GAP_Y) }),
      );
    });
    const width = layers.length * NODE_W + Math.max(layers.length - 1, 0) * GAP_X;
    return { pos, width, height };
  }, [graph]);

  // ── 开始视图 ────────────────────────────────────────────────────────────
  if (graphId === '') {
    const firstRunFacts = {
      // 读不出图列表时同样按「不知道」处理。
      graphCount: metas === null || metasError ? null : metas.length,
      // 读不出资源时按「不知道」处理，引导不出现（宁可不提示，也不误判）。
      resourceCount: modelsError || !modelsRead ? null : models.length,
      folderPath: session.folderPath ?? null,
    };
    const onFirstRunStep = (key: FirstRunStepKey) => {
      if (key === 'folder') {
        import('@tauri-apps/plugin-dialog')
          .then(({ open }) => open({ directory: true }))
          .then((picked) => {
            if (typeof picked === 'string') dispatch({ type: 'SET_FOLDER', path: picked });
          })
          .catch(() => {});
        return;
      }
      if (key === 'resources') {
        openResourcesPage();
        return;
      }
      // 选预设：右栏切到「方案与编排」，预设下拉就在那儿。右栏收起时先展开，
      // 否则这一步点了没有可见反应。展开的那一下 RightPanel 还没 mount，
      // 光发事件会丢，所以走 requestOrchestrationPanel（事件 + 一次性标记）。
      if (state.rightPanelHidden) dispatch({ type: 'TOGGLE_RIGHT_PANEL' });
      requestOrchestrationPanel();
    };

    return (
      <div className="graph-tab graph-start">
        <div className="graph-start-inner">
          <div className="graph-start-title">对话图</div>
          <div className="graph-start-desc">
            会话是一张从左向右生长的图。发第一句得到根节点，@ 模型分叉，盖预设编排。
          </div>
          {shouldShowFirstRun(firstRunFacts) && (
            <FirstRunGuide facts={firstRunFacts} onStep={onFirstRunStep} />
          )}
          <button className="graph-primary-btn" onClick={createGraph}>
            新建对话图
          </button>
          {metas === null && <div className="graph-start-empty">读取中…</div>}
          {metas !== null && metas.length === 0 && (
            <div className="graph-start-empty">
              {metasError ? '读取已保存的图失败' : '还没有保存的图'}
            </div>
          )}
          {metas !== null && metas.length > 0 && (
            <>
              <div className="graph-start-sub">已有图</div>
              <div className="graph-start-list">
                {metas.map((m) => (
                  <button key={m.id} className="graph-start-item" onClick={() => openGraph(m.id)}>
                    <span className="graph-start-item-title">{m.title || '对话图'}</span>
                    <span className="graph-start-item-time">
                      {m.updated_at ? new Date(m.updated_at).toLocaleString('zh-CN') : ''}
                    </span>
                  </button>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    );
  }

  if (!graph || !layout) {
    return (
      <div className="graph-tab graph-center-note">
        {loadFailed ? (
          <>
            <div>找不到这张图，可能已被删除。</div>
            <button
              className="graph-primary-btn"
              onClick={() =>
                dispatch({ type: 'SET_TERMINAL_TOOL', id: session.id, tool: 'graph', toolData: '' })
              }
            >
              返回
            </button>
          </>
        ) : (
          <div>读取中…</div>
        )}
      </div>
    );
  }

  // ── 会话图 ──────────────────────────────────────────────────────────────
  const edges: React.ReactNode[] = [];
  for (const [id, p] of layout.pos) {
    const node = graph.nodes[id];
    for (const parent of node.parents) {
      const pp = layout.pos.get(parent);
      if (!pp) continue;
      const x1 = pp.x + NODE_W;
      const y1 = pp.y + NODE_H / 2;
      const x2 = p.x;
      const y2 = p.y + NODE_H / 2;
      edges.push(
        <path
          key={`${parent}-${id}`}
          className="graph-edge"
          d={`M ${x1} ${y1} C ${x1 + 28} ${y1}, ${x2 - 28} ${y2}, ${x2} ${y2}`}
        />,
      );
    }
  }

  const renderNodeCard = (id: string) => {
    const node = graph.nodes[id];
    const p = layout.pos.get(id);
    if (!node || !p) return null;
    const isFocus = id === focusNodeId;
    const hit = node.target ? findModel(models, node.target) : undefined;
    const color = vendorColor(node.target?.agent, hit?.vendor);
    const status = nodeStatusLabel(node);
    const disconnected = validateNode(graph, id, models).includes('未接入');
    return (
      <div key={id}>
        <div
          className={`graph-node${isFocus ? ' focused' : ''}${node.kind === 'accept' ? ' accept' : ''}`}
          style={{ left: p.x, top: p.y, width: NODE_W, height: NODE_H }}
          onClick={() =>
            dispatch({ type: 'SET_GRAPH_FOCUS', focus: { graphId: graph.id, nodeId: id } })
          }
          onDoubleClick={(e) => {
            if (e.ctrlKey || e.metaKey) pinColumn(id);
            else alignMain(id);
          }}
          title="单击高亮 · 双击对齐主面板 · Ctrl+双击固定到侧列"
        >
          <div className="graph-node-row">
            <span className="graph-node-dot" style={{ background: color }} />
            <span className="graph-node-model">{nodeLabel(node)}</span>
          </div>
          <div className="graph-node-row">
            {node.target?.role && <span className="graph-node-role">{node.target.role}</span>}
            {node.kind === 'accept' && <span className="graph-node-role">汇合</span>}
            {status && <span className="graph-node-status">{status}</span>}
            {disconnected && <span className="hint-capsule">未接入</span>}
          </div>
        </div>
        {isFocus && (
          <div className="graph-node-actions" style={{ left: p.x, top: p.y + NODE_H + 4 }}>
            {node.reply?.status === 'failed' && (
              <button className="graph-mini-btn" onClick={(e) => { e.stopPropagation(); commit(redraftNode(graph, id)); setRetryNodes((current) => [...new Set([...current, id])]); }}>
                重派
              </button>
            )}
            {node.reply?.status === 'draft' && childrenOf(graph, id).length === 0 && (
              <button className="graph-mini-btn" onClick={(e) => { e.stopPropagation(); commit(removeDraftNode(graph, id)); }}>
                删除
              </button>
            )}
            <button
              className="graph-mini-btn graph-stamp-trigger"
              onClick={(e) => {
                e.stopPropagation();
                forkFrom(id);
              }}
            >
              分叉
            </button>
            <button
              className="graph-mini-btn"
              onClick={(e) => {
                e.stopPropagation();
                if (stampFor === id) {
                  closeStamp();
                  return;
                }
                const rect = e.currentTarget.getBoundingClientRect();
                setStampAt({ left: rect.left, top: rect.bottom + 4 });
                setStampFor(id);
              }}
            >
              盖预设
            </button>
          </div>
        )}
      </div>
    );
  };

  // 菜单本体：挂在 document.body 上，按触发按钮的视口坐标 fixed 定位，
  // 再夹一次防止贴着窗口右/下边缘时溢出屏幕。
  const renderStampMenu = () => {
    if (stampFor === null || stampAt === null) return null;
    const nodeId = stampFor;
    const left = Math.min(stampAt.left, Math.max(8, window.innerWidth - STAMP_MENU_W - 8));
    const top = Math.min(stampAt.top, Math.max(8, window.innerHeight - STAMP_MENU_MIN_H - 8));
    return createPortal(
      <div className="graph-stamp-menu" style={{ left, top }}>
        {presets.map((preset) => (
          <button
            key={preset.id}
            className="graph-stamp-item"
            onClick={(e) => {
              e.stopPropagation();
              applyStamp(nodeId, preset);
            }}
          >
            {preset.name}
          </button>
        ))}
        {presets.length === 0 && <div className="graph-stamp-empty">无可用预设</div>}
      </div>,
      document.body,
    );
  };

  // ── 聊天面板 ────────────────────────────────────────────────────────────
  const renderTurn = (node: GraphNode) => {
    const hasUser = node.user.text !== '' || node.user.attachments.length > 0;
    const showReplySlot = node.kind !== 'material';
    return (
      <div
        key={node.id}
        className={`graph-turn${node.id === focusNodeId ? ' focused' : ''}`}
        onClick={() =>
          dispatch({ type: 'SET_GRAPH_FOCUS', focus: { graphId: graph.id, nodeId: node.id } })
        }
      >
        {hasUser && (
          <div className="graph-msg graph-msg-user">
            {node.user.text !== '' && <div className="graph-msg-text">{node.user.text}</div>}
            {node.user.attachments.length > 0 && (
              <div className="graph-msg-att">附件：{node.user.attachments.join('、')}</div>
            )}
          </div>
        )}
        {showReplySlot && (
          <div className="graph-msg graph-msg-reply">
            <div className="graph-msg-meta">
              {nodeLabel(node)}
              {(node.reply?.tokens_in !== undefined || node.reply?.tokens_out !== undefined) && (
                <span> · in {node.reply?.tokens_in ?? '—'} / out {node.reply?.tokens_out ?? '—'}</span>
              )}
              {node.reply?.adopted && <span> · 已采纳</span>}
              {node.target?.role === '顾问' && <span> · 顾问意见</span>}
              {fallbackNoticeOf(node.reply) && <span> · {fallbackNoticeOf(node.reply)}</span>}
            </div>
            {node.reply && node.reply.status !== 'draft' && node.reply.status !== 'pending' ? (
              <div className="graph-msg-text">{node.reply.text}</div>
            ) : (
              // 对账判「未知」的节点（R1-10）：这句说明要看得到，不然
              // 「请检查项目目录后重派或放弃」只在 G1 里闪一次
              <div className="graph-msg-pending">{node.reply?.skipped_reason ? `已跳过：${node.reply.skipped_reason}` : node.reply?.status === 'pending' ? '运行中' : node.reply?.text === RECONCILE_UNKNOWN_TEXT ? RECONCILE_UNKNOWN_TEXT : '待派'}</div>
            )}
          </div>
        )}
      </div>
    );
  };

  const renderColumn = (endId: string, colIdx: number) => {
    const endNode = graph.nodes[endId];
    if (!endNode) return null;
    const path = pathTo(graph, endId);
    const ctx = contextOf(graph, endId);
    const visibility = endNode.target?.visibility ?? '共享';
    const open = !!ctxOpen[colIdx];
    const hints = validateNode(graph, endId, models);
    const value = drafts[colIdx] ?? '';
    const atMatch = value.match(/@([^\s@]*)$/);
    const filter = (atMatch?.[1] ?? '').toLowerCase();
    // 与节点面板同一套分组（agent · provider）与状态后缀，见 resources/model-picker.ts
    const grouped = atMatch
      ? groupPickableModels(
          models.filter(
            (m) => filter === '' || `${m.agent}/${m.id}`.toLowerCase().includes(filter),
          ),
        )
      : [];
    const pickModel = (m: DispatchableModel) => {
      const v = drafts[colIdx] ?? '';
      const nm = v.match(/@([^\s@]*)$/);
      const base = nm ? v.slice(0, v.length - nm[0].length) : v;
      setDrafts((d) => ({ ...d, [colIdx]: `${base}@${m.agent}/${m.id} ` }));
    };
    const stampName = presetNameOf(graph.id, endId);

    return (
      <div className="graph-col" key={colIdx}>
        <div className="graph-col-head">
          <span className="graph-col-title">{nodeLabel(endNode)}</span>
          <button
            className="graph-col-ctx"
            onClick={() => setCtxOpen((s) => ({ ...s, [colIdx]: !open }))}
          >
            上下文 {ctx.length} 条 · {visibility}
          </button>
        </div>
        {hints.length > 0 && (
          <div className="graph-hints">
            {hints.map((h) => (
              <span key={h} className="hint-capsule">
                {h}
              </span>
            ))}
          </div>
        )}
        {open && (
          <div className="graph-ctx-preview">
            {ctx.length === 0 && <div className="graph-ctx-empty">（空上下文）</div>}
            {ctx.map((m, i) => (
              <div key={i} className="graph-ctx-item">
                <div className="graph-ctx-text">{m.text === '' ? '（空）' : m.text}</div>
                {m.attachments.length > 0 && (
                  <div className="graph-ctx-att">附件：{m.attachments.join('、')}</div>
                )}
              </div>
            ))}
          </div>
        )}
        <div className="graph-col-body">{path.map((id) => renderTurn(graph.nodes[id]))}</div>
        <div className="graph-col-input-area">
          {colIdx === 0 && (
            <button
              className="graph-current-node"
              title="打开右栏「方案与编排」"
              onClick={() => window.dispatchEvent(new CustomEvent('aicross:open-orchestration'))}
            >
              当前节点：{nodeLabel(endNode)} · {stampName ?? '手动'}
            </button>
          )}
          <div className="graph-input-wrap">
            {atMatch && (
              <div className="graph-at-popup">
                {models.length === 0 && (
                  <div className="graph-at-empty">
                    尚未接入资源{modelsError ? `（${modelsError}）` : ''}，可直接手输 agent/模型
                  </div>
                )}
                {grouped.map((group) => (
                  <div key={group.label}>
                    <div className="graph-at-group">{group.label}</div>
                    {group.models.map((m) => (
                      <button
                        key={modelKey(m)}
                        className="graph-at-item"
                        onMouseDown={(e) => {
                          e.preventDefault();
                          pickModel(m);
                        }}
                      >
                        <span
                          className="graph-node-dot"
                          style={{ background: vendorColor(m.agent, m.vendor) }}
                        />
                        {modelOptionLabel(m)}
                      </button>
                    ))}
                  </div>
                ))}
              </div>
            )}
            <input
              className="graph-input"
              value={value}
              placeholder="@agent/模型 分叉，/advise 问顾问，回车发送"
              onChange={(e) => setDrafts((d) => ({ ...d, [colIdx]: e.target.value }))}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.nativeEvent.isComposing) sendInColumn(colIdx);
              }}
            />
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className="graph-tab">
      <div className="graph-dispatch-toolbar">
        {unfinishedRun && <span className="graph-resume">上次派发未完成；若上次是崩溃退出，请先手动检查残留进程 <button onClick={continueUnfinished}>继续</button><button onClick={abandonUnfinished}>放弃</button></span>}
        {!unfinishedRun && reconcileUi.note !== '' && <span className="graph-resume">{reconcileUi.note}</span>}
        <span>可派 {dispatchableNodeIds(graph).length} 路</span>
        <button className="graph-primary-btn" disabled={dispatchableNodeIds(graph).length === 0} onClick={() => setDispatchOpen(true)}>派发</button>
      </div>
      <div className="graph-canvas" ref={canvasRef}>
        <div
          className="graph-canvas-inner"
          style={{ width: layout.width, height: layout.height + ACTIONS_H }}
        >
          <svg className="graph-edges" width={layout.width} height={layout.height}>
            {edges}
          </svg>
          {Object.keys(graph.nodes).map(renderNodeCard)}
        </div>
      </div>
      <div className="graph-columns">{columns.map(renderColumn)}</div>
      {renderStampMenu()}
      {panelNode && graph.nodes[panelNode] && (
        <NodePanel
          key={panelNode}
          graph={graph}
          nodeId={panelNode}
          models={models}
          onCommit={commit}
          onClose={() => setPanelNode(null)}
        />
      )}
      {dispatchOpen && (
        <DispatchOverlay
          graph={graph}
          models={models}
          projectDir={session.folderPath ?? ''}
          needsReview={reconcileUi.needsReview}
          retryNodes={retryNodes}
          onCommit={commit}
          onClose={() => { setDispatchOpen(false); setRetryNodes([]); }}
          onFocusNode={(nodeId) => {
            dispatch({ type: 'SET_GRAPH_FOCUS', focus: { graphId: graph.id, nodeId } });
            setColumns((cols) => [nodeId, ...cols.slice(1).filter((c) => c !== nodeId)]);
          }}
        />
      )}
    </div>
  );
}
