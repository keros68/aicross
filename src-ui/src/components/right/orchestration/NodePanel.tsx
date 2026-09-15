// 节点面板（TASK-02 C2.3）：弹窗改一个节点的 Target 全字段，改动即写回图。
// TargetEditor 也独立导出，给预设构建器复用（构建器里的节点不在图上）。
// 用法注意：调用侧用 key={nodeId} 挂 NodePanel/TargetEditor，换节点即重置内部状态。

import { useState } from 'react';
import { createPortal } from 'react-dom';
import type {
  DispatchableModel,
  Graph,
  SwarmItem,
  Target,
  Thinking,
  VisibilityKind,
  WriteMode,
} from '../../../graph/types';
import { roleDefaults } from '../../../graph/types';
import { defaultTurnTarget, updateNodeSwarm, updateNodeTarget, updateNodeUserText } from '../../../graph/mutate';
import { ancestorsOf } from '../../../graph/layout';
import { validateNode } from '../../../graph/validate';
import { nodeLabel } from '../../../graph/label';
import { redraftNode } from '../../../graph/dispatch';
import {
  groupPickableModels,
  modelKey,
  modelStatusSuffix,
  modelSelectValue,
  parseModelSelectValue,
  targetSelectValue,
} from '../../../resources/model-picker';
import { agentModelDisplay } from '../../../resources/agent-display';
import './OrchestrationPanel.css';

const ROLES = ['出方案', '拆分', '执行', '验收', '验证', '顾问', '仲裁'];
const THINKINGS: Thinking[] = ['关', '低', '中', '高'];
const VISIBILITIES: VisibilityKind[] = ['共享', '盲', '指定上游'];
const WRITES: WriteMode[] = ['只读', '单写者', 'worktree'];

const MANUAL = '__manual';
const CUSTOM_ROLE = '__custom';

