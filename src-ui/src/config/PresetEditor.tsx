// PresetEditor.tsx — 自定义模式编辑器（TASK-55，应用内对话框）：名称、介绍，阶段列表
// （阶段之间「→」），每个位置一行：角色、可见性、写权限、思考、删除；可添加位置与阶段。
// 前端先按与引擎一致的规则校验并就地显示（名称在输入后或点保存时才校验），保存经模式页调 preset_save，
// 引擎的错误显示在底部。每个阶段的位置上方有列名；新阶段默认「验证 · 盲」（TASK-58）。
// Esc 或点背景关闭（保存中不关）。

import { useEffect, useState } from 'react';
import type { ModeEntry, PresetPayload } from '../tauri';
import {
  PRESET_NAME_MAX,
  PRESET_THINKING_OPTIONS,
  ROLE_OPTIONS,
  VISIBILITY_OPTIONS,
  WRITE_OPTIONS,
  hasErrors,
  newNode,
  newStageNode,
  toPresetPayload,
  validatePreset,
  type DraftNode,
  type PresetDraft,
} from './preset-edit';
import { Icon } from '../components/common/Icon';

export function PresetEditor({
  initial,
  others,
  onSave,
  onClose,
}: {
  initial: PresetDraft;
  /** 用来查重名的全部模式（含内置）。 */
  others: Pick<ModeEntry, 'id' | 'name'>[];
  /** 保存（由模式页经写入协调发出，带当前主 agent）；失败时抛出 stderr 那行说明。 */
  onSave: (preset: PresetPayload) => Promise<void>;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState<PresetDraft>(initial);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  // 新建时名称还没填过就不提示「名称为空」。
  const [nameTouched, setNameTouched] = useState(initial.name !== '');
  const errors = validatePreset(draft, others);
  const invalid = hasErrors(errors);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.isComposing && !saving) {
        event.preventDefault();
        event.stopPropagation();
        onClose();
      }
    };
    document.addEventListener('keydown', onKey, true);
    return () => document.removeEventListener('keydown', onKey, true);
  }, [onClose, saving]);

  const setNode = (stageIndex: number, nodeIndex: number, patch: Partial<DraftNode>) => {
    setDraft((current) => ({
      ...current,
      stages: current.stages.map((stage, s) => (s !== stageIndex ? stage : {
        nodes: stage.nodes.map((node, n) => (n === nodeIndex ? { ...node, ...patch } : node)),
      })),
    }));
  };

  const removeNode = (stageIndex: number, nodeIndex: number) => {
    setDraft((current) => ({
      ...current,
      stages: current.stages.map((stage, s) => (s !== stageIndex ? stage : {
        nodes: stage.nodes.filter((_, n) => n !== nodeIndex),
      })),
    }));
  };

  const addNode = (stageIndex: number) => {
    setDraft((current) => ({
      ...current,
      stages: current.stages.map((stage, s) => (s === stageIndex ? { nodes: [...stage.nodes, newNode()] } : stage)),
    }));
  };

  const removeStage = (stageIndex: number) => {
    setDraft((current) => ({ ...current, stages: current.stages.filter((_, s) => s !== stageIndex) }));
  };

  const addStage = () => {
    setDraft((current) => ({ ...current, stages: [...current.stages, { nodes: [newStageNode()] }] }));
  };

  // 保存时才对没填过的名称报错（TASK-58）。
  const save = async () => {
    setNameTouched(true);
    if (invalid) return;
    setSaving(true);
    setError('');
    try {
      await onSave(toPresetPayload(draft));
    } catch (err) {
      setError(String(err));
      setSaving(false);
    }
  };

  return (
    <div className="dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget && !saving) onClose(); }}>
      <div className="dialog preset-editor" role="dialog" aria-modal="true" aria-label={draft.id === '' ? '新建模式' : '编辑模式'}>
        <div className="dialog-title">{draft.id === '' ? '新建模式' : '编辑模式'}</div>
        <div className="preset-fields">
          <label className="preset-field">
            <span>名称</span>
            <input
              value={draft.name}
              maxLength={PRESET_NAME_MAX + 10}
              autoFocus
              aria-invalid={nameTouched && errors.name !== ''}
              onChange={(event) => {
                setNameTouched(true);
                setDraft({ ...draft, name: event.target.value });
              }}
            />
          </label>
          {errors.name !== '' && nameTouched && <span className="cfg-error preset-field-error">{errors.name}</span>}
          <label className="preset-field">
            <span>介绍</span>
            <input value={draft.description} onChange={(event) => setDraft({ ...draft, description: event.target.value })} />
          </label>
        </div>
        <div className="preset-stages">
          {draft.stages.map((stage, stageIndex) => (
            <div key={stageIndex} className="preset-stage-wrap">
              {stageIndex > 0 && <div className="preset-arrow" aria-hidden="true">→</div>}
              <section className="preset-stage">
                <div className="preset-stage-head">
                  <span className="seat-row-stage">{stageIndex + 1}</span>
                  <span>第 {stageIndex + 1} 阶段</span>
                  <span className="cfg-spacer" />
                  <button
                    type="button"
                    className="icon-btn"
                    title="删除阶段"
                    aria-label="删除阶段"
                    disabled={draft.stages.length <= 1}
                    onClick={() => removeStage(stageIndex)}
                  >
                    <Icon name="trash" />
                  </button>
                </div>
                {stage.nodes.length > 0 && (
                  <div className="preset-node preset-node-head" aria-hidden="true">
                    <span>角色</span>
                    <span>可见性</span>
                    <span>写权限</span>
                    <span>思考</span>
                    <span />
                  </div>
                )}
                {stage.nodes.map((node, nodeIndex) => (
                  <div key={nodeIndex} className="preset-node">
                    <select className="pill-select" aria-label="角色" value={node.role} onChange={(event) => setNode(stageIndex, nodeIndex, { role: event.target.value })}>
                      {ROLE_OPTIONS.map((role) => <option key={role} value={role}>{role}</option>)}
                    </select>
                    <select className="pill-select" aria-label="可见性" value={node.visibility} onChange={(event) => setNode(stageIndex, nodeIndex, { visibility: event.target.value })}>
                      {VISIBILITY_OPTIONS.map((value) => <option key={value} value={value}>{value}</option>)}
                    </select>
                    <select className="pill-select" aria-label="写权限" value={node.write} onChange={(event) => setNode(stageIndex, nodeIndex, { write: event.target.value })}>
                      {WRITE_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
                    </select>
                    <select className="pill-select" aria-label="思考" value={node.thinking} onChange={(event) => setNode(stageIndex, nodeIndex, { thinking: event.target.value })}>
                      {PRESET_THINKING_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>{`思考 · ${option.label}`}</option>
                      ))}
                    </select>
                    <button
                      type="button"
                      className="icon-btn"
                      title="删除位置"
                      aria-label="删除位置"
                      onClick={() => removeNode(stageIndex, nodeIndex)}
                    >
                      <Icon name="close" />
                    </button>
                  </div>
                ))}
                {errors.stages[stageIndex] && <span className="cfg-error">{errors.stages[stageIndex]}</span>}
                <button type="button" className="btn small preset-add" onClick={() => addNode(stageIndex)}>
                  <Icon name="plus" size={14} />
                  添加位置
                </button>
              </section>
            </div>
          ))}
          <button type="button" className="btn small preset-add" onClick={addStage}>
            <Icon name="plus" size={14} />
            添加阶段
          </button>
        </div>
        {errors.total !== '' && <p className="cfg-error">{errors.total}</p>}
        {error !== '' && <p className="cfg-error">{error}</p>}
        <div className="dialog-actions">
          <button type="button" className="btn" disabled={saving} onClick={onClose}>取消</button>
          <button
            type="button"
            className="btn primary"
            disabled={saving || (nameTouched && invalid)}
            onClick={() => void save()}
          >
            {saving ? '保存中…' : '保存'}
          </button>
        </div>
      </div>
    </div>
  );
}
