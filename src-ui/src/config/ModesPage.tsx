// ModesPage.tsx — 「模式」页（TASK-47；TASK-49、TASK-51 多轮改版，fix-51-r1 改为 magpie
// 的 Agents 页样式）：工具行左边是主 agent 的实际厂商，右边「一键推荐」与「更多 →
// 全部按推荐重填」（需确认）；页顶数字条；模式分常用 / 更多模式 / 自定义三组，每个
// 模式一张卡片，卡片头是模式图标、名称与默认标记，下面每个位置一行：「阶段号 · 角色」
// 加模型选择下拉、思考下拉与备选按钮，选完即保存（config_set_seat，Rust 侧记为
// 手选）。首次使用按 资源 → skill → 一键推荐 的顺序在顶部给一条提示。卡片头下有
// 模式介绍；可新建模式、把内置模式复制为新模式、编辑与删除自定义模式（TASK-55）；
// 可调数量的内置模式在卡片头有「<角色> × <数值>」与 −/+（TASK-56，减少会清人选时先确认）。
// TASK-58：位置行带评审视角；「未选择」用中性虚线框；推荐后列出各 agent 未验证的模型数与
// 「去测试」；「一键推荐」同一时间只出现一处（未配置时在提示条，其余在工具行）。
// TASK-60：评审模式位置的视角是下拉（可选视角 / 默认 / 自定义…），改动调 config_set_focus。数据按当前主
// agent 取（config_snapshot(host)），写入命令都带 host。

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  commands,
  isTauri,
  type ConfigSnapshot,
  type HostEntry,
  type ModeEntry,
  type ModeSeatNode,
  type SkillStatus,
} from '../tauri';
import {
  THINKING_OPTIONS,
  flowColumns,
  modesOverview,
  optionsWithCurrent,
  seatDraft,
  seatFromDraft,
  seatOptions,
  seatRowState,
  type SeatDraft,
  type SeatOption,
} from './config-view';
import { MODE_ICONS, groupModes, modeKind } from './mode-kind';
import { findHostStatus, modesPrompt, recommendNotes } from './skill-view';
import { ModelPicker } from './ModelPicker';
import { PresetEditor } from './PresetEditor';
import { seatRoleText, unverifiedLines } from './newuser-view';
import { FOCUS_CUSTOM, FOCUS_MAX, focusChoices, focusError, focusIndex } from './focus-view';
import { canDecrease, canIncrease, decreaseConfirmText, seatsLostOnDecrease } from './mode-count';
import {
  draftFromCopy,
  draftFromMode,
  emptyDraft,
  impactLines,
  removeImpact,
  type PresetDraft,
} from './preset-edit';
import { ConfirmDialog } from '../components/common/ConfirmDialog';
import { HostBar } from '../components/common/HostBar';
import { Icon } from '../components/common/Icon';
import { PageHeader } from '../components/common/PageHeader';
import { StatCards } from '../components/common/StatCards';
import { useDismiss } from '../components/common/use-dismiss';
import './ConfigPages.css';

const CLEAR_KEY = '\u0000clear';

/** 评审视角：下拉选可选视角或「默认」，「自定义…」进入行内输入框（回车保存、Esc 取消）。 */
function FocusSelect({
  options,
  current,
  disabled,
  onSave,
}: {
  options: string[];
  current: string;
  disabled: boolean;
  onSave: (focus: string) => void;
}) {
  const [custom, setCustom] = useState<string | null>(null);
  if (custom !== null) {
    const error = focusError(custom);
    const submit = () => {
      if (error !== '') return;
      onSave(custom.trim());
      setCustom(null);
    };
    return (
      <span className="focus-custom">
        <input
          className="focus-input"
          value={custom}
          maxLength={FOCUS_MAX + 10}
          autoFocus
          aria-label="自定义视角"
          aria-invalid={custom !== '' && error !== ''}
          onChange={(event) => setCustom(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
              event.preventDefault();
              submit();
            } else if (event.key === 'Escape') {
              event.preventDefault();
              event.stopPropagation();
              setCustom(null);
            }
          }}
          onBlur={() => setCustom(null)}
        />
        {custom !== '' && error !== '' && <span className="cfg-error">{error}</span>}
      </span>
    );
  }
  return (
    <select
      className="pill-select focus-select"
      aria-label="视角"
      value={current}
      disabled={disabled}
      onChange={(event) => {
        const value = event.target.value;
        if (value === FOCUS_CUSTOM) setCustom('');
        else onSave(value);
      }}
    >
      {focusChoices(options, current).map((choice) => (
        <option key={choice.value} value={choice.value}>{choice.label}</option>
      ))}
    </select>
  );
}

