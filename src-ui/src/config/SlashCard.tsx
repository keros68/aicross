// SlashCard.tsx — skill 页的「斜杠命令」卡片（TASK-53；TASK-54 改为按模式列出）：
// 第一行是当前主 agent 下调用 ai-cross 的默认写法；下面每个内置模式一行：模式、带参数
// 写法、短命令（有则显示触发写法、状态与编辑 / 删除，无则显示建议名与「生成」）；不属于
// 模式行的命令列在「其它」小节。卡片头「全部生成」（slash_add_all，前缀 ax-）与「添加」
// （自定义模式或自定义名称）。数据取 slash_list，添加、编辑、生成都调 slash_add（同名
// 即更新），删除调 slash_remove。文件内容旧了的命令标「待更新」并给「更新」；有任一条时
// 卡片头加「全部更新」（TASK-60，名称、模式、说明沿用原值）。

import { useCallback, useEffect, useRef, useState } from 'react';
import { commands, isTauri, type HostEntry, type SlashCommand, type SlashList } from '../tauri';
import { MODE_ICONS, modeKind } from './mode-kind';
import {
  SLASH_DEFAULT_PREFIX,
  SLASH_INVALID_STATE,
  SLASH_NOTE_MAX,
  canAddAll,
  normalizeSlashNote,
  otherSlashCommands,
  slashModeRows,
  staleCommands,
  slashNameError,
  slashNameValid,
  slashStateTone,
  triggerPrefix,
  triggerPreview,
} from './slash-view';
import { copyText } from '../lib/clipboard';
import { slashRowAction } from './newuser-view';
import { ConfirmDialog } from '../components/common/ConfirmDialog';
import { Icon } from '../components/common/Icon';

export interface ModeChoice {
  id: string;
  name: string;
}

interface Draft {
  /** 编辑时为原名称，名称不可改；添加时为空串。 */
  editing: string;
  name: string;
  mode: string;
  note: string;
}

function ModeMark({ id, name, description }: ModeChoice & { description?: string }) {
  const kind = modeKind(id, name);
  const tip = (description ?? '').trim();
  return (
    <span className={`slash-mode tone-${kind}`} title={tip === '' ? undefined : tip}>
      <Icon name={MODE_ICONS[kind]} size={14} />
      <span className="slash-mode-name">{name}</span>
    </span>
  );
}

/** 复制按钮：复制后短暂显示「已复制」，失败时短暂显示「复制失败」。 */
function CopyButton({ text, icon }: { text: string; icon?: boolean }) {
  const [result, setResult] = useState<'' | 'done' | 'fail'>('');
  // 结果提示的定时器：再次点击先清掉上一次的，卸载时清理。
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const copy = async () => {
    window.clearTimeout(timer.current);
    setResult((await copyText(text)) ? 'done' : 'fail');
    timer.current = window.setTimeout(() => setResult(''), 1200);
  };
  const label = result === 'done' ? '已复制' : result === 'fail' ? '复制失败' : '复制';
  if (icon) {
    return (
      <button
        type="button"
        className={`icon-btn${result === 'fail' ? ' copy-fail' : ''}`}
        title={label}
        aria-label={label}
        onClick={() => void copy()}
      >
        {result === 'fail' ? '复制失败' : <Icon name={result === 'done' ? 'check' : 'copy'} />}
      </button>
    );
  }
  return (
    <button type="button" className={`btn small${result === 'fail' ? ' copy-fail' : ''}`} onClick={() => void copy()}>
      {label}
    </button>
  );
}

