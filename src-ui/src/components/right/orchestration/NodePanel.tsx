// 节点面板（TASK-02 C2.3）：弹窗改一个节点的 Target 全字段，改动即写回图。
// TargetEditor 也独立导出，给预设构建器复用（构建器里的节点不在图上）。
// 用法注意：调用侧用 key={nodeId} 挂 NodePanel/TargetEditor，换节点即重置内部状态。

import { useState } from 'react';
import { createPortal } from 'react-dom';
import type {
  DispatchableModel,
  Graph,
  Target,
  Thinking,
  VisibilityKind,
  WriteMode,
} from '../../../graph/types';
import { roleDefaults } from '../../../graph/types';
import { defaultTurnTarget, updateNodeTarget } from '../../../graph/mutate';
import { ancestorsOf } from '../../../graph/layout';
import { validateNode } from '../../../graph/validate';
import { nodeLabel } from '../../../graph/label';
import './OrchestrationPanel.css';

const ROLES = ['出方案', '执行', '验收', '验证'];
const THINKINGS: Thinking[] = ['关', '低', '中', '高'];
const VISIBILITIES: VisibilityKind[] = ['共享', '盲', '指定上游'];
const WRITES: WriteMode[] = ['只读', '单写者', 'worktree'];

const MANUAL = '__manual';
const CUSTOM_ROLE = '__custom';

interface TargetEditorProps {
  target: Target;
  models: DispatchableModel[];
  /** visibility = 指定上游 时的候选祖先节点 */
  ancestors?: { id: string; label: string }[];
  /** validateNode 的提示（黄色胶囊，只标注不阻止） */
  hints?: string[];
  onChange: (t: Target) => void;
}