/** 一个位置一行：左边「阶段号 · 角色」，右边模型选择、思考、备选，选完即保存。 */
function SeatRow({
  host,
  mode,
  stage,
  index,
  node,
  options,
  locked,
  submit,
  onBusyChange,
}: {
  host: string;
  mode: ModeEntry;
  stage: number;
  index: number;
  node: ModeSeatNode;
  options: SeatOption[];
  /** 其他位置在保存或一键推荐进行中。 */
  locked: boolean;
  /** 发出保存或清除并把结果并入页面快照；失败时抛出 stderr 那行说明。 */
  submit: (action: () => Promise<ModeEntry>) => Promise<void>;
  onBusyChange: (busy: boolean) => void;
}) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  // 保存成功后快照更新，本行可能随之重建；await 返回后先看是否还挂着。
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const draft = seatDraft(node);
  const { missing, invalid, recommended, presetFixed } = seatRowState(node);

  const run = async (action: () => Promise<ModeEntry>) => {
    setSaving(true);
    setError('');
    onBusyChange(true);
    try {
      await submit(action);
    } catch (err) {
      if (mounted.current) setError(String(err));
    } finally {
      if (mounted.current) setSaving(false);
      onBusyChange(false);
    }
  };

  const save = (next: SeatDraft) => {
    const seat = seatFromDraft(next);
    if (seat) void run(() => commands.configSetSeat(host, mode.id, stage, index, seat));
  };
  const clear = () => void run(() => commands.configClearSeat(host, mode.id, stage, index));
  const focusOptions = node.focusOptions ?? [];
  const saveFocus = (focus: string) => {
    const position = focusIndex(mode.stages, stage, index);
    if (position > 0) void run(() => commands.configSetFocus(host, mode.id, position, focus));
  };

  const disabled = saving || locked || presetFixed;
  const personOptions = optionsWithCurrent(options, draft.personKey);
  const fallbackOptions = optionsWithCurrent(options, draft.fallbackKey);

  return (
    <div className="card-row seat-row">
      <div className="seat-row-main">
        <span className="seat-row-role">
          <span className="seat-row-stage">{stage}</span>
          {focusOptions.length > 0 ? (
            <>
              {node.role || '未命名角色'}
              <FocusSelect
                options={focusOptions}
                current={node.focus ?? ''}
                disabled={saving || locked}
                onSave={saveFocus}
              />
            </>
          ) : seatRoleText(node.role || '未命名角色', node.focus)}
          {presetFixed && <span className="pill seat-row-tag">预设固定</span>}
        </span>
        <span className="card-sub">{node.visibility} · {node.write}</span>
      </div>
      <div className="seat-row-controls">
        <ModelPicker
          value={missing ? '' : draft.personKey}
          options={personOptions}
          placeholder="未选择"
          alert={invalid !== ''}
          empty={missing}
          badge={(
            <>
              {invalid !== '' && <span className="picker-badge alert">{invalid}</span>}
              {recommended && <span className="picker-badge">推荐</span>}
            </>
          )}
          extra={node.seat !== null ? { key: CLEAR_KEY, label: '清除人选' } : null}
          disabled={disabled}
          onSelect={(key) => (key === CLEAR_KEY ? clear() : save({ ...draft, personKey: key }))}
        />
        <select
          className="pill-select seat-row-thinking"
          aria-label="思考"
          title="思考"
          value={draft.thinking}
          disabled={disabled || draft.personKey === ''}
          onChange={(event) => save({ ...draft, thinking: event.target.value })}
        >
          {THINKING_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>{option.value === '' ? '思考 · 预设' : `思考 · ${option.label}`}</option>
          ))}
        </select>
        <ModelPicker
          value={draft.fallbackKey}
          options={fallbackOptions}
          placeholder="不设"
          icon="fallback"
          label="备选"
          extra={draft.fallbackKey !== '' ? { key: CLEAR_KEY, label: '不设备选' } : null}
          disabled={disabled || draft.personKey === ''}
          onSelect={(key) => save({ ...draft, fallbackKey: key === CLEAR_KEY ? '' : key })}
        />
      </div>
      {saving && <span className="seat-row-saving">保存中…</span>}
      {error !== '' && <p className="cfg-error seat-row-error">{error}</p>}
    </div>
  );
}

