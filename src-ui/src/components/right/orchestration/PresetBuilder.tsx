// 预设构建器（TASK-02 C2.5）：阶段按行编辑，底部一行记法与表单双向同步。
// 记法失焦解析回表单；解析失败红字提示且**不覆盖表单**。
// 编辑内置预设后「保存为预设」= 存成新的用户预设（builtin=false，后端再防御一层）。

import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import type { DispatchableModel, Preset, Stage, Target } from '../../../graph/types';
import { cloneTarget } from '../../../graph/types';
import { parseNotation, printNotation } from '../../../graph/notation';
import { findModel } from '../../../graph/validate';
import { defaultTurnTarget } from '../../../graph/mutate';
import { vendorColor } from '../../../graph/vendor';
import { TargetEditor } from './NodePanel';
import './OrchestrationPanel.css';

interface PresetBuilderProps {
  initial: Preset | null;
  models: DispatchableModel[];
  canApply: boolean;
  onApply: (preset: Preset) => void;
  onSave: (name: string, stages: Stage[]) => Promise<void>;
  onClose: () => void;
}

function cloneStages(stages: Stage[]): Stage[] {
  return stages.map((s) => ({ nodes: s.nodes.map(cloneTarget) }));
}

function formPreset(stages: Stage[]): Preset {
  return { id: '', name: '', builtin: false, stages };
}

function targetLabel(t: Target): string {
  if (t.agent === '' && t.model === '') return '未指定模型';
  if (t.agent !== '') return `${t.agent}/${t.model === '' ? '?' : t.model}`;
  return t.model;
}

