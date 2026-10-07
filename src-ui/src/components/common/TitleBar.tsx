// TitleBar.tsx — 窗口按钮（最小化 / 最大化 / 关闭）。fix-49-r1 起不另占一条
// 标题栏，并入每页的页头行（PageHeader）；Tauri 的 decorations: false 需要它
// 替代系统边框。macOS 用原生红绿灯（tauri.macos.conf.json 的 Overlay，画在
// 左侧导航顶部），这里不画。

import { IS_MACOS } from '../../lib/platform';
import { commands, isTauri } from '../../tauri';
import './TitleBar.css';

export function WindowControls() {
  if (IS_MACOS) return null;
  const minimize = () => isTauri && commands.windowMinimize().catch(() => {});
  const maximize = () => isTauri && commands.windowMaximize().catch(() => {});
  const close    = () => isTauri && commands.windowClose().catch(() => {});

  return (
    <div className="titlebar-controls">
      <button type="button" className="titlebar-btn" onClick={minimize} aria-label="最小化">
        <svg width="10" height="10" viewBox="0 0 10 10">
          <rect x="1" y="4.5" width="8" height="1" fill="currentColor"/>
        </svg>
      </button>
      <button type="button" className="titlebar-btn" onClick={maximize} aria-label="最大化">
        <svg width="10" height="10" viewBox="0 0 10 10">
          <rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1"/>
        </svg>
      </button>
      <button type="button" className="titlebar-btn close" onClick={close} aria-label="关闭">
        <svg width="10" height="10" viewBox="0 0 10 10">
          <path d="M1.5 1.5 l7 7 M1.5 8.5 l7 -7" fill="none" stroke="currentColor" strokeWidth="1"/>
        </svg>
      </button>
    </div>
  );
}