export function TargetEditor({ target, models, ancestors, hints, onChange }: TargetEditorProps) {
  const isBlank = target.agent === '' && target.model === '';
  const known = models.find((m) => m.agent === target.agent && m.id === target.model);
  const [manual, setManual] = useState(!isBlank && !known);
  const [customRole, setCustomRole] = useState(target.role !== '' && !ROLES.includes(target.role));

  const patch = (p: Partial<Target>) => onChange({ ...target, ...p });

  // 模型下拉按 agent 分组
  const byAgent = new Map<string, DispatchableModel[]>();
  for (const m of models) {
    const arr = byAgent.get(m.agent) ?? [];
    arr.push(m);
    byAgent.set(m.agent, arr);
  }

  const modelValue = manual ? MANUAL : isBlank ? '' : `${target.agent}/${target.model}`;

  return (
    <div className="target-editor">
      <div className="te-row">
        <label>模型</label>
        <select
          value={modelValue}
          onChange={(e) => {
            const v = e.target.value;
            if (v === MANUAL) {
              setManual(true);
              return;
            }
            setManual(false);
            if (v === '') {
              patch({ agent: '', model: '' });
              return;
            }
            const slash = v.indexOf('/');
            patch({ agent: v.slice(0, slash), model: v.slice(slash + 1) });
          }}
        >
          <option value="">未指定模型</option>
          {[...byAgent.entries()].map(([agent, arr]) => (
            <optgroup key={agent} label={agent}>
              {arr.map((m) => (
                <option key={`${m.agent}/${m.id}`} value={`${m.agent}/${m.id}`}>
                  {m.agent}/{m.id}
                </option>
              ))}
            </optgroup>
          ))}
          <option value={MANUAL}>手动输入…</option>
        </select>
      </div>
      {(manual || (models.length === 0 && !isBlank)) && (
        <div className="te-row te-manual">
          <label>手填</label>
          <input
            value={target.agent}
            placeholder="agent"
            onChange={(e) => patch({ agent: e.target.value.trim() })}
          />
          <span className="te-slash">/</span>
          <input
            value={target.model}
            placeholder="模型 id"
            onChange={(e) => patch({ model: e.target.value.trim() })}
          />
        </div>
      )}
      {manual && !known && !isBlank && <div className="te-note">手填的模型标「未接入」。</div>}
      <div className="te-row">
        <label>provider</label>
        <input
          value={target.provider ?? ''}
          placeholder="留空按通道默认"
          onChange={(e) => patch({ provider: e.target.value === '' ? undefined : e.target.value })}
        />
      </div>
      <div className="te-row">
        <label>角色</label>
        <select
          value={customRole ? CUSTOM_ROLE : target.role}
          onChange={(e) => {
            const v = e.target.value;
            if (v === CUSTOM_ROLE) {
              setCustomRole(true);
              patch({ role: '' });
              return;
            }
            setCustomRole(false);
            // 换角色带上角色默认值（共享/盲 + 写权限），用户之后仍可逐项改
            patch({ role: v, ...roleDefaults(v) });
          }}
        >
          {ROLES.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
          <option value={CUSTOM_ROLE}>自定义…</option>
        </select>
      </div>
      {customRole && (
        <div className="te-row">
          <label>自定义角色</label>
          <input value={target.role} onChange={(e) => patch({ role: e.target.value })} />
        </div>
      )}
      <div className="te-row">
        <label>档位</label>
        <input
          value={target.tier ?? ''}
          placeholder="如 高 / 低"
          onChange={(e) => patch({ tier: e.target.value === '' ? undefined : e.target.value })}
        />
      </div>
      <div className="te-row">
        <label>thinking</label>
        <select value={target.thinking} onChange={(e) => patch({ thinking: e.target.value as Thinking })}>
          {THINKINGS.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
      </div>
      <div className="te-row">
        <label>可见性</label>
        <select
          value={target.visibility}
          onChange={(e) => patch({ visibility: e.target.value as VisibilityKind })}
        >
          {VISIBILITIES.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </select>
      </div>
      {target.visibility === '指定上游' && ancestors && (
        <div className="te-row te-upstreams">
          <label>上游节点</label>
          <div className="te-upstream-list">
            {ancestors.length === 0 && <span className="te-note">没有祖先节点。</span>}
            {ancestors.map((a) => {
              const checked = (target.upstreams ?? []).includes(a.id);
              return (
                <label key={a.id} className="te-upstream-item">
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={() => {
                      const cur = new Set(target.upstreams ?? []);
                      if (checked) cur.delete(a.id);
                      else cur.add(a.id);
                      patch({ upstreams: [...cur] });
                    }}
                  />
                  <span>{a.label}</span>
                </label>
              );
            })}
          </div>
        </div>
      )}
      <div className="te-row">
        <label>写权限</label>
        <select value={target.write} onChange={(e) => patch({ write: e.target.value as WriteMode })}>
          {WRITES.map((w) => (
            <option key={w} value={w}>
              {w}
            </option>
          ))}
        </select>
      </div>
      {hints && hints.length > 0 && (
        <div className="te-hints">
          {hints.map((h) => (
            <span key={h} className="hint-capsule">
              {h}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

interface NodePanelProps {
  graph: Graph;
  nodeId: string;
  models: DispatchableModel[];
  onCommit: (g: Graph) => void;
  onClose: () => void;
}

export function NodePanel({ graph, nodeId, models, onCommit, onClose }: NodePanelProps) {
  const node = graph.nodes[nodeId];
  if (!node) return null;
  const hints = validateNode(graph, nodeId, models);
  const ancestors = ancestorsOf(graph, nodeId).map((id) => ({
    id,
    label: nodeLabel(graph.nodes[id]),
  }));

  return createPortal(
    <div
      className="np-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="np-panel" role="dialog" aria-label="节点">
        <div className="np-head">
          <span className="np-title">{nodeLabel(node)}</span>
          <button className="np-close" onClick={onClose}>
            关闭
          </button>
        </div>
        {node.kind === 'material' ? (
          <div className="np-body">
            <p className="te-note">材料节点没有派发目标。</p>
            {hints.length > 0 && (
              <div className="te-hints">
                {hints.map((h) => (
                  <span key={h} className="hint-capsule">
                    {h}
                  </span>
                ))}
              </div>
            )}
          </div>
        ) : (
          <div className="np-body">
            <TargetEditor
              target={node.target ?? defaultTurnTarget()}
              models={models}
              ancestors={ancestors}
              hints={hints}
              onChange={(t) => onCommit(updateNodeTarget(graph, nodeId, t))}
            />
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
