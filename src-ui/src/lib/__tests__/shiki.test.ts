// 冒烟：走真实的 loader 把一段代码 token 化。
//
// 拆包那一轮动了三处加载路径 —— `shiki` 根入口换成 `shiki/core`、主题改成
// 显式 import、Oniguruma WASM 从 base64 的 JS 模块换成 `?url` 资源。这三处
// 出错的表现都是「高亮悄悄没了」，纯函数测试看不见，所以这里真的建一次
// highlighter、真的加载语法、真的出 token。
//
// 测试跑在 node 环境，没有能 fetch `/assets/...` 的服务器，所以把 fetch 换成
// 从 node_modules 直接读那个 .wasm 的实现。被测代码这边不变。

import { beforeAll, afterAll, describe, expect, it } from 'vitest';
// tsconfig.app.json 的 `types` 里故意只有 vite/client：应用代码不该碰 node
// 内置模块，而把 @types/node 拉进来会连带改写 setTimeout 的返回类型、
// 打断 TierTerminal。只有这个测试要读磁盘上的 .wasm，所以就地压掉两条解析
// 错误，运行时由 vitest 的 node 环境提供。
// @ts-expect-error node builtin, not in this project's `types`
import { readFile } from 'node:fs/promises';
// @ts-expect-error node builtin, not in this project's `types`
import { createRequire } from 'node:module';

const realFetch = globalThis.fetch;

beforeAll(() => {
  const wasmPath = createRequire(import.meta.url).resolve('shiki/onig.wasm');
  globalThis.fetch = (async (input: unknown) => {
    const url = String(input);
    if (!url.endsWith('.wasm')) return realFetch(input as RequestInfo);
    const bytes = await readFile(wasmPath);
    return new Response(bytes, { headers: { 'content-type': 'application/wasm' } });
  }) as typeof fetch;
});

afterAll(() => {
  globalThis.fetch = realFetch;
});

describe('tokenizeFile', () => {
  it('highlights TypeScript', async () => {
    const { tokenizeFile } = await import('../shiki');
    const lines = await tokenizeFile(
      'const answer: number = 42;\n',
      'src/answer.ts',
      'github-dark-default',
    );

    expect(lines).not.toBeNull();
    expect(lines!.length).toBeGreaterThan(0);
    const text = lines![0].map((t) => t.content).join('');
    expect(text).toContain('const');
    // 真的分了词，不是整行一个 token；且带上了主题颜色。
    expect(lines![0].length).toBeGreaterThan(1);
    expect(lines![0][0].color).toMatch(/^#/);
  }, 30_000);

  it('highlights Python through a second grammar load', async () => {
    const { tokenizeFile } = await import('../shiki');
    const lines = await tokenizeFile('def main():\n    return 1\n', 'main.py', 'github-light-default');

    expect(lines).not.toBeNull();
    expect(lines!.length).toBeGreaterThanOrEqual(2);
    expect(lines![0].map((t) => t.content).join('')).toContain('def');
  }, 30_000);

  it('returns null for an extension we ship no grammar for', async () => {
    const { tokenizeFile } = await import('../shiki');
    expect(await tokenizeFile('hello', 'notes.unknownext', 'github-dark-default')).toBeNull();
    expect(await tokenizeFile('hello', 'LICENSE', 'github-dark-default')).toBeNull();
  }, 30_000);
});

describe('getShikiTheme', () => {
  it('maps only the light theme to light; everything else is a dark variant', async () => {
    const { getShikiTheme } = await import('../shiki');
    expect(getShikiTheme('light')).toBe('github-light-default');
    expect(getShikiTheme('dark')).toBe('github-dark-default');
    expect(getShikiTheme('sakura')).toBe('github-dark-default');
    expect(getShikiTheme(null)).toBe('github-dark-default');
  });
});