// 遗留节点（TASK-12）的面板：只读清单，可整份复制；没有派发目标可改。
function LeftoverList({ line, items }: { line: string; items: string[] }) {
  const [copied, setCopied] = useState(false);
  const copy = () => {
    const text = items.join('\n');
    const write = navigator.clipboard?.writeText(text);
    if (write) write.then(() => setCopied(true)).catch(() => setCopied(false));
  };
  return (
    <div className="np-body">
      <div className="te-row">
        <label>遗留清单</label>
        <div className="te-note te-readonly">{line}</div>
      </div>
      <ol className="np-leftover">
        {items.map((item, index) => (
          <li key={`${index}-${item}`}>{item}</li>
        ))}
      </ol>
      <div className="te-row">
        <button onClick={copy}>复制清单</button>
        {copied && <span className="te-note">已复制</span>}
      </div>
      <p className="te-note te-readonly">遗留节点不派发；处理完可在来源验收节点上再分叉。</p>
    </div>
  );
}

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

  // 模型下拉按 agent · provider 分组（pi 一家挂三个 provider，只按 agent 分会挤成一组）；
  // 顺序沿用后端排好的「原生 / 已验证 → 未验证 → 验证失败」（09-15 起按 verification）。
  const groups = groupPickableModels(models);

  // 下拉的 value 是 [agent, provider, id] 三元组（见 model-picker.ts）：只有把
  // provider 编进 value，选中时才能把它一起写回 Target；否则 pi 三个 provider 下的
  // 同名模型区分不开，Target 的 provider 留空，派发 argv 的 --provider 段就是空的。
  const modelValue = manual ? MANUAL : isBlank ? '' : targetSelectValue(target);
  // 老图里的 Target 可能只有 agent + model 没有 provider，在清单里对不上任何一项。
  // 那就额外挂一条「当前值」选项如实显示它现在是什么（同 DispatchOverlay 的做法），
  // 用户重选一次即可绑上 provider；不做静默回填，免得界面显示的 provider 和图里存的不一致。
  const listed = models.some((m) => modelSelectValue(m) === modelValue);

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
              patch({ agent: '', model: '', provider: undefined });
              return;
            }
            const picked = parseModelSelectValue(v);
            if (picked) patch(picked);
          }}
        >
          <option value="">未指定模型</option>
          {!manual && !isBlank && !listed && (
            <option value={modelValue}>
              {agentModelDisplay(target.agent, target.model, target.provider)}
            </option>
          )}
          {groups.map((group) => (
            <optgroup key={group.label} label={group.label}>
              {group.models.map((m) => (
                <option key={modelKey(m)} value={modelSelectValue(m)}>
                  {agentModelDisplay(m.agent, m.id, m.provider)}{modelStatusSuffix(m)}
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
      <div className="te-row">
        <label>备选</label>
        <select
          value={JSON.stringify(target.fallback ? [target.fallback.agent, target.fallback.provider ?? '', target.fallback.model] : ['', '', ''])}
          onChange={(e) => {
            const [agent, provider, model] = JSON.parse(e.target.value) as string[];
            patch({ fallback: model === '' ? undefined : { agent, provider: provider === '' ? undefined : provider, model } });
          }}
        >
          <option value={JSON.stringify(['', '', ''])}>无</option>
          {target.fallback && !models.some((m) => m.agent === target.fallback!.agent && (m.provider ?? '') === (target.fallback!.provider ?? '') && m.id === target.fallback!.model) && (
            <option value={JSON.stringify([target.fallback.agent, target.fallback.provider ?? '', target.fallback.model])}>
              {agentModelDisplay(target.fallback.agent, target.fallback.model)}
            </option>
          )}
          {groups.map((group) => (
            <optgroup key={group.label} label={group.label}>
              {group.models.map((m) => (
                <option key={modelKey(m)} value={modelSelectValue(m)}>
                  {agentModelDisplay(m.agent, m.id, m.provider)}{modelStatusSuffix(m)}
                </option>
              ))}
            </optgroup>
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

// 蜂群清单条目编辑（TASK-08 B-4）：G1 判出范围重叠后，用户在这里改 files
// 直到不重叠。文件范围一行一条 glob——受控 textarea 不能边打边过滤空行，
// 原文留在本组件的局部状态里，写回图时才拆行去空。非 draft 节点只读。
export function SwarmEditor(
  { swarm, editable, onChange }: { swarm: SwarmItem; editable: boolean; onChange: (next: SwarmItem) => void },
) {
  const [filesText, setFilesText] = useState(swarm.files.join('\n'));
  const commitFiles = (text: string) => {
    setFilesText(text);
    onChange({ ...swarm, files: text.split('\n').map((line) => line.trim()).filter((line) => line !== '') });
  };
  return (
    <>
      <div className="te-row">
        <label>文件范围</label>
        {editable ? (
          <textarea
            rows={3}
            aria-label="文件范围"
            value={filesText}
            placeholder="一行一条 glob，如 src/a/**"
            onChange={(e) => commitFiles(e.target.value)}
          />
        ) : (
          <div className="te-note te-readonly">{swarm.files.join('、') || '未声明'}</div>
        )}
      </div>
      <div className="te-row">
        <label>验收命令</label>
        {editable ? (
          <input
            aria-label="验收命令"
            value={swarm.accept}
            placeholder="可空，空则用设置里的验证命令"
            onChange={(e) => onChange({ ...swarm, accept: e.target.value })}
          />
        ) : (
          <div className="te-note te-readonly">{swarm.accept || '用设置里的验证命令'}</div>
        )}
      </div>
      <div className="te-note">第 {swarm.item} 件，合并按件号顺序进行。</div>
    </>
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
        {(node.reply?.status === 'failed' || node.reply?.status === 'quota_exhausted') && (
          <div className="np-body"><button onClick={() => onCommit(redraftNode(graph, nodeId))}>重派</button></div>
        )}
        {node.leftover ? (
          <LeftoverList line={node.user.text} items={node.leftover.items} />
        ) : node.kind === 'material' ? (
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
            {(node.reply?.status === 'draft' || node.user.text !== '') && (
              <div className="te-row">
                <label>节点消息</label>
                {node.reply?.status === 'draft' ? (
                  <textarea
                    rows={3}
                    value={node.user.text}
                    placeholder="自定义视角"
                    onChange={(e) => onCommit(updateNodeUserText(graph, nodeId, e.target.value))}
                  />
                ) : (
                  <div className="te-note te-readonly">{node.user.text}</div>
                )}
              </div>
            )}
            {node.swarm && (
              <SwarmEditor
                swarm={node.swarm}
                editable={node.reply?.status === 'draft'}
                onChange={(next) => onCommit(updateNodeSwarm(graph, nodeId, next))}
              />
            )}
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
