// TitleBar.tsx — Custom draggable titlebar (replaces native OS window chrome)
// Tauri requires this for frameless windows with decorations: false
//
// Layout: [drag-area with panel toggles] … [min / max / close on the right]
//
// Left-side controls mirror VS Code's Activity Bar / Ctrl+B affordance:
//   1. Left panel toggle  (Explorer / directory listing)
//   2. Right panel toggle (TaskBoard / chat history)

import { lazy, Suspense, useEffect, useState } from 'react';
import { commands, isTauri } from '../../tauri';
import { useAppState, useAppDispatch, schemeLabels } from '../../store/app-state';
import { IS_MACOS } from '../../lib/platform';
import { useT } from '../../i18n/useT';
// 接入页按需载入（TASK-05 C 拆包）：整张覆盖层只在点「接入」后出现。
const ResourcesOverlay = lazy(() =>
  import('../resources/ResourcesOverlay').then((m) => ({ default: m.ResourcesOverlay })),
);
import { OPEN_RESOURCES_EVENT } from '../../resources/mention';
import './TitleBar.css';

export function TitleBar() {
  const { state } = useAppState();
  const dispatch = useAppDispatch();
  const t = useT();

  // 接入页（TASK-07）：本地开关 + 全局事件（左栏「管理 →」也走这个事件）。
  const [resourcesOpen, setResourcesOpen] = useState(false);
  useEffect(() => {
    const open = () => setResourcesOpen(true);
    window.addEventListener(OPEN_RESOURCES_EVENT, open);
    return () => window.removeEventListener(OPEN_RESOURCES_EVENT, open);
  }, []);

  // Shortcut labels are available on hover through native title tooltips.
  const hk = schemeLabels(state.hotkeyScheme);

  const minimize = () => isTauri && commands.windowMinimize().catch(() => {});
  const maximize = () => isTauri && commands.windowMaximize().catch(() => {});
  const close    = () => isTauri && commands.windowClose().catch(() => {});

  // Toggle just flips the layout flag — the OS window stays put and the
  // center column expands/contracts to fill the freed/reclaimed space,
  // matching VS Code / Cursor / Warp behavior. We previously also shrank
  // the OS window edge by 320px on the panel's axis, but the Tauri resize
  // IPC lands 1-3 frames after React commits, which made the center
  // column visibly squish-then-rebound (or expand-then-shrink) on every
  // toggle. Removing the window-edge move trades a non-standard "trim
  // collapsed window" effect for zero flicker — the right call.
  const toggleLeft  = () => dispatch({ type: 'TOGGLE_LEFT_PANEL' });
  const toggleRight = () => dispatch({ type: 'TOGGLE_RIGHT_PANEL' });

  // Drag is wired as a single JS path: mousedown anywhere on the bar (except
  // on a child <button>) calls `startDragging()`, double-click toggles
  // maximize. We previously set both `data-tauri-drag-region` and
  // `-webkit-app-region: drag` — two drag mechanisms on the same element —
  // and dropped them both for this single JS path; EchoBird uses the same
  // approach on the same Tauri version and is structurally cleaner.
  const onDragMouseDown = async (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest('button')) return;
    // Tool tabs now live in the bar — a press on the tab strip must reach the
    // tab's own click / pointer-reorder handlers, not start a window drag.
    if ((e.target as HTMLElement).closest('.chrome-tabs-header')) return;
    e.preventDefault();
    if (!isTauri) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().startDragging();
    } catch {}
  };
  const onDragDoubleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest('button')) return;
    if ((e.target as HTMLElement).closest('.chrome-tabs-header')) return;
    maximize();
  };

  return (
    <div className="titlebar" onMouseDown={onDragMouseDown} onDoubleClick={onDragDoubleClick}>
      {/* Left slot mirrors the left panel's width so the tab strip begins at
          the TOP OF THE CENTER COLUMN, not the window's left edge — that
          far-left strip belongs to the left sidebar (and hosts the macOS
          traffic lights). Collapses in lockstep with the panel via the shared
          --w-left var + the same 250ms curve. */}
      <div id="titlebar-brand-slot" className={`titlebar-left-slot${state.leftPanelHidden ? ' is-collapsed' : ''}`} />
      {/* Tool tabs (Windows-Terminal style) render here — CenterPanel portals
          its .chrome-tabs-header into this slot so the tabs sit on the drag bar
          above the center column, with the layout toggles + window controls to
          their right. The flexible spacer between them stays a draggable handle. */}
      <div className="titlebar-tabs" id="titlebar-tab-slot" />
      <div className="titlebar-drag-spacer" />
      {/* Icons come straight from Lucide (lucide.dev, ISC license). No
          runtime dependency — just the d-paths copied inline so we
          don't pay a 200KB+ import for four glyphs.

          Order (per user design):
            1. Multi-agent layout picker (only while a multi-agent tab is
               active — ephemeral, slides in when useful, out when not)
            2. Left / right panel toggles (always-on, fixed position on
               the right so they're in the same spot every session)

          No separator between groups — VS Code's titlebar uses pure
          proximity to group, which stays clean whether 2 or 4 icons
          are showing. */}
      {/* Sharp-corner 24×24 glyphs — strokeWidth 1.8 so the icons read
          at the same optical weight as Phosphor/Lucide at 16px render.
          Internal dividers are inset (y=4 → y=20) so they STOP before
          the outer border instead of crossing through it — avoids the
          "crossed-lines" look the user flagged. Active signal still travels
          via .is-active background only. */}
      <div className="titlebar-layout-toggles">
        <button
          className={`titlebar-btn titlebar-btn--layout${state.leftPanelHidden ? '' : ' is-active'}`}
          onClick={toggleLeft}
          aria-label="切换左栏"
          title={`切换左栏（${hk.left}）`}
          aria-pressed={!state.leftPanelHidden}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="square" strokeLinejoin="miter">
            <rect x="3" y="3" width="18" height="18" />
            <line x1="9" y1="4" x2="9" y2="20" />
          </svg>
        </button>
        <button
          className={`titlebar-btn titlebar-btn--layout${state.rightPanelHidden ? '' : ' is-active'}`}
          onClick={toggleRight}
          aria-label="切换右栏"
          title={`切换右栏（${hk.right}）`}
          aria-pressed={!state.rightPanelHidden}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="square" strokeLinejoin="miter">
            <rect x="3" y="3" width="18" height="18" />
            <line x1="15" y1="4" x2="15" y2="20" />
          </svg>
        </button>

        {/* 资源接入页（TASK-07）：插头图标，打开全屏覆盖层做探测/冒烟。 */}
        <button
          className={`titlebar-btn titlebar-btn--layout${resourcesOpen ? ' is-active' : ''}`}
          onClick={() => setResourcesOpen(true)}
          aria-label={t('resources.open')}
          title={t('resources.open')}
          aria-pressed={resourcesOpen}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <path d="M9 2v6" />
            <path d="M15 2v6" />
            <path d="M6 8h12v3a6 6 0 0 1-12 0V8z" />
            <path d="M12 17v5" />
          </svg>
        </button>

        {/* Personalization settings — gear opens the consolidated modal that
            replaced the old left-panel theme/language popovers. The `title`
            tooltip matters here: new users reported not finding settings
            because the gear sits unlabeled among the look-alike layout
            toggles (those at least carry hotkey-hint text). */}
        <button
          className={`titlebar-btn titlebar-btn--layout${state.settingsOpen ? ' is-active' : ''}`}
          onClick={() => dispatch({ type: 'TOGGLE_SETTINGS' })}
          aria-label={t('settings.title')}
          title={t('settings.title')}
          aria-pressed={state.settingsOpen}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
        </button>
      </div>

      {/* Custom min/max/close — Windows/Linux only. On macOS the OS draws the
          native traffic lights on the LEFT (titleBarStyle: "Overlay" in
          tauri.macos.conf.json), so we omit our own controls; the layout
          toggles above stay on the right and the left drag region hosts the
          native lights. */}
      {!IS_MACOS && (
        <div className="titlebar-controls">
          <button className="titlebar-btn" onClick={minimize} id="t-min">
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect x="1" y="4.5" width="8" height="1" fill="currentColor"/>
            </svg>
          </button>
          <button className="titlebar-btn" onClick={maximize} id="t-max">
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1"/>
            </svg>
          </button>
          <button className="titlebar-btn close" onClick={close} id="t-close">
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path d="M1.5 1.5 l7 7 M1.5 8.5 l7 -7" fill="none" stroke="currentColor" strokeWidth="1"/>
            </svg>
          </button>
        </div>
      )}
      {resourcesOpen && (
        <Suspense fallback={null}>
          <ResourcesOverlay onClose={() => setResourcesOpen(false)} />
        </Suspense>
      )}
    </div>
  );
}