/** active：导航切到「模式」时为 true。页面常驻不卸载，每次切回重新取快照，
 * 资源页刚冒烟或探测过的结果随之进入人选下拉。外壳按主 agent 换 key，换主 agent 时
 * 整页重建。host 为 null 表示还没有可作主 agent 的 agent。 */
export function ModesPage({
  active,
  host,
  hostsLoaded,
  refreshKey,
  onRefreshDone,
  onGoDetect,
  onGoSkill,
  onGoTest,
  onVendorChanged,
}: {
  active: boolean;
  host: HostEntry | null;
  /** 主 agent 列表已读到；读到之前不显示「尚未探测资源」。 */
  hostsLoaded: boolean;
  /** 顶栏「刷新」的计数，变化时重新读取。 */
  refreshKey: number;
  /** 刷新触发的读取结束后回报（key 为本次 refreshKey），顶栏据此停止转圈。 */
  onRefreshDone: (key: number, ok: boolean) => void;
  onGoDetect: () => void;
  onGoSkill: () => void;
  /** 推荐后「去测试」：跳到资源页并展开该 agent 卡片。 */
  onGoTest: (agent: string) => void;
  onVendorChanged: () => void;
}) {
  const hostId = host?.id ?? '';
  const [snapshot, setSnapshot] = useState<ConfigSnapshot | null>(null);
  const [loadError, setLoadError] = useState('');
  const [defaultBusy, setDefaultBusy] = useState(false);
  const [seatBusy, setSeatBusy] = useState(false);
  const [recommendBusy, setRecommendBusy] = useState(false);
  const [headError, setHeadError] = useState('');
  const [notes, setNotes] = useState<string[]>([]);
  // 推荐后各 agent 未验证、没进候选池的模型数（「去测试」用）。
  const [unverified, setUnverified] = useState<{ agent: string; text: string }[]>([]);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const closeMenu = useCallback(() => setMenuOpen(false), []);
  useDismiss(menuOpen, menuRef, closeMenu);
  const [confirmAll, setConfirmAll] = useState(false);
  const closeConfirm = useCallback(() => setConfirmAll(false), []);
  // 自定义模式：编辑器初值（null 为关闭）、待删除的模式与受影响项、删除进行中。
  const [editorDraft, setEditorDraft] = useState<PresetDraft | null>(null);
  const closeEditor = useCallback(() => setEditorDraft(null), []);
  const [removeAsk, setRemoveAsk] = useState<{ mode: ModeEntry; lines: string[] } | null>(null);
  const closeRemove = useCallback(() => setRemoveAsk(null), []);
  const [removingPreset, setRemovingPreset] = useState(false);
  // 删除前统计受影响项（要读斜杠命令列表）进行中。
  const [countingImpact, setCountingImpact] = useState(false);
  // 数量调节：进行中的模式 id；减少会清人选时待确认的请求。
  const [countBusy, setCountBusy] = useState('');
  const [countAsk, setCountAsk] = useState<{ mode: ModeEntry; next: number; text: string } | null>(null);
  const closeCount = useCallback(() => setCountAsk(null), []);
  // 保存或删除自定义模式后加一，重读快照。
  const [reloadKey, setReloadKey] = useState(0);
  // skill 是否装好，供首次使用提示用；读不到时不提示。
  const [skillState, setSkillState] = useState<{ loaded: boolean; status: SkillStatus | null }>({ loaded: false, status: null });
  // 快照请求序号：刷新、设为默认、保存 / 清除人选、一键推荐都会改快照，各自发起时
  // 取号，结果落地前核对，不是最新一次的结果丢弃，避免旧结果盖掉新结果。
  const snapshotSeq = useRef(0);

  useEffect(() => {
    if (!active) return;
    if (!isTauri || hostId === '') {
      if (refreshKey > 0) onRefreshDone(refreshKey, true);
      return;
    }
    let cancelled = false;
    const seq = ++snapshotSeq.current;
    const snapshotLoad = commands.configSnapshot(hostId)
      .then((snapshot) => {
        if (cancelled || seq !== snapshotSeq.current) return true;
        setSnapshot(snapshot);
        setLoadError('');
        return true;
      })
      .catch((err) => {
        if (!cancelled) setLoadError(String(err));
        return false;
      });
    const skillLoad = commands.skillStatus()
      .then((status) => {
        if (!cancelled) setSkillState({ loaded: true, status });
        return true;
      })
      .catch(() => {
        if (!cancelled) setSkillState({ loaded: false, status: null });
        return false;
      });
    void Promise.all([snapshotLoad, skillLoad]).then(([a, b]) => {
      if (refreshKey > 0) onRefreshDone(refreshKey, a && b);
    });
    return () => { cancelled = true; };
  }, [active, hostId, refreshKey, reloadKey, onRefreshDone]);

  const options = useMemo(() => seatOptions(snapshot?.resources), [snapshot]);

  if (!isTauri) {
    return (
      <div className="page">
        <div className="page-body"><p className="cfg-empty">仅在 aicross 窗口中可用</p></div>
      </div>
    );
  }

  if (host === null) {
    return (
      <div className="page">
        <div className="page-body">
          {!hostsLoaded ? <p className="cfg-empty">读取中…</p> : (
            <div className="cfg-banner">
              <Icon name="alert" />
              <span className="cfg-banner-text">尚未探测资源</span>
              <button type="button" className="btn primary" onClick={onGoDetect}>去探测</button>
            </div>
          )}
        </div>
      </div>
    );
  }

  const config = snapshot?.config;
  const modes = config?.modes ?? [];
  const defaultMode = config?.defaultMode ?? '';

  const submitSeat = async (action: () => Promise<ModeEntry>) => {
    const seq = ++snapshotSeq.current;
    const updated = await action();
    if (seq !== snapshotSeq.current) return;
    setSnapshot((current) => current && {
      ...current,
      config: {
        ...current.config,
        modes: current.config.modes.map((item) => (item.id === updated.id ? updated : item)),
      },
    });
  };

  const setDefault = async (id: string) => {
    setDefaultBusy(true);
    setHeadError('');
    const seq = ++snapshotSeq.current;
    try {
      const result = await commands.configSetDefaultMode(hostId, id);
      if (seq !== snapshotSeq.current) return;
      setSnapshot((current) => current && {
        ...current,
        config: { ...current.config, defaultMode: result.defaultMode, modes: result.modes },
      });
    } catch (err) {
      setHeadError(String(err));
    } finally {
      setDefaultBusy(false);
    }
  };

  const recommend = async (all: boolean) => {
    setConfirmAll(false);
    setMenuOpen(false);
    setRecommendBusy(true);
    setHeadError('');
    const seq = ++snapshotSeq.current;
    try {
      const result = await commands.configRecommend(hostId, null, all);
      if (seq === snapshotSeq.current) {
        setSnapshot((current) => current && {
          ...current,
          config: { ...current.config, defaultMode: result.defaultMode, modes: result.modes },
        });
      }
      setNotes(recommendNotes(result.notes));
      setUnverified(unverifiedLines(result.unverified));
    } catch (err) {
      setHeadError(String(err));
    } finally {
      setRecommendBusy(false);
    }
  };

  const editing = seatBusy || recommendBusy || removingPreset || countBusy !== '' || countingImpact;

  const applyCount = async (mode: ModeEntry, next: number) => {
    setCountAsk(null);
    setCountBusy(mode.id);
    setHeadError('');
    try {
      await submitSeat(() => commands.configSetCount(hostId, mode.id, next));
    } catch (err) {
      setHeadError(String(err));
    } finally {
      setCountBusy('');
    }
  };

  /** 减少时末尾位置有本模式人选就先确认；增加与不清人选的减少直接调。 */
  const changeCount = (mode: ModeEntry, delta: 1 | -1) => {
    const count = mode.count;
    if (!count) return;
    const next = count.value + delta;
    if (delta < 0) {
      const lost = seatsLostOnDecrease(mode, count.role, next);
      if (lost > 0) {
        setCountAsk({ mode, next, text: decreaseConfirmText(count.role, next, lost) });
        return;
      }
    }
    void applyCount(mode, next);
  };

  /** 删除前按当前快照数出受影响项；斜杠命令数取本主 agent 的列表，读不到按 0。 */
  const askRemove = async (mode: ModeEntry) => {
    // 已有对话框或操作进行中不再发起；统计期间置忙，连点不会叠出多个确认框。
    if (editing || editorDraft !== null || removeAsk !== null) return;
    setCountingImpact(true);
    let slash: { mode: string }[] = [];
    try {
      slash = (await commands.slashList(hostId)).commands;
    } catch {
      slash = [];
    }
    setCountingImpact(false);
    setRemoveAsk({ mode, lines: impactLines(removeImpact(mode, defaultMode, slash)) });
  };

  const removePreset = async (mode: ModeEntry) => {
    setRemoveAsk(null);
    setRemovingPreset(true);
    setHeadError('');
    try {
      await commands.presetRemove(mode.id);
    } catch (err) {
      setHeadError(String(err));
    } finally {
      setRemovingPreset(false);
      setReloadKey((value) => value + 1);
    }
  };

  const onPresetSaved = () => {
    setEditorDraft(null);
    setReloadKey((value) => value + 1);
  };
  const prompt = snapshot === null ? null : modesPrompt({
    resources: snapshot.resources,
    skillHost: findHostStatus(skillState.status, hostId),
    skillLoaded: skillState.loaded,
    modes,
  });

  return (
    <div className="page">
      <PageHeader
        left={<HostBar host={host} onVendorChanged={onVendorChanged} />}
        actions={(
          <>
            <button
              type="button"
              className="btn"
              disabled={snapshot === null || editing}
              onClick={() => setEditorDraft(emptyDraft())}
            >
              <Icon name="plus" />
              新建模式
            </button>
            {prompt !== 'recommend' && (
              <button
                type="button"
                className="btn primary"
                disabled={snapshot === null || editing}
                onClick={() => void recommend(false)}
              >
                <Icon name="sparkles" />
                {recommendBusy ? '推荐中…' : '一键推荐'}
              </button>
            )}
            <div className="menu-anchor" ref={menuRef}>
              <button
                type="button"
                className={`icon-btn${menuOpen ? ' on' : ''}`}
                title="更多"
                aria-label="更多"
                aria-haspopup="menu"
                aria-expanded={menuOpen}
                disabled={snapshot === null || editing}
                onClick={() => setMenuOpen((open) => !open)}
              >
                <Icon name="more" />
              </button>
              {menuOpen && (
                <div className="menu" role="menu">
                  <button type="button" role="menuitem" className="menu-item" onClick={() => { setMenuOpen(false); setConfirmAll(true); }}>
                    全部按推荐重填
                  </button>
                </div>
              )}
            </div>
          </>
        )}
      />
      <div className="page-body">
        {prompt === 'detect' && (
          <div className="cfg-banner">
            <Icon name="alert" />
            <span className="cfg-banner-text">尚未探测资源</span>
            <button type="button" className="btn primary" onClick={onGoDetect}>去探测</button>
          </div>
        )}
        {prompt === 'install' && (
          <div className="cfg-banner">
            <Icon name="alert" />
            <span className="cfg-banner-text">{host.name || host.id} 未安装 ai-cross skill</span>
            <button type="button" className="btn primary" onClick={onGoSkill}>去安装</button>
          </div>
        )}
        {prompt === 'recommend' && (
          <div className="cfg-banner">
            <Icon name="alert" />
            <span className="cfg-banner-text">未配置人选</span>
            <button type="button" className="btn primary" disabled={editing} onClick={() => void recommend(false)}>
              <Icon name="sparkles" />
              {recommendBusy ? '推荐中…' : '一键推荐'}
            </button>
          </div>
        )}
        {(notes.length > 0 || unverified.length > 0) && (
          <div className="cfg-banner">
            <Icon name="sparkles" />
            <div className="cfg-banner-text">
              {notes.map((note, index) => <div key={index}>{note}</div>)}
              {unverified.map((item) => (
                <div key={item.agent} className="banner-action-line">
                  <span>{item.text}</span>
                  <button
                    type="button"
                    className="btn small"
                    onClick={() => {
                      // 去测试后验证状态会变，这条推荐结果随之过时：清掉，回来重新推荐时再出。
                      setNotes([]);
                      setUnverified([]);
                      onGoTest(item.agent);
                    }}
                  >
                    去测试
                  </button>
                </div>
              ))}
            </div>
            <button
              type="button"
              className="icon-btn"
              title="关闭"
              aria-label="关闭"
              onClick={() => { setNotes([]); setUnverified([]); }}
            >
              <Icon name="close" />
            </button>
          </div>
        )}
        {loadError !== '' && <p className="cfg-error cfg-head-error">{loadError}</p>}
        {headError !== '' && <p className="cfg-error cfg-head-error">{headError}</p>}
        {snapshot === null && loadError === '' && <p className="cfg-empty">读取中…</p>}
        {snapshot !== null && <StatCards items={modesOverview(config)} />}
        {groupModes(modes).map((group) => (
          <section key={group.title} className="cfg-group">
            <h2 className="cfg-group-title">
              {group.title}
              {group.title === '自定义' && (
                <button
                  type="button"
                  className="btn small cfg-group-action"
                  disabled={editing}
                  onClick={() => setEditorDraft(emptyDraft())}
                >
                  <Icon name="plus" size={14} />
                  新建
                </button>
              )}
            </h2>
            <div className="card-stack">
              {group.modes.map((mode) => {
                const isDefault = mode.id === defaultMode;
                const kind = modeKind(mode.id, mode.name);
                const seats = flowColumns(mode.stages).flatMap((column) => column.seats);
                return (
                  <section key={mode.id} className={`card${isDefault ? ' active' : ''}`}>
                    <div className="card-row card-head">
                      <span className={`icon-block toned tone-${kind}`}>
                        <Icon name={MODE_ICONS[kind]} size={18} />
                      </span>
                      <span className="card-head-text">
                        <span className="card-title">{mode.name}</span>
                        {(mode.description ?? '').trim() !== '' && (
                          <span className="card-sub" title={mode.description}>{mode.description}</span>
                        )}
                      </span>
                      <span className="cfg-spacer" />
                      {mode.count && (
                        <span className="count-stepper">
                          <span className="count-label">{mode.count.role} × {mode.count.value}</span>
                          <button
                            type="button"
                            className="icon-btn count-btn"
                            title="减少"
                            aria-label={`减少${mode.count.role}`}
                            disabled={editing || !canDecrease(mode.count)}
                            onClick={() => changeCount(mode, -1)}
                          >
                            <Icon name="minus" size={14} />
                          </button>
                          <button
                            type="button"
                            className="icon-btn count-btn"
                            title="增加"
                            aria-label={`增加${mode.count.role}`}
                            disabled={editing || !canIncrease(mode.count)}
                            onClick={() => changeCount(mode, 1)}
                          >
                            <Icon name="plus" size={14} />
                          </button>
                        </span>
                      )}
                      {mode.builtin ? (
                        <button
                          type="button"
                          className="icon-btn"
                          title="复制为新模式"
                          aria-label="复制为新模式"
                          disabled={editing}
                          onClick={() => setEditorDraft(draftFromCopy(mode))}
                        >
                          <Icon name="copy" />
                        </button>
                      ) : (
                        <>
                          <button
                            type="button"
                            className="icon-btn"
                            title="编辑"
                            aria-label="编辑"
                            disabled={editing}
                            onClick={() => setEditorDraft(draftFromMode(mode))}
                          >
                            <Icon name="edit" />
                          </button>
                          <button
                            type="button"
                            className="icon-btn"
                            title="删除"
                            aria-label="删除"
                            disabled={editing}
                            onClick={() => void askRemove(mode)}
                          >
                            <Icon name="trash" />
                          </button>
                        </>
                      )}
                      {isDefault ? (
                        <span className="cfg-active-mark"><span className="cfg-dot" />默认</span>
                      ) : (
                        <button
                          type="button"
                          className="btn small"
                          disabled={defaultBusy || editing}
                          onClick={() => void setDefault(mode.id)}
                        >
                          <Icon name="star" size={14} />
                          设为默认
                        </button>
                      )}
                    </div>
                    {seats.length === 0 && <div className="card-row card-note">无位置</div>}
                    {seats.map((item) => (
                      <SeatRow
                        // 人选变了按新值重建本行。
                        key={`${item.stage}:${item.index}:${JSON.stringify(item.node.seat)}`}
                        host={hostId}
                        mode={mode}
                        stage={item.stage}
                        index={item.index}
                        node={item.node}
                        options={options}
                        locked={editing}
                        submit={submitSeat}
                        onBusyChange={setSeatBusy}
                      />
                    ))}
                  </section>
                );
              })}
            </div>
          </section>
        ))}
      </div>
      {editorDraft && (
        <PresetEditor initial={editorDraft} others={modes} onSaved={onPresetSaved} onClose={closeEditor} />
      )}
      {removeAsk && (
        <ConfirmDialog
          title={`删除模式 ${removeAsk.mode.name}`}
          confirmLabel="删除"
          onConfirm={() => void removePreset(removeAsk.mode)}
          onCancel={closeRemove}
        >
          {removeAsk.lines.length > 0 && (
            <ul className="dialog-list" aria-label="一并清理">
              {removeAsk.lines.map((line) => <li key={line}>{line}</li>)}
            </ul>
          )}
        </ConfirmDialog>
      )}
      {countAsk && (
        <ConfirmDialog
          title={countAsk.text}
          confirmLabel="减少"
          onConfirm={() => void applyCount(countAsk.mode, countAsk.next)}
          onCancel={closeCount}
        />
      )}
      {confirmAll && (
        <ConfirmDialog title="全部按推荐重填" confirmLabel="重填" onConfirm={() => void recommend(true)} onCancel={closeConfirm}>
          手动选的人选会被覆盖
        </ConfirmDialog>
      )}
    </div>
  );
}
