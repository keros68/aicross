// SettingsDialog.tsx — 顶栏「设置」打开的派发设置对话框（TASK-89）：读 dispatch_settings，
// 一页改完并发上限、超时（按分钟）、修正轮、脱敏、验证命令与 ACP。改动收进草稿，
// 点「保存」一次写入（dispatch_save，与 aicross config set 同一实现体；开启 ACP 时
// 安装结果带回显示，设置已写入不回滚）。底部注明设置文件路径。

import { useEffect, useRef, useState } from 'react';
import { commands, isTauri, type DispatchSettingsView } from '../tauri';
import {
  SETTINGS_SECTIONS,
  acpStatusText,
  changedCount,
  initialDraft,
  numberInputError,
  settingsPatch,
  type SettingsDraft,
  type SettingsField,
} from './settings-view';
import { Icon } from '../components/common/Icon';
import './ConfigPages.css';

type Load =
  | { state: 'loading' }
  | { state: 'error'; message: string }
  | { state: 'ready'; view: DispatchSettingsView; draft: SettingsDraft };

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const [load, setLoad] = useState<Load>({ state: 'loading' });
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState('');
  // 保存返回的说明（夹取、ACP 安装结果）；有关闭按钮，不拦着继续改。
  const [note, setNote] = useState('');
  const cancelRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    cancelRef.current?.focus();
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

  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    commands.dispatchSettings()
      .then((view) => {
        if (!cancelled) setLoad({ state: 'ready', view, draft: initialDraft(view.dispatch) });
      })
      .catch((err) => {
        if (!cancelled) setLoad({ state: 'error', message: String(err) });
      });
    return () => { cancelled = true; };
  }, []);

  const setDraftValue = (key: string, value: string | number | boolean) => {
    setLoad((current) => (current.state === 'ready'
      ? { ...current, draft: { ...current.draft, [key]: value } }
      : current));
  };

  const settings = load.state === 'ready' ? load.view.dispatch : null;
  const patch = settings !== null && load.state === 'ready'
    ? settingsPatch(load.draft, settings)
    : null;
  const changes = changedCount(patch);
  const invalid = patch === null;

  const save = async () => {
    if (patch === null || changes === 0 || settings === null) return;
    setSaving(true);
    setSaveError('');
    setNote('');
    try {
      const result = await commands.dispatchSave(patch);
      // 写入后的设置重读一遍（可能被夹取），带说明时留着让用户看到结果。
      const view = await commands.dispatchSettings();
      setLoad({ state: 'ready', view, draft: initialDraft(view.dispatch) });
      if (result.note) setNote(result.note);
      else onClose();
    } catch (err) {
      setSaveError(String(err));
    }
    setSaving(false);
  };

  return (
    <div className="dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div className="dialog settings-dialog" role="dialog" aria-modal="true" aria-label="派发设置">
        <div className="dialog-title">派发设置</div>
        <div className="dialog-body settings-body">
          {load.state === 'loading' && <p className="cfg-empty">读取中…</p>}
          {load.state === 'error' && <p className="cfg-error">{load.message}</p>}
          {load.state === 'ready' && (
            <>
              {SETTINGS_SECTIONS.map((section) => (
                <div key={section.title} className="settings-section">
                  <div className="settings-section-title">{section.title}</div>
                  {section.fields.map((field) => (
                    <SettingsRow
                      key={field.key}
                      field={field}
                      draft={load.draft}
                      disabled={saving}
                      onChange={setDraftValue}
                    />
                  ))}
                  {section.fields.some((field) => field.key === 'acpEnabled') && acpStatusText(load.view.acp) !== '' && (
                    <p className="cfg-dim settings-acp">{acpStatusText(load.view.acp)}</p>
                  )}
                </div>
              ))}
              <p className="cfg-dim settings-path" title={load.view.path}>设置文件 {load.view.path}</p>
            </>
          )}
        </div>
        {note !== '' && (
          <div className="settings-note">
            <span className="settings-note-text">{note}</span>
            <button
              type="button"
              className="icon-btn"
              title="关闭"
              aria-label="关闭"
              onClick={() => setNote('')}
            >
              <Icon name="close" />
            </button>
          </div>
        )}
        {saveError !== '' && <p className="cfg-error settings-save-error">{saveError}</p>}
        <div className="dialog-actions">
          <button type="button" className="btn" ref={cancelRef} disabled={saving} onClick={onClose}>取消</button>
          <button
            type="button"
            className="btn primary"
            disabled={saving || invalid || changes === 0}
            onClick={() => void save()}
          >
            {saving ? '保存中…' : '保存'}
          </button>
        </div>
      </div>
    </div>
  );
}

/** 一行设置：左边名称与说明，右边控件。数值键的校验说明显示在行下。 */
function SettingsRow({
  field,
  draft,
  disabled,
  onChange,
}: {
  field: SettingsField;
  draft: SettingsDraft;
  disabled: boolean;
  onChange: (key: string, value: string | number | boolean) => void;
}) {
  const label = (
    <span className="settings-label">
      <span className="settings-name">{field.label}</span>
      <span className="card-sub">{field.hint}</span>
    </span>
  );
  if (field.kind === 'segmented') {
    const current = draft[field.key];
    return (
      <div className="settings-row">
        {label}
        <div className="segmented" role="radiogroup" aria-label={field.label}>
          {field.choices.map((choice) => (
            <button
              key={choice}
              type="button"
              role="radio"
              aria-checked={current === choice}
              className={`segmented-item${current === choice ? ' selected' : ''}`}
              disabled={disabled}
              onClick={() => onChange(field.key, choice)}
            >
              {choice}
            </button>
          ))}
        </div>
      </div>
    );
  }
  if (field.kind === 'number') {
    const raw = typeof draft[field.key] === 'string' ? String(draft[field.key]) : String(draft[field.key]);
    const error = numberInputError(field, raw);
    return (
      <div className="settings-row">
        {label}
        <span className="settings-number-wrap">
          <input
            className="settings-number"
            inputMode="decimal"
            aria-label={field.label}
            aria-invalid={error !== ''}
            value={raw}
            disabled={disabled}
            onChange={(event) => onChange(field.key, event.target.value)}
          />
          <span className="cfg-dim">{field.unit}</span>
        </span>
        {error !== '' && <p className="cfg-error settings-field-error">{error}</p>}
      </div>
    );
  }
  if (field.kind === 'bool') {
    return (
      <div className="settings-row">
        {label}
        <input
          type="checkbox"
          className="settings-check"
          aria-label={field.label}
          checked={draft[field.key] === true}
          disabled={disabled}
          onChange={(event) => onChange(field.key, event.target.checked)}
        />
      </div>
    );
  }
  return (
    <div className="settings-row settings-row-text">
      {label}
      <input
        className="path-input settings-text"
        aria-label={field.label}
        value={typeof draft[field.key] === 'string' ? String(draft[field.key]) : ''}
        disabled={disabled}
        onChange={(event) => onChange(field.key, event.target.value)}
      />
    </div>
  );
}
