// SettingsModal.tsx — centered personalization modal opened by the titlebar
// gear. Keeps the supported appearance, terminal, task, language and feedback
// controls in one sectioned surface.
//
// Layout: an icon rail (left) + a content column whose header (section title +
// close) is fixed while the body scrolls, so a long list (language) can never
// slide under the close button. Theme colours render as two-tone preview cards
// (surface band + accent band) with the label BELOW the swatch, never on it.
//
// Dispatch + persistence mirror the former Explorer wiring exactly so behaviour
// is unchanged; only the presentation moved.

import { useEffect, useState, type ReactNode } from 'react';
import { useAppState, useAppDispatch, type ThemeColor } from '../../store/app-state';
import { useT } from '../../i18n/useT';
import { IS_WINDOWS } from '../../lib/platform';
import { TERM_COLOR_SCHEMES } from '../center/TierTerminal';
import { commands, type AcpAdapterStatus, type FontInfo, type RoleProfile, type RoleSettings } from '../../tauri';
import type { DispatchableModel } from '../../graph/types';
import { attachmentDraftState, buildPremiseCheckGraph, matchPremiseResults, premiseCheckBlockReason, validatePremiseAttachments, type Premise } from '../../graph/premise';
import { modelStatusSuffix } from '../../resources/model-picker';
import { FontPicker } from './FontPicker';
import { THEME_COLORS, LANGUAGES, TASK_VIEW_MODES } from '../../lib/personalization';
import { fixRoundLimitFromInput, loadDispatchSettings, saveDispatchSettings, type UiDispatchSettings } from '../../graph/dispatch-settings';
import { FIX_ROUND_LIMIT_MAX, ROLE_PROMPT_TEMPLATES } from '../../graph/dispatch';
import {
  UI_SCALE_CHANGED_EVENT,
  UI_SCALE_LEVELS,
  readUiScale,
  updateUiScale,
  type UiScale,
} from '../../settings/ui-scale';
import './SettingsModal.css';

type Section = 'appearance' | 'terminal' | 'roles' | 'dispatch' | 'premises' | 'tasks' | 'language' | 'feedback';

// Trailing "opens outside the app" affordance on the feedback cards.
const ExternalLinkArrow = () => (
  <svg className="settings-feedback-card-arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M7 17 17 7" /><path d="M7 7h10v10" />
  </svg>
);

// Per-mode preview glyphs for the Tasks section (checklist vs sticky note).
const TASK_VIEW_ICONS: Record<'list' | 'note' | 'prompt', ReactNode> = {
  list: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 6h11" /><path d="M9 12h11" /><path d="M9 18h11" />
      <path d="m3 6 1 1 2-2" /><path d="m3 12 1 1 2-2" /><circle cx="4" cy="18" r="1" />
    </svg>
  ),
  note: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M15.5 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10l6-6V5a2 2 0 0 0-2-2Z" />
      <path d="M15 21v-5a1 1 0 0 1 1-1h5" />
    </svg>
  ),
  prompt: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 11V5a2 2 0 0 1 2-2h6l9 9-8 8-9-9Z" />
      <circle cx="7.5" cy="7.5" r="1.2" fill="currentColor" stroke="none" />
    </svg>
  ),
};

const ICONS: Record<Section, ReactNode> = {
  appearance: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="13.5" cy="6.5" r=".5" fill="currentColor" /><circle cx="17.5" cy="10.5" r=".5" fill="currentColor" />
      <circle cx="8.5" cy="7.5" r=".5" fill="currentColor" /><circle cx="6.5" cy="12.5" r=".5" fill="currentColor" />
      <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.563-2.512 5.563-5.563C21.5 6.012 17.262 2 12 2z" />
    </svg>
  ),
  terminal: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="m7 11 2-2-2-2" /><path d="M11 13h4" /><rect width="18" height="18" x="3" y="3" rx="2" ry="2" />
    </svg>
  ),
  dispatch: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="5" cy="12" r="2" /><circle cx="19" cy="6" r="2" /><circle cx="19" cy="18" r="2" />
      <path d="M7 12h4c3 0 3-6 6-6M11 12c3 0 3 6 6 6" />
    </svg>
  ),
  roles: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="8" cy="8" r="3" /><circle cx="17" cy="9" r="2.5" /><path d="M3 20c0-4 2-6 5-6s5 2 5 6M14 20c.2-2.7 1.5-4.2 3.5-4.2S21 17.3 21 20" />
    </svg>
  ),
  tasks: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M11 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-6" />
      <path d="m9 11 3 3L22 4" />
    </svg>
  ),
  premises: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z" />
      <path d="M14 2v6h6" /><path d="M9 13h6" /><path d="M9 17h6" />
    </svg>
  ),
  language: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" /><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" /><path d="M2 12h20" />
    </svg>
  ),
  feedback: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="m8 2 1.88 1.88" /><path d="M14.12 3.88 16 2" />
      <path d="M9 7.13V6a3 3 0 1 1 6 0v1.13" />
      <path d="M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6Z" />
      <path d="M12 20v-9" /><path d="M6.53 9C4.6 8.8 3 7.1 3 5" /><path d="M6 13H2" /><path d="M6 17c-1.7 0-3 1.3-3 3" />
      <path d="M17.47 9c1.93-.2 3.53-1.9 3.53-4" /><path d="M18 13h4" /><path d="M18 17c1.7 0 3 1.3 3 3" />
    </svg>
  ),
};

