// 右栏「方案与编排」（TASK-02 C2）：预设下拉 + 子图预览 + 节点面板 +
// 构建器 + 讲解。派发执行由中栏 DispatchOverlay 承担；图变更统一走 commitGraph。
// 文案直接中文硬编码（同 GraphTab 的选择，见 REPORT-02）。

import { useEffect, useMemo, useState } from 'react';
import { useAppState } from '../../../store/app-state';
import { commands, isTauri } from '../../../tauri';
import type { DispatchableModel, Graph, Preset, Stage, Target } from '../../../graph/types';
import { stamp, descendantsOf } from '../../../graph/stamp';
import { findModel, validateNode } from '../../../graph/validate';
import { depthMap } from '../../../graph/layout';
import { commitGraph, recordStamp } from '../../../graph/persist';
import { vendorColor } from '../../../graph/vendor';
import { nodeLabel } from '../../../graph/label';
import { NodePanel } from './NodePanel';
import { PresetBuilder } from './PresetBuilder';
import { ExplainOverlay } from './ExplainOverlay';
import { RESOURCES_CHANGED_EVENT } from '../../../resources/mention';
import { applyRoleDefaults } from '../../../graph/dispatch';
import './OrchestrationPanel.css';

// 后置期次的模式：下拉里置灰 + 标期次，不实现（TASK-02 A8）。
const FUTURE_PRESETS = ['擂台', '评审团', '仲裁', '前提抽查', '主备', '顾问', '红蓝', '群聊', '蜂群'];

