// main.tsx — Entry point

import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { invoke } from './tauri';
import { IS_MACOS } from './lib/platform';
import { followSystemTheme } from './lib/theme';
import './styles/global.css';

// macOS uses native window decorations + traffic lights (titleBarStyle:
// "Overlay" in tauri.macos.conf.json). Tag <html> before first paint so the
// titlebar renders without our custom min/max/close and the #root corner clip
// is dropped — the OS owns the corners there. Win/Linux keep the frameless
// shell with custom controls on the right.
if (IS_MACOS) document.documentElement.classList.add('is-macos');

// 浅色 / 深色跟随系统设置，窗口开着时切换系统配色也会实时跟上。
followSystemTheme();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

// Window starts with `visible: false` (see tauri.conf.json) to hide the
// Windows-default chrome flash. Reveal it only after the first paint so
// the first frame the user sees is the final themed UI. Rust 侧另有 3 秒
// 兜底：前端起不来时强制显示窗口（server.rs setup）。
requestAnimationFrame(() => {
  requestAnimationFrame(() => {
    invoke('show_main_window').catch(() => {});
  });
});

// Suppress the WebView's built-in context menu (Back / Reload / Save As /
// Print / Inspect…). In `npm run dev` we leave it alone so the native
// context menu can reach "Inspect Element".
if (!import.meta.env.DEV) {
  window.addEventListener('contextmenu', (e) => {
    e.preventDefault();
  });

  // Production: block F12 / Ctrl+Shift+I / Ctrl+Shift+J / Ctrl+Shift+C to
  // prevent users from opening the WebView devtools on a shipped build.
  window.addEventListener('keydown', (e) => {
    if (e.key === 'F12') { e.preventDefault(); return; }
    if ((e.ctrlKey || e.metaKey) && e.shiftKey) {
      const k = e.key.toUpperCase();
      if (k === 'I' || k === 'J' || k === 'C') { e.preventDefault(); }
    }
  });
}
