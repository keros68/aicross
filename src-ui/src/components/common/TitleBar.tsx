// TitleBar.tsx — 自绘标题栏（TASK-40 最小形态）：拖动、双击最大化、
// 最小化 / 最大化 / 关闭。Tauri 的 decorations: false 需要它替代系统边框。
// macOS 用原生红绿灯（tauri.macos.conf.json 的 Overlay），不画右侧三键。

import { IS_MACOS } from '../../lib/platform';
import { commands, isTauri } from '../../tauri';
import './TitleBar.css';

export function TitleBar() {
  const minimize = () => isTauri && commands.windowMinimize().catch(() => {});
  const maximize = () => isTauri && commands.windowMaximize().catch(() => {});
  const close    = () => isTauri && commands.windowClose().catch(() => {});

  // Drag is wired as a single JS path: mousedown anywhere on the bar (except
  // on a child <button>) calls `startDragging()`, double-click toggles
  // maximize. Do NOT also set `data-tauri-drag-region` or
  // `-webkit-app-region: drag` — two drag mechanisms on one element race
  // Tauri's drag and stall paint on direction reversal.
  const onDragMouseDown = async (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest('button')) return;
    e.preventDefault();
    if (!isTauri) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().startDragging();
    } catch {}
  };
  const onDragDoubleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest('button')) return;
    maximize();
  };

  return (
    <div className="titlebar" onMouseDown={onDragMouseDown} onDoubleClick={onDragDoubleClick}>
      {/* macOS：左侧让出原生红绿灯的位置（Overlay 模式从 x:14 开始画）。 */}
      {IS_MACOS && <div className="titlebar-macos-slot" />}
      <div className="titlebar-drag-spacer" />
      {!IS_MACOS && (
        <div className="titlebar-controls">
          <button className="titlebar-btn" onClick={minimize} id="t-min" aria-label="最小化">
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect x="1" y="4.5" width="8" height="1" fill="currentColor"/>
            </svg>
          </button>
          <button className="titlebar-btn" onClick={maximize} id="t-max" aria-label="最大化">
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1"/>
            </svg>
          </button>
          <button className="titlebar-btn close" onClick={close} id="t-close" aria-label="关闭">
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path d="M1.5 1.5 l7 7 M1.5 8.5 l7 -7" fill="none" stroke="currentColor" strokeWidth="1"/>
            </svg>
          </button>
        </div>
      )}
    </div>
  );
}
