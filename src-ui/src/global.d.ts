/// <reference types="vite/client" />

// Build-time platform constant injected by vite.config.ts via `define`.
// True iff the binary was built on a Linux CI runner. Used to gate
// CPU-expensive WebKit2GTK animations (SMIL path morphs in masks, dot
// grids with box-shadow opacity loops) that are essentially free on
// WebView2 / WKWebView but peg cores on TextureMapper + Cairo.
declare const __IS_LINUX__: boolean;

// vitest 默认把 CSS 导入桩成空串（`?raw` 也一样），样式断言只能直接读文件。
// tsconfig.app.json 的 types 只有 vite/client，不引 @types/node（node 的全局类型
// 会盖掉 DOM 的 setTimeout 等签名），所以这里只声明用到的这一个函数。
declare module 'node:fs' {
  export function readFileSync(path: string | URL, encoding: 'utf8'): string;
}
