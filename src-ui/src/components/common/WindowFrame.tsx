// 无系统边框窗口的自绘部分：顶栏右上的窗口按钮，与窗口四边四角的缩放手柄。
// macOS 用系统红绿灯，不画按钮。

import { useEffect, useState } from 'react';
import { IS_MACOS } from '../../lib/os';
import { commands, isTauri, type WindowAction } from '../../tauri';

const BUTTONS: ReadonlyArray<{ action: WindowAction; label: string; path: string }> = [
  { action: 'minimize', label: '最小化', path: 'M1 5.5h8' },
  { action: 'toggle-maximize', label: '最大化', path: 'M1.5 1.5h7v7h-7z' },
  { action: 'close', label: '关闭', path: 'M1.5 1.5l7 7M8.5 1.5l-7 7' },
];

export function WindowButtons() {
  if (IS_MACOS) return null;
  return (
    <div className="window-buttons">
      {BUTTONS.map(({ action, label, path }) => (
        <button
          key={action}
          type="button"
          className={action === 'close' ? 'window-button window-button-close' : 'window-button'}
          aria-label={label}
          onClick={() => {
            if (isTauri) commands.windowAction(action).catch(() => {});
          }}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <path d={path} fill="none" stroke="currentColor" strokeWidth="1" />
          </svg>
        </button>
      ))}
    </div>
  );
}

type ResizeDirection =
  | 'North' | 'South' | 'East' | 'West'
  | 'NorthEast' | 'NorthWest' | 'SouthEast' | 'SouthWest';

/** 边条 4px 宽，四角 12px 见方，角上的手柄盖住边条两端。 */
const EDGE = 4;
const CORNER = 12;
const HANDLES: ReadonlyArray<{ direction: ResizeDirection; cursor: string; style: React.CSSProperties }> = [
  { direction: 'North', cursor: 'n-resize', style: { top: 0, left: CORNER, right: CORNER, height: EDGE } },
  { direction: 'South', cursor: 's-resize', style: { bottom: 0, left: CORNER, right: CORNER, height: EDGE } },
  { direction: 'West', cursor: 'w-resize', style: { left: 0, top: CORNER, bottom: CORNER, width: EDGE } },
  { direction: 'East', cursor: 'e-resize', style: { right: 0, top: CORNER, bottom: CORNER, width: EDGE } },
  { direction: 'NorthWest', cursor: 'nw-resize', style: { top: 0, left: 0, width: CORNER, height: CORNER } },
  { direction: 'NorthEast', cursor: 'ne-resize', style: { top: 0, right: 0, width: CORNER, height: CORNER } },
  { direction: 'SouthWest', cursor: 'sw-resize', style: { bottom: 0, left: 0, width: CORNER, height: CORNER } },
  { direction: 'SouthEast', cursor: 'se-resize', style: { bottom: 0, right: 0, width: CORNER, height: CORNER } },
];

/**
 * 缩放手柄。Windows 与 macOS 由系统处理边缘缩放，按下事件到不了页面；
 * Linux 的 WebKitGTK 不处理，靠这里调 startResizeDragging。最大化时不显示。
 */
export function ResizeHandles() {
  const maximized = useMaximized();
  if (!isTauri || maximized) return null;

  const begin = (direction: ResizeDirection) => async (event: React.MouseEvent) => {
    if (event.button !== 0) return;
    event.preventDefault();
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().startResizeDragging(direction);
    } catch { /* 当前平台不支持时忽略 */ }
  };

  return (
    <div className="resize-handles" aria-hidden="true">
      {HANDLES.map(({ direction, cursor, style }) => (
        <div key={direction} className="resize-handle" style={{ ...style, cursor }} onMouseDown={begin(direction)} />
      ))}
    </div>
  );
}

/** 跟踪窗口是否最大化；窗口每次改尺寸时重读。 */
function useMaximized(): boolean {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    if (!isTauri) return;
    let stop: (() => void) | undefined;
    let unmounted = false;
    void (async () => {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const current = getCurrentWindow();
      const read = () => current.isMaximized().then(setMaximized, () => {});
      await read();
      const unlisten = await current.onResized(() => void read());
      if (unmounted) unlisten();
      else stop = unlisten;
    })().catch(() => {});
    return () => {
      unmounted = true;
      stop?.();
    };
  }, []);
  return maximized;
}