export function SlashCard({
  host,
  modes,
  skillInstalled,
  active,
  refreshKey,
}: {
  host: HostEntry;
  /** 当前主 agent 的模式（id 与名称），供下拉选择。 */
  modes: ModeChoice[];
  /** skill 状态已读到且没装时为 false；没读到时传 true，不提示。 */
  skillInstalled: boolean;
  active: boolean;
  refreshKey: number;
}) {
  const [list, setList] = useState<SlashList | null>(null);
  const [loadError, setLoadError] = useState('');
  const [draft, setDraft] = useState<Draft | null>(null);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState('');
  const [rowError, setRowError] = useState('');
  const [removing, setRemoving] = useState<SlashCommand | null>(null);
  const [busyName, setBusyName] = useState('');
  // 生成进行中：模式 id，或「全部生成」时为 '*'。
  const [generating, setGenerating] = useState('');
  const [skipped, setSkipped] = useState<{ name: string; reason: string }[]>([]);
  const closeRemove = useCallback(() => setRemoving(null), []);

  useEffect(() => {
    if (!isTauri || !active) return;
    let cancelled = false;
    commands.slashList(host.id)
      .then((next) => {
        // 返回的不是当前主 agent（切换时的旧请求）就丢弃。
        if (cancelled || next.host !== host.id) return;
        setList(next);
        setLoadError('');
      })
      .catch((err) => { if (!cancelled) setLoadError(String(err)); });
    return () => { cancelled = true; };
  }, [host.id, active, refreshKey]);

  const startAdd = () => {
    setFormError('');
    setDraft({ editing: '', name: '', mode: modes[0]?.id ?? '', note: '' });
  };

  const startEdit = (command: SlashCommand) => {
    setFormError('');
    // 模式已被删除时不预选，模式下拉显示「未选择」，选好才能保存。
    const mode = modes.some((item) => item.id === command.mode) ? command.mode : '';
    setDraft({ editing: command.name, name: command.name, mode, note: command.note });
  };

  const save = async () => {
    if (!draft || !slashNameValid(draft.name) || draft.mode === '') return;
    setSaving(true);
    setFormError('');
    try {
      const note = normalizeSlashNote(draft.note);
      const next = await commands.slashAdd(host.id, draft.name, draft.mode, note === '' ? null : note);
      if (next.host === host.id) setList(next);
      setDraft(null);
    } catch (err) {
      setFormError(String(err));
    } finally {
      setSaving(false);
    }
  };

  const remove = async (command: SlashCommand) => {
    setRemoving(null);
    setBusyName(command.name);
    setRowError('');
    try {
      const next = await commands.slashRemove(host.id, command.name);
      if (next.host === host.id) setList(next);
      // 表单正在编辑这一条时一并关闭。
      setDraft((current) => (current && current.editing === command.name ? null : current));
    } catch (err) {
      setRowError(String(err));
    } finally {
      setBusyName('');
    }
  };

  /** 给一个模式生成建议名的命令。 */
  const generate = async (mode: string, name: string) => {
    setGenerating(mode);
    setRowError('');
    try {
      const next = await commands.slashAdd(host.id, name, mode, null);
      if (next.host === host.id) setList(next);
    } catch (err) {
      setRowError(String(err));
    } finally {
      setGenerating('');
    }
  };

  const generateAll = async () => {
    setGenerating('*');
    setRowError('');
    setSkipped([]);
    try {
      const next = await commands.slashAddAll(host.id, SLASH_DEFAULT_PREFIX);
      if (next.host === host.id) setList(next);
      setSkipped(next.skipped ?? []);
    } catch (err) {
      setRowError(String(err));
    } finally {
      setGenerating('');
    }
  };

  /** 用原名称、模式与说明重写命令文件。 */
  const rewrite = (command: SlashCommand) => {
    const note = normalizeSlashNote(command.note);
    return commands.slashAdd(host.id, command.name, command.mode, note === '' ? null : note);
  };

  const update = async (command: SlashCommand) => {
    setGenerating(`update:${command.name}`);
    setRowError('');
    try {
      const next = await rewrite(command);
      if (next.host === host.id) setList(next);
    } catch (err) {
      setRowError(String(err));
    } finally {
      setGenerating('');
    }
  };

  /** 逐条更新全部待更新的命令；单条失败不中断，最后一条的错误显示在列表下。 */
  const updateAll = async () => {
    const targets = staleCommands(list);
    if (targets.length === 0) return;
    setGenerating('update:*');
    setRowError('');
    let latest: SlashList | null = null;
    for (const command of targets) {
      try {
        latest = await rewrite(command);
      } catch (err) {
        setRowError(String(err));
      }
    }
    if (latest && latest.host === host.id) setList(latest);
    setGenerating('');
  };

  const busy = busyName !== '' || generating !== '' || saving;
  const stale = staleCommands(list);

  const commandControls = (command: SlashCommand) => {
    const tone = slashStateTone(command.state);
    const invalidName = command.state === SLASH_INVALID_STATE;
    return (
      <>
        <span className={`pill slash-state ${tone}`}>{command.state}</span>
        {command.stale === true && !invalidName && (
          <>
            <span className="pill warn">待更新</span>
            <button
              type="button"
              className="btn small"
              disabled={busy || draft !== null}
              onClick={() => void update(command)}
            >
              {generating === `update:${command.name}` ? '更新中…' : '更新'}
            </button>
          </>
        )}
        <span className="slash-actions">
          {!invalidName && <CopyButton text={command.trigger} icon />}
          {!invalidName && (
            <button
              type="button"
              className="icon-btn"
              title="编辑"
              aria-label="编辑"
              disabled={draft !== null || busy}
              onClick={() => startEdit(command)}
            >
              <Icon name="edit" />
            </button>
          )}
          <button
            type="button"
            className="icon-btn"
            title="删除"
            aria-label="删除"
            disabled={draft !== null || busy}
            onClick={() => setRemoving(command)}
          >
            {busyName === command.name ? '删除中…' : <Icon name="trash" />}
          </button>
        </span>
      </>
    );
  };

  const modeRows = slashModeRows(list);
  const others = otherSlashCommands(list);

  const nameError = draft !== null && draft.name !== '' ? slashNameError(draft.name) : '';
  const nameInvalid = nameError !== '';
  const draftMode = modes.find((mode) => mode.id === draft?.mode);

  return (
    <section className="card slash-card">
      <div className="card-row card-head">
        <span className="card-title">斜杠命令</span>
        <span className="cfg-spacer" />
        {modeRows.length > 0 && (
          <button type="button" className="btn small" disabled={busy || draft !== null || !canAddAll(list)} onClick={() => void generateAll()}>
            {generating === '*' ? '生成中…' : '全部生成'}
          </button>
        )}
        {stale.length > 0 && (
          <button type="button" className="btn small" disabled={busy || draft !== null} onClick={() => void updateAll()}>
            {generating === 'update:*' ? '更新中…' : '全部更新'}
          </button>
        )}
        <button type="button" className="btn small" disabled={draft !== null || modes.length === 0} onClick={startAdd}>
          <Icon name="plus" size={14} />
          添加
        </button>
      </div>
      {!skillInstalled && (
        <div className="card-row slash-warn">
          <Icon name="alert" />
          <span>{host.name || host.id} 未安装 ai-cross skill</span>
        </div>
      )}
      {skipped.length > 0 && (
        <div className="card-row slash-warn slash-skipped">
          <Icon name="alert" />
          <div className="slash-skipped-list">
            {skipped.map((item) => <div key={item.name}><span className="cfg-mono">{item.name}</span>：{item.reason}</div>)}
          </div>
          <button type="button" className="icon-btn" title="关闭" aria-label="关闭" onClick={() => setSkipped([])}>
            <Icon name="close" />
          </button>
        </div>
      )}
      {loadError !== '' && <div className="card-row"><span className="cfg-error">{loadError}</span></div>}
      {list && (
        <div className="card-row slash-row">
          <span className="slash-label">默认</span>
          <span className="cfg-mono slash-trigger">{list.invoke}</span>
          <span className="cfg-spacer" />
          <CopyButton text={list.invoke} />
        </div>
      )}
      {modeRows.map((row) => (
        <div key={row.mode} className="card-row slash-row slash-mode-row">
          <span className="slash-mode-cell">
            <ModeMark id={row.mode} name={row.modeName || row.mode} description={row.description} />
          </span>
          <span className="slash-arg">
            <span className="cfg-mono slash-trigger" title={row.argForm}>{row.argForm}</span>
            <CopyButton text={row.argForm} icon />
          </span>
          <span className="cfg-spacer" />
          {(() => {
            const action = slashRowAction(row);
            if (row.command) {
              return (
                <>
                  <span className="cfg-mono slash-trigger" title={row.command.path}>{row.command.trigger}</span>
                  {commandControls(row.command)}
                </>
              );
            }
            if (action.kind === 'occupied') {
              return (
                <span className="pill alert" title={row.existing?.path}>文件已被占用</span>
              );
            }
            if (action.kind === 'register') {
              // 命令文件已存在（我们写的、但这个数据目录还没登记）：登记后沿用这个文件。
              return (
                <>
                  <span className="cfg-mono slash-trigger" title={row.existing?.path}>{triggerPreview(host.id, action.name)}</span>
                  <span className="pill">已存在</span>
                  <button
                    type="button"
                    className="btn small"
                    disabled={busy || draft !== null}
                    onClick={() => void generate(row.mode, action.name)}
                  >
                    {generating === row.mode ? '登记中…' : '登记'}
                  </button>
                </>
              );
            }
            return (
              <>
                <span className="cfg-mono slash-trigger slash-suggested">{row.suggestedTrigger}</span>
                <button
                  type="button"
                  className="btn small"
                  disabled={busy || draft !== null}
                  onClick={() => void generate(row.mode, row.suggestedName)}
                >
                  {generating === row.mode ? '生成中…' : '生成'}
                </button>
              </>
            );
          })()}
        </div>
      ))}
      {others.length > 0 && modeRows.length > 0 && <div className="card-row card-group-title">其它</div>}
      {others.map((command) => (
        <div key={command.name} className="card-row slash-row">
          <span className="cfg-mono slash-trigger" title={command.path}>{command.trigger}</span>
          <ModeMark id={command.mode} name={command.modeName || command.mode} />
          <span className="slash-note" title={command.note}>{command.note}</span>
          {commandControls(command)}
        </div>
      ))}
      {rowError !== '' && <div className="card-row"><span className="cfg-error">{rowError}</span></div>}
      {draft && (
        <div className="card-row slash-form">
          <div className="slash-form-fields">
            <label className="slash-name">
              <span className="cfg-mono slash-prefix">{triggerPrefix(host.id)}</span>
              <input
                className="cfg-mono"
                value={draft.name}
                placeholder="名称"
                maxLength={32}
                autoFocus={draft.editing === ''}
                disabled={draft.editing !== '' || saving}
                aria-invalid={nameInvalid}
                onChange={(event) => setDraft({ ...draft, name: event.target.value.trim() })}
              />
            </label>
            <span className="slash-mode-select">
              {draftMode && <Icon name={MODE_ICONS[modeKind(draftMode.id, draftMode.name)]} size={16} />}
              <select
                className="pill-select"
                aria-label="模式"
                value={draft.mode}
                disabled={saving}
                onChange={(event) => setDraft({ ...draft, mode: event.target.value })}
              >
                {draft.mode === '' && <option value="" disabled>未选择</option>}
                {modes.map((mode) => <option key={mode.id} value={mode.id}>{mode.name}</option>)}
              </select>
            </span>
            <input
              className="slash-note-input"
              value={draft.note}
              placeholder="附加说明"
              maxLength={SLASH_NOTE_MAX}
              disabled={saving}
              onChange={(event) => setDraft({ ...draft, note: event.target.value })}
            />
            <button
              type="button"
              className="btn primary small"
              disabled={saving || !slashNameValid(draft.name) || draft.mode === ''}
              onClick={() => void save()}
            >
              {saving ? '保存中…' : '保存'}
            </button>
            <button type="button" className="btn small" disabled={saving} onClick={() => setDraft(null)}>取消</button>
          </div>
          {nameInvalid && <span className="cfg-error">{nameError}</span>}
          {formError !== '' && <span className="cfg-error">{formError}</span>}
        </div>
      )}
      {removing && (
        <ConfirmDialog title={`删除 ${removing.trigger}`} confirmLabel="删除" onConfirm={() => void remove(removing)} onCancel={closeRemove} />
      )}
    </section>
  );
}
