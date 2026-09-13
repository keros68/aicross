// 中栏「对话图」tab（TASK-02 C1）：上半会话图（按深度分层、左→右生长），
// 下半 1–3 列聊天面板。本期不做派发执行：发送只产生 draft 节点；
// 图变更统一走 commitGraph（全局状态 + save_graph 落盘）。
// 文案直接中文硬编码（见 REPORT-02：接入 11 语言 i18n 字典成本过高）。

import { useEffect, useMemo, useRef, useState } from 'react';
import { useAppState, type TerminalSession } from '../../../store/app-state';
import { commands, isTauri, type GraphMeta } from '../../../tauri';
import type { DispatchableModel, Graph, GraphNode, Preset } from '../../../graph/types';
import { newGraph } from '../../../graph/types';
import { contextOf } from '../../../graph/context';
import { stamp } from '../../../graph/stamp';
import { findModel, validateNode } from '../../../graph/validate';
import { latestLeaf, layersOf, normalizeNodeKinds, pathTo, sameBranch } from '../../../graph/layout';
import { appendTurn, defaultTurnTarget, fillRoot, parseSend } from '../../../graph/mutate';
import { commitGraph, presetNameOf, recordStamp } from '../../../graph/persist';
import { vendorColor } from '../../../graph/vendor';
import { nodeLabel } from '../../../graph/label';
import { NodePanel } from '../../right/orchestration/NodePanel';
import './GraphTab.css';

// 节点卡固定尺寸，图区横向滚动；不做缩放、不做拖拽（任务书硬约束）。
const NODE_W = 168;
const NODE_H = 56;
const GAP_X = 64;
const GAP_Y = 40; // 留出高亮节点下方「分叉」「盖预设」按钮的位置，不压下一行
const ACTIONS_H = 32; // 节点下方按钮行高度，加进画布高度，末行按钮不被底边裁掉

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
  const [columns, setColumns] = useState<string[]>([]);
  const [stampFor, setStampFor] = useState<string | null>(null);
  const [panelNode, setPanelNode] = useState<string | null>(null);
  const [ctxOpen, setCtxOpen] = useState<Record<number, boolean>>({});
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [failedId, setFailedId] = useState<string | null>(null);

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
        if (!dead) setMetas([]);
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

  const applyStamp = (nodeId: string, preset: Preset) => {
    if (!graph) return;
    const before = new Set(Object.keys(graph.nodes));
    const g2 = stamp(graph, nodeId, preset);
    recordStamp(
      graph.id,
      Object.keys(g2.nodes).filter((id) => !before.has(id)),
      preset.name,
    );
    commit(g2);
    setStampFor(null);
  };

  const sendInColumn = (colIdx: number) => {
    if (!graph) return;
    const raw = (drafts[colIdx] ?? '').trim();
    if (raw === '') return;
    const endId = columns[colIdx];
    const endNode = endId ? graph.nodes[endId] : undefined;
    if (!endNode) return;
    const { text, target } = parseSend(raw, models);
    if (text === '') return;
    setDrafts((d) => ({ ...d, [colIdx]: '' }));
    // 列末端是根且根消息为空 → 用户的第一句填进根节点（标题取首行前 20 字）
    if (endId === graph.root && endNode.user.text === '' && endNode.user.attachments.length === 0) {
      commit(fillRoot(graph, text));
      return;
    }
    // 没写 @ 沿用列末端 Target；根节点没有 Target 时用「执行」默认值
    const finalTarget = target ?? endNode.target ?? defaultTurnTarget();
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
    return (
      <div className="graph-tab graph-start">
        <div className="graph-start-inner">
          <div className="graph-start-title">对话图</div>
          <div className="graph-start-desc">
            会话是一张从左向右生长的图。发第一句得到根节点，@ 模型分叉，盖预设编排。
          </div>
          <button className="graph-primary-btn" onClick={createGraph}>
            新建对话图
          </button>
          {metas === null && <div className="graph-start-empty">读取中…</div>}
          {metas !== null && metas.length === 0 && (
            <div className="graph-start-empty">还没有保存的图</div>
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
    const status =
      node.kind !== 'material' && node.reply?.status === 'draft' ? '待派' : null;
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
            <button
              className="graph-mini-btn"
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
                setStampFor(stampFor === id ? null : id);
              }}
            >
              盖预设
            </button>
          </div>
        )}
        {stampFor === id && (
          <div className="graph-stamp-menu" style={{ left: p.x, top: p.y + NODE_H + 34 }}>
            {presets.map((preset) => (
              <button
                key={preset.id}
                className="graph-stamp-item"
                onClick={(e) => {
                  e.stopPropagation();
                  applyStamp(id, preset);
                }}
              >
                {preset.name}
              </button>
            ))}
            {presets.length === 0 && <div className="graph-stamp-empty">无可用预设</div>}
          </div>
        )}
      </div>
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
            <div className="graph-msg-meta">{nodeLabel(node)}</div>
            {node.reply && node.reply.status !== 'draft' && node.reply.status !== 'pending' ? (
              <div className="graph-msg-text">{node.reply.text}</div>
            ) : (
              <div className="graph-msg-pending">待派</div>
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
    const grouped = new Map<string, DispatchableModel[]>();
    if (atMatch) {
      for (const m of models) {
        if (filter !== '' && !`${m.agent}/${m.id}`.toLowerCase().includes(filter)) continue;
        const arr = grouped.get(m.agent) ?? [];
        arr.push(m);
        grouped.set(m.agent, arr);
      }
    }
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
                {[...grouped.entries()].map(([agent, arr]) => (
                  <div key={agent}>
                    <div className="graph-at-group">{agent}</div>
                    {arr.map((m) => (
                      <button
                        key={`${m.agent}/${m.id}`}
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
                        {m.agent}/{m.id}
                      </button>
                    ))}
                  </div>
                ))}
              </div>
            )}
            <input
              className="graph-input"
              value={value}
              placeholder="@agent/模型 分叉，回车发送"
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
    </div>
  );
}
