// 配色跟随系统：prefers-color-scheme 决定 <html data-theme>，系统切换时实时跟着变。
// index.html 里的内联脚本在首帧前先按同一规则设一次，避免闪烁。

export type Theme = 'light' | 'dark';

export const DARK_QUERY = '(prefers-color-scheme: dark)';

/** matchMedia 的结果 → 主题；拿不到（不支持 matchMedia）时用深色。 */
export function themeFromMedia(media: Pick<MediaQueryList, 'matches'> | null | undefined): Theme {
  if (!media) return 'dark';
  return media.matches ? 'dark' : 'light';
}

/** 设好当前主题并监听系统变化；返回注销函数。 */
export function followSystemTheme(root: HTMLElement = document.documentElement): () => void {
  let media: MediaQueryList | null = null;
  try {
    media = typeof window.matchMedia === 'function' ? window.matchMedia(DARK_QUERY) : null;
  } catch {
    media = null;
  }
  root.setAttribute('data-theme', themeFromMedia(media));
  if (!media) return () => {};
  const onChange = (event: MediaQueryListEvent) => {
    root.setAttribute('data-theme', themeFromMedia(event));
  };
  media.addEventListener('change', onChange);
  return () => media?.removeEventListener('change', onChange);
}