export function SettingsModal() {
  const { state } = useAppState();
  const dispatch = useAppDispatch();
  const t = useT();
  const [section, setSection] = useState<Section>('appearance');
  const [version, setVersion] = useState('');
  const [dispatchSettings, setDispatchSettings] = useState<UiDispatchSettings>(loadDispatchSettings);
  const [uiScale, setUiScale] = useState<UiScale>(readUiScale);
  const [roleSettings, setRoleSettings] = useState<RoleSettings>({
    version: 1,
    planning: { agent: '', provider: '', model: '', tier: '', thinking: '高' },
    execution: { agent: '', provider: '', model: '', tier: '', thinking: '低' },
    acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
  });
  const [roleModels, setRoleModels] = useState<DispatchableModel[]>([]);
  const [acpAdapters, setAcpAdapters] = useState<AcpAdapterStatus[] | null>(null);
  const [acpProbeError, setAcpProbeError] = useState('');
  // 前提台账（TASK-05B）：~/.aicross/premises.json，抽查图结论回本页并列显示
  const [premiseFile, setPremiseFile] = useState<{ version: 1; premises: Premise[] }>({ version: 1, premises: [] });
  const [premiseDraft, setPremiseDraft] = useState('');
  const [premiseTargetKey, setPremiseTargetKey] = useState('');
  const [premiseNote, setPremiseNote] = useState('');
  // 附件手输框的草稿（TASK-06 R1-13）：受控组件，按 premise id 存；保存成功后
  // 删掉这一条，框回到已存列表。校验没通过时草稿留着并标「未保存」。
  const [attachDrafts, setAttachDrafts] = useState<Record<string, string>>({});
  // Installed fonts for the terminal font picker — loaded lazily (Rust scan)
  // the first time the Terminal section is opened. null = not loaded yet.
  const [fonts, setFonts] = useState<FontInfo[] | null>(null);
  // Probed shell availability, fed to the default-shell picker so it only
  // offers shells the user actually has installed. null = not probed yet.
  // Platform-specific fields are absent cross-OS — read with optional
  // chaining.
  const [shellCaps, setShellCaps] = useState<{
    pwsh_available?: boolean; pwsh_version?: string | null; powershell_version?: string | null;
    git_bash_available?: boolean;
    zsh_available?: boolean; bash_available?: boolean;
    fish_available?: boolean; sh_available?: boolean;
    wsl_available: boolean;
  } | null>(null);

  const open = state.settingsOpen;
  const close = () => dispatch({ type: 'SET_SETTINGS_OPEN', open: false });

  // App version for the rail footer — pulled from the Tauri runtime (matches
  // tauri.conf.json) once, lazily so non-Tauri dev just shows nothing.
  useEffect(() => {
    let cancelled = false;
    import('@tauri-apps/api/app')
      .then(({ getVersion }) => getVersion())
      .then(v => { if (!cancelled) setVersion(v); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    const onScaleChanged = (event: Event) => {
      setUiScale((event as CustomEvent<UiScale>).detail);
    };
    window.addEventListener(UI_SCALE_CHANGED_EVENT, onScaleChanged);
    return () => window.removeEventListener(UI_SCALE_CHANGED_EVENT, onScaleChanged);
  }, []);

  // Lazy-load the system font list + probe available shells when the
  // Terminal section is first shown. Both come from the Rust side and are
  // cheap, but no point paying them when the user never opens Terminal.
  useEffect(() => {
    if (!open || section !== 'terminal') return;
    if (fonts === null) commands.listSystemFonts().then(setFonts).catch(() => setFonts([]));
    if (shellCaps === null) commands.detectShells().then(setShellCaps).catch(() => {});
  }, [open, section, fonts, shellCaps]);

  useEffect(() => {
    if (!open || section !== 'roles') return;
    commands.loadRoles().then(setRoleSettings).catch(() => {});
    commands.listDispatchableModels().then((result) => setRoleModels(result.models)).catch(() => setRoleModels([]));
  }, [open, section]);

  useEffect(() => {
    if (!open || section !== 'dispatch' || acpAdapters !== null) return;
    commands.detectAcpAdapters().then((items) => { setAcpAdapters(items); setAcpProbeError(''); }).catch((error) => { setAcpAdapters([]); setAcpProbeError(String(error)); });
  }, [open, section, acpAdapters]);
  // 前提台账：打开时读盘，并把已跑完的抽查图结论写回台账（按 last_graph_id
  // 读图匹配，取图中验证节点的完成回复；回写失败要提示，不吞）。
  useEffect(() => {
    if (!open || section !== 'premises') return;
    let dead = false;
    (async () => {
      const [file, rolesResult, modelsResult] = await Promise.all([
        commands.loadPremises().catch(() => ({ version: 1 as const, premises: [] as Premise[] })),
        commands.loadRoles().catch(() => null),
        commands.listDispatchableModels().catch(() => null),
      ]);
      if (dead) return;
      if (rolesResult) setRoleSettings(rolesResult);
      if (modelsResult) setRoleModels(modelsResult.models);
      const matched = await matchPremiseResults(file.premises, (id) => commands.loadGraph(id));
      if (dead) return;
      const next = { version: 1 as const, premises: matched.premises };
      if (matched.changed) {
        commands.savePremises(next).catch((err) => { if (!dead) setPremiseNote(String(err)); });
      }
      setPremiseFile(next);
    })();
    return () => { dead = true; };
  }, [open, section]);

  if (!open) return null;

  // ── Handlers (identical to the former left-panel ThemeMenu/Lang wiring) ──
  const setTheme = (th: ThemeColor) => dispatch({ type: 'SET_THEME', theme: th });
  const setLang = (code: string) => {
    dispatch({ type: 'SET_LANG', lang: code });
    try {
      localStorage.setItem('cc-lang', code);
      if (code !== 'en') localStorage.setItem('cc-native-lang', code);
    } catch {}
  };
  const setScheme = (id: string) => {
    try { id ? localStorage.setItem('cc-term-scheme', id) : localStorage.removeItem('cc-term-scheme'); } catch {}
    dispatch({ type: 'SET_TERM_SCHEME', scheme: id });
  };
  const setFont = (family: string) => {
    // Strip quotes/backslashes — the value is interpolated into a CSS
    // fontFamily string, so don't let a stray quote break out of it.
    const clean = family.replace(/["\\]/g, '');
    try { clean ? localStorage.setItem('cc-term-font', clean) : localStorage.removeItem('cc-term-font'); } catch {}
    dispatch({ type: 'SET_TERM_FONT', font: clean });
  };
  const setDefaultShell = (shell: string) => {
    try { shell ? localStorage.setItem('cc-default-shell', shell) : localStorage.removeItem('cc-default-shell'); } catch {}
    dispatch({ type: 'SET_DEFAULT_SHELL', shell });
  };
  const setTaskView = (mode: 'list' | 'note' | 'prompt') => dispatch({ type: 'SET_TASK_VIEW_MODE', mode });
  const setDispatchOption = <K extends keyof UiDispatchSettings>(key: K, value: UiDispatchSettings[K]) => {
    const next = { ...dispatchSettings, [key]: value };
    setDispatchSettings(next);
    saveDispatchSettings(next);
    if (key === 'acpEnabled' && value === false) commands.releaseAcpPool().catch(() => {});
  };
  const setRole = (key: 'planning' | 'execution' | 'acceptance', profile: RoleProfile) => {
    const next = { ...roleSettings, [key]: profile };
    setRoleSettings(next);
    commands.saveRoles(next).then(() => {
      window.dispatchEvent(new CustomEvent('aicross:presets-changed'));
    }).catch(() => {});
  };

  // ── 前提台账 ──────────────────────────────────────────────────────────
  const savePremiseFile = (next: { version: 1; premises: Premise[] }) => {
    setPremiseFile(next);
    commands.savePremises(next).catch((err) => setPremiseNote(String(err)));
  };
  const addPremise = () => {
    const text = premiseDraft.trim();
    if (!text) return;
    savePremiseFile({
      ...premiseFile,
      premises: [...premiseFile.premises, { id: crypto.randomUUID(), text, recorded_at: new Date().toISOString(), attachments: [] }],
    });
    setPremiseDraft('');
  };
  const removePremise = (id: string) =>
    savePremiseFile({ ...premiseFile, premises: premiseFile.premises.filter((p) => p.id !== id) });
  // 手输附件每行一条（TASK-06-5）：不再按逗号拆，含逗号的路径不会被拆坏。
  const setPremiseAttachments = (id: string, raw: string) => {
    const current = premiseFile.premises.find((p) => p.id === id);
    if (!current) return;
    const { attachments, unsaved } = attachmentDraftState(raw, current.attachments);
    if (!unsaved) {
      setAttachDrafts((drafts) => { const next = { ...drafts }; delete next[id]; return next; });
      return;
    }
    // 保存前校验（R2-2）：相对路径、同名 basename 会让抽查节点整体失败，拒绝并提示
    const invalid = validatePremiseAttachments(attachments);
    if (invalid) {
      // 草稿留在框里并标「未保存」（R1-13）：框里显示的与真正会外发的不是一回事
      setPremiseNote(`附件未保存：${invalid}`);
      return;
    }
    setAttachDrafts((drafts) => { const next = { ...drafts }; delete next[id]; return next; });
    savePremiseFile({
      ...premiseFile,
      premises: premiseFile.premises.map((p) => (p.id === id ? { ...p, attachments } : p)),
    });
  };
  // 文件选择器（R2-2）：系统对话框选出的就是绝对路径；与已有附件合并去重后保存。
  const pickPremiseAttachments = async (id: string) => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ multiple: true });
      if (!selected) return;
      const paths = (Array.isArray(selected) ? selected : [selected]).filter((p): p is string => typeof p === 'string');
      const current = premiseFile.premises.find((p) => p.id === id);
      if (!current || paths.length === 0) return;
      const attachments = [...new Set([...current.attachments, ...paths])];
      const invalid = validatePremiseAttachments(attachments);
      if (invalid) {
        setPremiseNote(`附件未保存：${invalid}`);
        return;
      }
      setAttachDrafts((drafts) => { const next = { ...drafts }; delete next[id]; return next; });
      savePremiseFile({
        ...premiseFile,
        premises: premiseFile.premises.map((p) => (p.id === id ? { ...p, attachments } : p)),
      });
    } catch (err) {
      setPremiseNote(String(err));
    }
  };
  const runPremiseCheck = (premise: Premise) => {
    // 建图前再校验一次附件（TASK-06-5）：premises.json 被手改过时这里才拦得住。
    const blocked = premiseCheckBlockReason(premise);
    if (blocked) {
      setPremiseNote(blocked);
      return;
    }
    const [agent, provider, model] = premiseTargetKey
      ? (JSON.parse(premiseTargetKey) as string[])
      : ['', '', ''];
    const graph = buildPremiseCheckGraph(
      premise,
      model ? { agent, provider: provider || undefined, model } : undefined,
    );
    commands.saveGraph(graph).then(() => {
      // 建图只记 last_graph_id（回写匹配用）；last_checked_at 等结论写回时再记
      savePremiseFile({
        ...premiseFile,
        premises: premiseFile.premises.map((p) =>
          p.id === premise.id ? { ...p, last_graph_id: graph.id } : p),
      });
      setPremiseNote(`已建图「${graph.title}」，在中栏对话图起始页打开并派发；结论自动回到本页`);
    }).catch((err) => setPremiseNote(String(err)));
  };

  const SECTIONS: { id: Section; label: string }[] = [
    { id: 'appearance', label: t('settings.appearance' as any) },
    { id: 'terminal',   label: t('settings.terminal' as any) },
    { id: 'roles',      label: '角色' },
    { id: 'dispatch',   label: '派发' },
    { id: 'premises',   label: '前提' },
    { id: 'tasks',      label: t('settings.tasks' as any) },
    { id: 'language',   label: t('settings.language' as any) },
    { id: 'feedback',   label: t('settings.feedback' as any) },
  ];
  const currentLabel = SECTIONS.find(s => s.id === section)?.label ?? '';

  // 抽查目标清单：排除 roles.json 三个角色所用厂商（非项目常用厂商才够独立）
  const roleVendors = new Set(
    [roleSettings.planning, roleSettings.execution, roleSettings.acceptance]
      .map((profile) =>
        roleModels.find((m) =>
          m.agent === profile.agent && m.id === profile.model &&
          (profile.provider === '' || m.provider === profile.provider))?.vendor)
      .filter((vendor): vendor is string => !!vendor),
  );
  const premiseModels = roleModels.filter((m) => !roleVendors.has(m.vendor));

  return (
    <div className="settings-overlay" onMouseDown={close}>
      <div
        className="settings-modal"
        onMouseDown={e => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <aside className="settings-rail">
          <div className="settings-rail-title">{t('settings.title' as any)}</div>
          {SECTIONS.map(s => (
            <button
              key={s.id}
              className={`settings-rail-item${section === s.id ? ' active' : ''}`}
              onClick={() => setSection(s.id)}
            >
              <span className="settings-rail-icon">{ICONS[s.id]}</span>
              {s.label}
            </button>
          ))}
          {version && <div className="settings-rail-version"><span className="settings-rail-version-num">v{version}</span></div>}
        </aside>

        <section className="settings-content">
          <header className="settings-header">
            <span className="settings-header-title">{currentLabel}</span>
            <button className="settings-close" onClick={close} aria-label="Close">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M18 6 6 18" /><path d="m6 6 12 12" />
              </svg>
            </button>
          </header>

          <div className="settings-body">
            {section === 'appearance' && (
              <>
                <div className="settings-section-label">{t('theme.section.color')}</div>
                <div className="settings-theme-grid">
                  {THEME_COLORS.map(c => {
                    const active = c.code === state.currentTheme;
                    return (
                      <button
                        key={c.code}
                        className={`settings-theme-card${active ? ' active' : ''}`}
                        onClick={() => setTheme(c.code)}
                        aria-pressed={active}
                      >
                        <span className="settings-theme-preview" style={{ ['--ring' as any]: c.ring }}>
                          <span className="settings-theme-band-bg" style={{ background: c.swatch }} />
                          <span className="settings-theme-band-accent" style={{ background: c.ring }} />
                          {active && (
                            <span className="settings-theme-check" style={{ background: c.ring }}>
                              <svg viewBox="0 0 24 24" fill="none" stroke="#000" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round"><path d="M20 6 9 17l-5-5" /></svg>
                            </span>
                          )}
                        </span>
                        <span className="settings-theme-name">{t(c.labelKey as any)}</span>
                      </button>
                    );
                  })}
                </div>

                <div className="settings-section-label">界面缩放</div>
                <div className="settings-scale-row">
                  <div className="settings-chip-row" role="group" aria-label="界面缩放">
                    {UI_SCALE_LEVELS.map((level) => (
                      <button
                        key={level}
                        type="button"
                        className={`settings-chip${uiScale === level ? ' active' : ''}`}
                        aria-pressed={uiScale === level}
                        onClick={() => updateUiScale(level)}
                      >
                        {level}%
                      </button>
                    ))}
                  </div>
                  <span className="settings-scale-hint">Ctrl + / - / 0</span>
                </div>

              </>
            )}

            {section === 'terminal' && (
              <>
                <div className="settings-section-label">{t('settings.terminal.scheme' as any)}</div>
                <div className="settings-chip-row">
                  <button
                    className={`settings-term-chip reset${state.termColorScheme === '' ? ' active' : ''}`}
                    onClick={() => setScheme('')}
                  >
                    Aa
                  </button>
                  {TERM_COLOR_SCHEMES.map(s => (
                    <button
                      key={s.id}
                      className={`settings-term-chip${state.termColorScheme === s.id ? ' active' : ''}`}
                      style={{ color: s.fg }}
                      onClick={() => setScheme(state.termColorScheme === s.id ? '' : s.id)}
                    >
                      Aa
                    </button>
                  ))}
                </div>

                <div className="settings-section-label">{t('settings.terminal.font' as any)}</div>
                <FontPicker fonts={fonts} value={state.termFont} onChange={setFont} />
                <div
                  className="settings-font-preview"
                  style={{ fontFamily: state.termFont ? `"${state.termFont}", monospace` : 'monospace' }}
                >
                  {'aicross · AaBb 0123 {}=>'}
                </div>

                {/* Default shell picker. Shell NAMES are not translated
                    (read from the OS / detection as-is); only this label
                    and the "not recommended" suffix are localized. Card
                    style reuses the gambit keyboard-card — same control
                    surface the user already knows from hotkey selection.
                    Shells that weren't detected installed are omitted so
                    the user can't pick a dead one; Auto is always first. */}
                <div className="settings-section-label">{t('settings.terminal.shell' as any)}</div>
                <div className="settings-key-row">
                  <button
                    className={`settings-key-card${state.defaultShell === '' ? ' active' : ''}`}
                    onClick={() => setDefaultShell('')}
                    aria-pressed={state.defaultShell === ''}
                  >
                    <span className="settings-key-combo"><kbd>Auto</kbd></span>
                    <span className="settings-key-sub">{t('settings.terminal.shell.auto' as any)}</span>
                  </button>
                  {IS_WINDOWS && (() => {
                    const caps = shellCaps;
                    // Order: pwsh (PowerShell 7) → PowerShell 5 (inbox) →
                    // Git Bash → cmd (末位). pwsh / git-bash only when
                    // detected; PowerShell 5 + cmd are inbox on every
                    // supported Windows so always offered.
                    //
                    // Labels show the major version (PowerShell 7 / 5) plus
                    // the EXACT probed version as a sub-label — 7 and 5 are
                    // distinct CLIs with different runtimes (.NET 8+ cross-
                    // platform vs inbox Windows-only) and users have
                    // opinions. A bare "pwsh" vs "PowerShell" pair was
                    // opaque; showing the real build (e.g. 7.4.6) lets users
                    // pick the one they want (most prefer 7) and proves the
                    // detection is live.
                    const opts: { id: string; label: string; show: boolean; version?: string; notRecommended?: boolean }[] = [
                      { id: 'pwsh', label: 'PowerShell 7', show: !!caps?.pwsh_available, version: caps?.pwsh_version ?? undefined },
                      { id: 'powershell', label: 'PowerShell 5', show: true, version: caps?.powershell_version ?? undefined },
                      { id: 'git-bash', label: 'Git Bash', show: !!caps?.git_bash_available },
                      { id: 'cmd', label: 'Command Prompt', show: true, notRecommended: true },
                    ];
                    return opts.filter(o => o.show).map(o => {
                      const active = state.defaultShell === o.id;
                      return (
                        <button
                          key={o.id}
                          className={`settings-key-card${active ? ' active' : ''}`}
                          onClick={() => setDefaultShell(o.id)}
                          aria-pressed={active}
                        >
                          <span className="settings-key-combo"><kbd>{o.label}</kbd></span>
                          {o.version && (
                            <span className="settings-key-sub">{o.version}</span>
                          )}
                          {o.notRecommended && !o.version && (
                            <span className="settings-key-sub">{t('settings.terminal.shell.not_recommended' as any)}</span>
                          )}
                        </button>
                      );
                    });
                  })()}
                  {!IS_WINDOWS && (() => {
                    // macOS/Linux: each candidate is probed via `which`, so
                    // only installed shells get a card (fish is absent by
                    // default on macOS → no dead card). Auto reads $SHELL.
                    const caps = shellCaps;
                    const opts = [
                      { id: 'zsh', label: 'zsh', show: !!caps?.zsh_available },
                      { id: 'bash', label: 'bash', show: !!caps?.bash_available },
                      { id: 'fish', label: 'fish', show: !!caps?.fish_available },
                      { id: 'sh', label: 'sh', show: !!caps?.sh_available },
                    ];
                    return opts.filter(o => o.show).map(o => {
                      const active = state.defaultShell === o.id;
                      return (
                        <button
                          key={o.id}
                          className={`settings-key-card${active ? ' active' : ''}`}
                          onClick={() => setDefaultShell(o.id)}
                          aria-pressed={active}
                        >
                          <span className="settings-key-combo"><kbd>{o.label}</kbd></span>
                        </button>
                      );
                    });
                  })()}
                </div>
              </>
            )}

            {section === 'roles' && (
              <>
                <div className="settings-section-label">经济模式默认角色</div>
                <div className="settings-dispatch-grid">
                  {([
                    ['planning', '出方案'],
                    ['execution', '执行'],
                    ['acceptance', '验收'],
                  ] as const).map(([key, label]) => {
                    const profile = roleSettings[key];
                    const availableModels = roleModels;
                    const configured = availableModels.some((model) =>
                      model.agent === profile.agent && model.provider === profile.provider && model.id === profile.model);
                    const value = configured
                      ? JSON.stringify([profile.agent, profile.provider, profile.model])
                      : JSON.stringify(['', '', '']);
                    return (
                      <div key={key} className="settings-dispatch-field settings-dispatch-wide">
                        <span>{label}</span>
                        <select value={value} onChange={(event) => {
                          const [agent, provider, model] = JSON.parse(event.target.value) as string[];
                          const hit = roleModels.find((item) => item.agent === agent && item.provider === provider && item.id === model);
                          setRole(key, { ...profile, agent, provider, model, tier: hit?.tier ?? profile.tier });
                        }}>
                          <option value={JSON.stringify(['', '', ''])}>未指定</option>
                          {availableModels.map((model) => (
                            <option key={JSON.stringify([model.agent, model.provider, model.id])} value={JSON.stringify([model.agent, model.provider, model.id])}>
                              {model.agent}/{model.provider ? `${model.provider}/` : ''}{model.id}{modelStatusSuffix(model)}
                            </option>
                          ))}
                        </select>
                        <div className="settings-key-row">
                          <input value={profile.tier} placeholder="档位" onChange={(event) => setRole(key, { ...profile, tier: event.target.value })} />
                          <select value={profile.thinking} onChange={(event) => setRole(key, { ...profile, thinking: event.target.value })}>
                            <option>关</option><option>低</option><option>中</option><option>高</option>
                          </select>
                        </div>
                      </div>
                    );
                  })}
                </div>
                <div className="settings-section-label">固定 prompt 模板（只读）</div>
                <div className="settings-role-templates">
                  {(['出方案', '执行', '验收'] as const).map((role) => (
                    <div key={role} className="settings-role-template">
                      <span>{role}</span>
                      <pre>{ROLE_PROMPT_TEMPLATES[role]}</pre>
                    </div>
                  ))}
                </div>
              </>
            )}

            {section === 'dispatch' && (
              <>
                <div className="settings-section-label">执行</div>
                <div className="settings-dispatch-grid">
                  <label className="settings-dispatch-field">
                    <span>默认超时（秒）</span>
                    <input
                      type="number"
                      min={1}
                      max={3600}
                      value={dispatchSettings.timeoutSecs}
                      onChange={(e) => setDispatchOption('timeoutSecs', Math.max(1, Math.min(3600, Number(e.target.value) || 1)))}
                    />
                  </label>
                  <label className="settings-dispatch-field">
                    <span>并发上限</span>
                    <select
                      value={dispatchSettings.concurrency}
                      onChange={(e) => setDispatchOption('concurrency', Number(e.target.value))}
                    >
                      <option value={1}>1</option><option value={2}>2</option><option value={3}>3</option>
                    </select>
                  </label>
                  <label className="settings-dispatch-field">
                    <span>蜂群件数上限</span>
                    <select
                      value={dispatchSettings.swarmLimit}
                      onChange={(e) => setDispatchOption('swarmLimit', Number(e.target.value))}
                    >
                      <option value={1}>1</option><option value={2}>2</option><option value={3}>3</option><option value={4}>4</option>
                    </select>
                  </label>
                  <label className="settings-dispatch-field">
                    <span>通道在飞上限</span>
                    <select
                      value={dispatchSettings.consoleRunLimit}
                      onChange={(e) => setDispatchOption('consoleRunLimit', Number(e.target.value))}
                    >
                      <option value={1}>1</option><option value={2}>2</option><option value={3}>3</option>
                    </select>
                  </label>
                  <label className="settings-dispatch-field">
                    <span>总请求上限</span>
                    <input
                      type="number"
                      min={1}
                      max={100}
                      value={dispatchSettings.maxRequests}
                      onChange={(e) => setDispatchOption('maxRequests', Math.max(1, Math.min(100, Number(e.target.value) || 1)))}
                    />
                  </label>
                  <label className="settings-dispatch-field">
                    <span>修正轮上限</span>
                    <input
                      type="number"
                      min={0}
                      max={FIX_ROUND_LIMIT_MAX}
                      value={dispatchSettings.fixRoundLimit}
                      onChange={(e) => setDispatchOption('fixRoundLimit', fixRoundLimitFromInput(e.target.value))}
                    />
                    <span>到上限后未闭合的条目生成遗留节点，不再派修正。验证命令未通过和测试未钉住都计入此上限；到顶后验收标为「已跳过」。</span>
                  </label>
                  <label className="settings-dispatch-field settings-dispatch-wide">
                    <span>归类模型</span>
                    <input
                      value={dispatchSettings.classificationModel}
                      placeholder="默认取已接入的低档模型"
                      disabled
                      title="归类未接入"
                    />
                    <span>未接入，当前不可用</span>
                  </label>
                  <label className="settings-dispatch-field settings-dispatch-wide">
                    <span>cc_switch.py 路径</span>
                    <input
                      value={dispatchSettings.ccSwitchPath}
                      placeholder="未配置"
                      onChange={(e) => setDispatchOption('ccSwitchPath', e.target.value)}
                    />
                  </label>
                  <label className="settings-dispatch-field settings-dispatch-wide">
                    <span>验证命令</span>
                    <input
                      value={dispatchSettings.validationCommand}
                      placeholder="可空，例如 python -m pytest"
                      onChange={(e) => setDispatchOption('validationCommand', e.target.value)}
                    />
                  </label>
                </div>
                <div className="settings-section-label">ACP 长驻</div>
                <div className="settings-key-row">
                  <button
                    className={`settings-key-card${dispatchSettings.acpEnabled ? ' active' : ''}`}
                    onClick={() => setDispatchOption('acpEnabled', !dispatchSettings.acpEnabled)}
                    aria-pressed={dispatchSettings.acpEnabled}
                  >
                    <span className="settings-key-combo">Kimi 长驻</span>
                    <span className="settings-key-sub">{dispatchSettings.acpEnabled ? '已开启' : '已关闭'}</span>
                  </button>
                </div>
                <div className="settings-dispatch-grid">
                  {(acpAdapters ?? []).map((adapter) => (
                    <div key={adapter.id} className="settings-dispatch-field">
                      <span>{adapter.label}</span>
                      <span>{adapter.installed ? `已检测到：${adapter.command}` : '未检测到'}</span>
                      {adapter.id !== 'kimi' && !adapter.installed && <span>二进制：{adapter.command}，需自行安装</span>}
                    </div>
                  ))}
                  {acpAdapters === null && <span>正在探测适配器…</span>}
                  {acpProbeError && <span>探测失败：{acpProbeError}</span>}
                </div>
                <button onClick={() => { setAcpAdapters(null); setAcpProbeError(''); }}>重新探测</button>
                <span>当前只有 Kimi 长驻运行，Claude Code 与 Codex 仅探测适配器。长驻下只读节点只允许读类请求；一次性 Kimi 不受限。</span>
                <div className="settings-section-label">外发与记录</div>
                <div className="settings-key-row">
                  <button
                    className={`settings-key-card${dispatchSettings.redact ? ' active' : ''}`}
                    onClick={() => setDispatchOption('redact', !dispatchSettings.redact)}
                    aria-pressed={dispatchSettings.redact}
                  >
                    <span className="settings-key-combo">外发脱敏</span>
                    <span className="settings-key-sub">{dispatchSettings.redact ? '已开启' : '已关闭'}</span>
                  </button>
                  <button
                    className={`settings-key-card${dispatchSettings.writeProjectDispatch ? ' active' : ''}`}
                    onClick={() => setDispatchOption('writeProjectDispatch', !dispatchSettings.writeProjectDispatch)}
                    aria-pressed={dispatchSettings.writeProjectDispatch}
                  >
                    <span className="settings-key-combo">派发记录</span>
                    <span className="settings-key-sub">{dispatchSettings.writeProjectDispatch ? '写入 .dispatch/' : '只写运行目录'}</span>
                  </button>
                </div>
              </>
            )}

            {section === 'premises' && (
              <>
                <div className="settings-section-label">前提台账</div>
                <div className="settings-premise-add">
                  <input
                    value={premiseDraft}
                    placeholder="一行一条，例如：上游 API 保持 v2 兼容"
                    onChange={(e) => setPremiseDraft(e.target.value)}
                    onKeyDown={(e) => { if (e.key === 'Enter' && !e.nativeEvent.isComposing) addPremise(); }}
                  />
                  <button onClick={addPremise}>记录</button>
                </div>
                <div className="settings-section-label">抽查目标（已排除三个角色所用厂商）</div>
                <div className="settings-dispatch-grid">
                  <label className="settings-dispatch-field settings-dispatch-wide">
                    <span>抽查发给</span>
                    <select value={premiseTargetKey} onChange={(e) => setPremiseTargetKey(e.target.value)}>
                      <option value="">派发时再选</option>
                      {premiseModels.map((model) => (
                        <option key={JSON.stringify([model.agent, model.provider, model.id])} value={JSON.stringify([model.agent, model.provider, model.id])}>
                          {model.agent}/{model.provider ? `${model.provider}/` : ''}{model.id}{modelStatusSuffix(model)}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                {premiseFile.premises.length === 0 && (
                  <p className="settings-feedback-desc">还没有记录的前提。</p>
                )}
                {premiseFile.premises.map((premise) => (
                  <div key={premise.id} className="settings-premise-row">
                    <div className="settings-premise-text">{premise.text}</div>
                    <div className="settings-premise-meta">
                      记录 {premise.recorded_at ? new Date(premise.recorded_at).toLocaleDateString('zh-CN') : '—'}
                      {' · 最近抽查 '}
                      {premise.last_checked_at ? new Date(premise.last_checked_at).toLocaleString('zh-CN') : '—'}
                    </div>
                    {premise.last_result && <pre className="settings-premise-result">{premise.last_result}</pre>}
                    <textarea
                      className="settings-premise-attach"
                      rows={Math.min(4, Math.max(2, premise.attachments.length + 1))}
                      value={attachDrafts[premise.id] ?? premise.attachments.join('\n')}
                      placeholder="附件路径，每行一条（可空），随抽查一起发出"
                      onChange={(e) => setAttachDrafts((drafts) => ({ ...drafts, [premise.id]: e.target.value }))}
                      onBlur={(e) => setPremiseAttachments(premise.id, e.target.value)}
                    />
                    {attachDrafts[premise.id] !== undefined
                      && attachmentDraftState(attachDrafts[premise.id], premise.attachments).unsaved
                      && <div className="settings-premise-unsaved">未保存，抽查仍按已存附件发出</div>}
                    <div className="settings-premise-actions">
                      <button onClick={() => pickPremiseAttachments(premise.id)}>选择文件</button>
                      <button onClick={() => runPremiseCheck(premise)}>抽查</button>
                      <button onClick={() => removePremise(premise.id)}>删除</button>
                    </div>
                  </div>
                ))}
                {premiseNote && <p className="settings-feedback-desc">{premiseNote}</p>}
              </>
            )}

            {section === 'tasks' && (
              <>
                <div className="settings-section-label">{t('settings.tasks.view' as any)}</div>
                <div className="settings-key-row">
                  {TASK_VIEW_MODES.map(m => {
                    const active = state.taskViewMode === m.code;
                    return (
                      <button
                        key={m.code}
                        className={`settings-key-card settings-taskview-card${active ? ' active' : ''}`}
                        onClick={() => setTaskView(m.code)}
                        aria-pressed={active}
                      >
                        <span className="settings-key-combo">
                          <span className="settings-taskview-icon">{TASK_VIEW_ICONS[m.code]}</span>
                          <span className="settings-taskview-label">{t(m.labelKey as any)}</span>
                        </span>
                        <span className="settings-key-sub">{t(m.subKey as any)}</span>
                      </button>
                    );
                  })}
                </div>
              </>
            )}

            {section === 'language' && (
              <div className="settings-lang-list">
                {LANGUAGES.map(lang => (
                  <button
                    key={lang.code}
                    className={`settings-lang-item${lang.code === state.currentLang ? ' active' : ''}`}
                    onClick={() => setLang(lang.code)}
                  >
                    <span className="settings-lang-glyph">{lang.glyph}</span>
                    <span className="settings-lang-label">{lang.label}</span>
                    {lang.code === state.currentLang && (
                      <span className="settings-lang-check">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M20 6 9 17l-5-5" /></svg>
                      </span>
                    )}
                  </button>
                ))}
              </div>
            )}

            {section === 'feedback' && (
              <>
                <p className="settings-feedback-desc">{t('settings.feedback.desc' as any)}</p>
                <div className="settings-feedback-cards">
                  <button
                    type="button"
                    className="settings-feedback-card"
                    onClick={() => commands.openUrl('https://github.com/keros68/aicross/issues').catch(() => {})}
                  >
                    <span className="settings-feedback-card-icon">
                      <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
                        <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
                      </svg>
                    </span>
                    <span className="settings-feedback-card-label">GitHub</span>
                    <ExternalLinkArrow />
                  </button>
                </div>
              </>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
