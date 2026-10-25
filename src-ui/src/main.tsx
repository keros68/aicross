import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { invoke } from './tauri';
import { IS_MACOS } from './lib/os';
import { followSystemTheme } from './lib/theme';
import './styles/global.css';

// macOS 由系统画窗口按钮与圆角，样式里按这个类名让位。
if (IS_MACOS) document.documentElement.classList.add('is-macos');

followSystemTheme();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

// 窗口创建时隐藏，等第一帧画完再显示，避免先看到空白窗口。
// 前端加载失败时 Rust 侧 3 秒后照样显示。
requestAnimationFrame(() => requestAnimationFrame(() => {
  invoke('show_main_window').catch(() => {});
}));

// 安装版不弹浏览器右键菜单，也不让 F12 等快捷键打开开发者工具；开发时照常。
if (!import.meta.env.DEV) {
  const devtoolKeys = new Set(['I', 'J', 'C']);
  window.addEventListener('contextmenu', (event) => event.preventDefault());
  window.addEventListener('keydown', (event) => {
    const devtools = event.key === 'F12'
      || ((event.ctrlKey || event.metaKey) && event.shiftKey && devtoolKeys.has(event.key.toUpperCase()));
    if (devtools) event.preventDefault();
  });
}