export function OrchestrationPanel() {
  const { state, dispatch } = useAppState();
  const [models, setModels] = useState<DispatchableModel[]>([]);
  const [modelsError, setModelsError] = useState<string | null>(() =>
    isTauri ? null : '未在应用内运行，无法读取资源',
  );
  const [presets, setPresets] = useState<Preset[]>([]);
  const [selectedPresetId, setSelectedPresetId] = useState<string>('');
  // 选了未应用的预设 → 预览里虚线追加；应用后清空（子图已经在图上）
  const [ghost, setGhost] = useState<Preset | null>(null);
  const [builderOpen, setBuilderOpen] = useState(false);
  const [explainOpen, setExplainOpen] = useState(false);
  const [panelNode, setPanelNode] = useState<string | null>(null);
  const [roles, setRoles] = useState<{ planning: Target; execution: Target; acceptance: Target } | null>(null);

  const focus = state.graphFocus;
  const graph: Graph | undefined = focus ? state.graphs[focus.graphId] : undefined;
  const focusNode = graph && focus ? graph.nodes[focus.nodeId] : undefined;

  const refreshPresets = () => {
    if (!isTauri) return;
    commands
      .listPresets()
      .then((f) => setPresets(f.presets))
      .catch(() => {});
  };

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
        if (dead) return;
        setPresets(f.presets);
        // 默认选中经济模式（产品的主线预设），没有则第一个
        setSelectedPresetId((cur) => {
          if (cur !== '') return cur;
          const economy = f.presets.find((p) => p.id === 'builtin-economy');
          return (economy ?? f.presets[0])?.id ?? '';
        });
      })
      .catch(() => {});
    return () => {
      dead = true;
    };
  }, []);

  useEffect(() => {
    if (isTauri) commands.loadRoles().then((value) => setRoles(value as unknown as { planning: Target; execution: Target; acceptance: Target })).catch(() => {});
  }, []);

  useEffect(() => {
    const reloadRoles = () => { if (isTauri) commands.loadRoles().then((value) => setRoles(value as unknown as { planning: Target; execution: Target; acceptance: Target })).catch(() => {}); };
    window.addEventListener('aicross:presets-changed', reloadRoles);
    window.addEventListener('aicross:presets-changed', refreshPresets);
    window.addEventListener(RESOURCES_CHANGED_EVENT, refreshPresets);
    return () => {
      window.removeEventListener('aicross:presets-changed', reloadRoles);
      window.removeEventListener('aicross:presets-changed', refreshPresets);
      window.removeEventListener(RESOURCES_CHANGED_EVENT, refreshPresets);
    };
  }, []);

  // 接入页写盘后重拉可派发模型（TASK-07 修正轮 R1：纯追加监听，不改既有
  // 加载逻辑）——「尚未接入资源」提示与节点面板模型下拉都吃这份 state。
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

  const commit = (g: Graph) => commitGraph(dispatch, g);

  const selectedPreset = presets.find((p) => p.id === selectedPresetId) ?? null;

  const applyPreset = (preset: Preset | null) => {
    if (!graph || !focus || !focusNode || !preset) return;
    const before = new Set(Object.keys(graph.nodes));
    const effective = roles ? { ...preset, stages: preset.stages.map((stage) => ({ ...stage, nodes: stage.nodes.map((target) => applyRoleDefaults(target, roles)) })) } : preset;
    const g2 = stamp(graph, focus.nodeId, effective);
    recordStamp(
      graph.id,
      Object.keys(g2.nodes).filter((id) => !before.has(id)),
      preset.name,
    );
    commit(g2);
    setGhost(null);
  };

  const saveUserPreset = async (name: string, stages: Stage[]) => {
    const newPreset: Preset = {
      id: crypto.randomUUID(),
      name,
      builtin: false,
      stages,
    };
    const users = [...presets.filter((p) => !p.builtin), newPreset];
    await commands.saveUserPresets(users);
    refreshPresets();
    setSelectedPresetId(newPreset.id);
  };

  // 子图预览：高亮节点 + 全部后代，按相对深度分成阶段条
  const previewStages = useMemo(() => {
    if (!graph || !focus || !focusNode) return [];
    const depths = depthMap(graph);
    const base = depths.get(focus.nodeId) ?? 0;
    const ids = [focus.nodeId, ...descendantsOf(graph, focus.nodeId)];
    const byStage = new Map<number, string[]>();
    for (const id of ids) {
      const rel = (depths.get(id) ?? base) - base;
      const arr = byStage.get(rel) ?? [];
      arr.push(id);
      byStage.set(rel, arr);
    }
    return [...byStage.entries()].sort((a, b) => a[0] - b[0]).map(([, v]) => v);
  }, [graph, focus, focusNode]);

  const previewChip = (id: string) => {
    if (!graph) return null;
    const node = graph.nodes[id];
    if (!node) return null;
    const hit = node.target ? findModel(models, node.target) : undefined;
    const hints = validateNode(graph, id, models);
    return (
      <button
        key={id}
        className={`orch-chip${node.kind === 'accept' ? ' accept' : ''}`}
        onClick={() => setPanelNode(id)}
        title={hints.length > 0 ? hints.join('；') : undefined}
      >
        <span
          className="graph-node-dot"
          style={{ background: vendorColor(node.target?.agent, hit?.vendor) }}
        />
        <span className="orch-chip-label">{nodeLabel(node)}</span>
        {node.target?.role && <span className="orch-chip-role">{node.target.role}</span>}
        {hints.map((h) => (
          <span key={h} className="hint-capsule">
            {h}
          </span>
        ))}
      </button>
    );
  };

  return (
    <div className="orch-panel">
      {/* 顶部：预设下拉 + 应用 / 编辑编排 / 讲解 */}
      <div className="orch-toolbar">
        <select
          className="orch-preset-select"
          value={selectedPresetId}
          onChange={(e) => {
            setSelectedPresetId(e.target.value);
            setGhost(presets.find((p) => p.id === e.target.value) ?? null);
          }}
        >
          {selectedPresetId === '' && <option value="">选择预设</option>}
          {presets.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
              {p.builtin ? '' : '（我的）'}
            </option>
          ))}
          {FUTURE_PRESETS.map((n) => (
            <option key={n} disabled value={`future-${n}`}>
              {n} · 后续期次
            </option>
          ))}
        </select>
        <div className="orch-toolbar-btns">
          <button
            className="orch-btn orch-btn-primary"
            disabled={!focusNode || !selectedPreset}
            onClick={() => applyPreset(selectedPreset)}
          >
            应用
          </button>
          <button className="orch-btn" onClick={() => setBuilderOpen(true)}>
            编辑编排
          </button>
          <button className="orch-btn" onClick={() => setExplainOpen(true)}>
            讲解
          </button>
        </div>
      </div>

      {/* 子图预览 */}
      <div className="orch-preview">
        {!focusNode && <div className="orch-empty">在图上选一个节点</div>}
        {focusNode && (
          <div className="orch-stages">
            {previewStages.map((ids, i) => (
              <div key={i} className="orch-stage-wrap">
                {i > 0 && <span className="orch-arrow">→</span>}
                <div className="orch-stage">{ids.map(previewChip)}</div>
              </div>
            ))}
            {ghost &&
              ghost.stages.map((stage, i) => (
                <div key={`ghost-${i}`} className="orch-stage-wrap">
                  <span className="orch-arrow">→</span>
                  <div className="orch-stage">
                    {stage.nodes.map((t, j) => (
                      <span key={j} className="orch-chip orch-chip-ghost">
                        <span
                          className="graph-node-dot"
                          style={{
                            background: vendorColor(t.agent, findModel(models, t)?.vendor),
                          }}
                        />
                        <span className="orch-chip-label">
                          {t.agent !== '' ? `${t.agent}/${t.model === '' ? '?' : t.model}` : t.model || '未指定模型'}
                        </span>
                        <span className="orch-chip-role">{t.role}</span>
                      </span>
                    ))}
                  </div>
                </div>
              ))}
          </div>
        )}
        {modelsError && models.length === 0 && (
          <div className="orch-note">尚未接入资源（{modelsError}）</div>
        )}
      </div>

      {panelNode && graph && graph.nodes[panelNode] && (
        <NodePanel
          key={panelNode}
          graph={graph}
          nodeId={panelNode}
          models={models}
          onCommit={commit}
          onClose={() => setPanelNode(null)}
        />
      )}

      {builderOpen && (
        <PresetBuilder
          initial={selectedPreset}
          models={models}
          canApply={!!focusNode}
          onApply={(preset) => {
            applyPreset(preset);
            setBuilderOpen(false);
          }}
          onSave={saveUserPreset}
          onClose={() => setBuilderOpen(false)}
        />
      )}

      {explainOpen && <ExplainOverlay onClose={() => setExplainOpen(false)} />}
    </div>
  );
}