export function PresetBuilder({ initial, models, canApply, onApply, onSave, onClose }: PresetBuilderProps) {
  const [stages, setStages] = useState<Stage[]>(() =>
    initial ? cloneStages(initial.stages) : [{ nodes: [defaultTurnTarget()] }],
  );
  const [notation, setNotation] = useState(() => printNotation(formPreset(stages)).text);
  const [notationError, setNotationError] = useState<{ pos: number; message: string } | null>(null);
  const [unresolved, setUnresolved] = useState<string[]>([]);
  const [lossy, setLossy] = useState<string[]>(() => printNotation(formPreset(stages)).lossy);
  const [editing, setEditing] = useState<{ si: number; ni: number } | null>(null);
  const [saveAsk, setSaveAsk] = useState(false);
  const [saveName, setSaveName] = useState('');
  const [saveBusy, setSaveBusy] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  // 表单 → 记法：stages 每次由表单变化时重打正则形式
  useEffect(() => {
    const printed = printNotation(formPreset(stages));
    setNotation(printed.text);
    setLossy(printed.lossy);
    setNotationError(null);
  }, [stages]);

  // 记法 → 表单：失焦解析；失败红字（含位置），不覆盖表单。
  // 成功时关掉节点编辑器：stages 数组被整体替换，editing 索引可能越界。
  const onNotationBlur = () => {
    const r = parseNotation(notation, models);
    if (r.error) {
      setNotationError(r.error);
      return;
    }
    setNotationError(null);
    setUnresolved(r.unresolved ?? []);
    if (r.preset) {
      setEditing(null);
      setStages(r.preset.stages);
    }
  };

  const patchTarget = (si: number, ni: number, t: Target) =>
    setStages((ss) =>
      ss.map((s, i) =>
        i === si ? { nodes: s.nodes.map((n, j) => (j === ni ? t : n)) } : s,
      ),
    );

  const addNode = (si: number) =>
    setStages((ss) => ss.map((s, i) => (i === si ? { nodes: [...s.nodes, defaultTurnTarget()] } : s)));

  const removeNode = (si: number, ni: number) => {
    setEditing(null);
    setStages((ss) => {
      const s = ss[si];
      if (s.nodes.length <= 1) return ss.filter((_, i) => i !== si); // 删空 = 删阶段
      return ss.map((st, i) => (i === si ? { nodes: st.nodes.filter((_, j) => j !== ni) } : st));
    });
  };

  const addStage = () => setStages((ss) => [...ss, { nodes: [defaultTurnTarget()] }]);

  const removeStage = (si: number) => {
    setEditing(null);
    setStages((ss) => (ss.length <= 1 ? ss : ss.filter((_, i) => i !== si)));
  };

  const moveStage = (si: number, dir: -1 | 1) => {
    setEditing(null);
    setStages((ss) => {
      const j = si + dir;
      if (j < 0 || j >= ss.length) return ss;
      const next = [...ss];
      [next[si], next[j]] = [next[j], next[si]];
      return next;
    });
  };

  const doSave = async () => {
    const name = saveName.trim();
    if (name === '') return;
    setSaveBusy(true);
    setSaveError(null);
    try {
      await onSave(name, cloneStages(stages));
      setSaveAsk(false);
      setSaveName('');
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setSaveBusy(false);
    }
  };

  return createPortal(
    <div
      className="np-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="builder" role="dialog" aria-label="编辑编排">
        <div className="np-head">
          <span className="np-title">编辑编排{initial ? `：${initial.name}` : ''}</span>
          <button className="np-close" onClick={onClose}>
            关闭
          </button>
        </div>
        {initial?.builtin && <div className="builder-note">内置预设不可改，保存后存为自定义模式。</div>}

        <div className="builder-stages">
          {stages.map((stage, si) => (
            <div key={si} className="builder-stage">
              <div className="builder-stage-head">
                <span className="builder-stage-name">阶段 {si + 1}</span>
                <span className="builder-stage-ops">
                  <button className="graph-mini-btn" onClick={() => addNode(si)}>
                    新建节点
                  </button>
                  <button
                    className="graph-mini-btn"
                    disabled={si === 0}
                    onClick={() => moveStage(si, -1)}
                  >
                    上移
                  </button>
                  <button
                    className="graph-mini-btn"
                    disabled={si === stages.length - 1}
                    onClick={() => moveStage(si, 1)}
                  >
                    下移
                  </button>
                  <button
                    className="graph-mini-btn"
                    disabled={stages.length <= 1}
                    onClick={() => removeStage(si)}
                  >
                    删除阶段
                  </button>
                </span>
              </div>
              <div className="builder-nodes">
                {stage.nodes.map((t, ni) => {
                  const hit = findModel(models, t);
                  return (
                    <span key={ni} className="orch-chip builder-chip">
                      <span
                        className="graph-node-dot"
                        style={{ background: vendorColor(t.agent, hit?.vendor) }}
                      />
                      <button
                        className="builder-chip-edit"
                        onClick={() => setEditing({ si, ni })}
                        title="编辑节点"
                      >
                        {targetLabel(t)} · {t.role}
                      </button>
                      <button
                        className="builder-chip-del"
                        onClick={() => removeNode(si, ni)}
                        title="删除节点"
                      >
                        ×
                      </button>
                    </span>
                  );
                })}
              </div>
              {editing?.si === si && stage.nodes[editing.ni] && (
                <div className="builder-target-editor">
                  <div className="builder-target-editor-head">
                    <span>节点 {editing.ni + 1}</span>
                    <button className="graph-mini-btn" onClick={() => setEditing(null)}>
                      收起
                    </button>
                  </div>
                  <TargetEditor
                    key={`${editing.si}-${editing.ni}`}
                    target={stage.nodes[editing.ni]}
                    models={models}
                    onChange={(t) => patchTarget(si, editing.ni, t)}
                  />
                </div>
              )}
            </div>
          ))}
          <button className="graph-mini-btn builder-add-stage" onClick={addStage}>
            新建阶段
          </button>
        </div>

        <div className="builder-notation">
          <label className="builder-notation-label">一行记法</label>
          <input
            className={`builder-notation-input${notationError ? ' has-error' : ''}`}
            value={notation}
            onChange={(e) => setNotation(e.target.value)}
            onBlur={onNotationBlur}
            spellCheck={false}
          />
          {notationError && (
            <div className="builder-notation-error">
              第 {notationError.pos + 1} 个字符：{notationError.message}
            </div>
          )}
          {!notationError && lossy.length > 0 && (
            <div className="builder-notation-warn">一行记法不表达：{lossy.join('、')}</div>
          )}
          {!notationError && unresolved.length > 0 && (
            <div className="builder-notation-warn">未接入：{unresolved.join('、')}</div>
          )}
        </div>

        {saveAsk && (
          <div className="builder-save-row">
            <input
              value={saveName}
              placeholder="预设名"
              autoFocus
              onChange={(e) => setSaveName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.nativeEvent.isComposing) void doSave();
              }}
            />
            <button className="orch-btn orch-btn-primary" disabled={saveBusy || saveName.trim() === ''} onClick={() => void doSave()}>
              确定
            </button>
            <button className="orch-btn" onClick={() => setSaveAsk(false)}>
              取消
            </button>
          </div>
        )}
        {saveError && <div className="builder-notation-error">保存失败：{saveError}</div>}

        <div className="builder-actions">
          <button className="orch-btn" onClick={() => setSaveAsk((v) => !v)}>
            保存为预设
          </button>
          <button
            className="orch-btn orch-btn-primary"
            disabled={!canApply}
            title={canApply ? undefined : '先在图上选一个节点'}
            onClick={() =>
              onApply({
                id: initial?.id ?? '',
                name: initial?.name ?? '自定义模式',
                builtin: false,
                stages: cloneStages(stages),
              })
            }
          >
            应用
          </button>
          <button className="orch-btn" onClick={onClose}>
            关闭
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
