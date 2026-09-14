// shiki.ts — Singleton Shiki highlighter for the Diff panel.
// Lazy-loaded so the highlighter (and its grammar/theme bundles) only fetch
// when the user actually opens a diff. Languages load on demand the first
// time a file with that extension is highlighted; subsequent files of the
// same language hit the cache.

import type { HighlighterCore, ThemedToken } from 'shiki/core';
// The Oniguruma WASM ships as a real .wasm asset rather than the base64 JS
// module `shiki/wasm` — same bytes, but the browser gets a 600 kB binary it
// can stream instead of a 600 kB script chunk (TASK-05 C, 拆包).
import onigWasmUrl from 'shiki/onig.wasm?url';

let highlighterPromise: Promise<HighlighterCore> | null = null;

async function getHighlighter(): Promise<HighlighterCore> {
  if (!highlighterPromise) {
    // `shiki/core` instead of `shiki`: the root entry carries the registry of
    // every bundled language and theme, which pulls all ~600 grammars into the
    // build. Dynamic import still keeps all of it out of the main bundle.
    highlighterPromise = (async () => {
      const [{ createHighlighterCore }, { createOnigurumaEngine }, dark, light] =
        await Promise.all([
          import('shiki/core'),
          import('shiki/engine/oniguruma'),
          import('shiki/themes/github-dark-default.mjs'),
          import('shiki/themes/github-light-default.mjs'),
        ]);
      return createHighlighterCore({
        themes: [dark.default, light.default],
        langs: [],
        engine: createOnigurumaEngine(fetch(onigWasmUrl)),
      });
    })();
  }
  return highlighterPromise;
}

// File extension → Shiki language id. Covers the languages CLI users edit
// daily; unknown extensions fall through to plain text (no highlight, no
// crash). Add entries as needed.
const LANG_MAP: Record<string, string> = {
  ts: 'typescript', tsx: 'tsx', mts: 'typescript', cts: 'typescript',
  js: 'javascript', jsx: 'jsx', mjs: 'javascript', cjs: 'javascript',
  py: 'python', rs: 'rust', go: 'go', java: 'java',
  cs: 'csharp', cpp: 'cpp', cc: 'cpp', cxx: 'cpp',
  c: 'c', h: 'c', hpp: 'cpp',
  css: 'css', scss: 'scss', sass: 'sass', less: 'less',
  html: 'html', xml: 'xml', svg: 'xml', vue: 'vue', svelte: 'svelte',
  json: 'json', md: 'markdown', mdx: 'mdx',
  yml: 'yaml', yaml: 'yaml', toml: 'toml',
  sh: 'bash', bash: 'bash', zsh: 'bash',
  rb: 'ruby', php: 'php', swift: 'swift', kt: 'kotlin',
  sql: 'sql', dockerfile: 'docker',
};

// One explicit import per grammar we ship (TASK-05 C, 拆包).
//
// Shiki's own `highlighter.loadLanguage(someString)` resolves to
// `import('shiki/langs/' + id)`, which the bundler can only satisfy by
// emitting a chunk for EVERY grammar in the package — 600-odd of them,
// including several over 500 kB (emacs-lisp, wolfram) that nothing here can
// ever ask for. Listing the loaders keeps the emitted set to the languages
// LANG_MAP actually names; each is still its own lazily-fetched chunk.
const LANG_LOADERS: Record<string, () => Promise<unknown>> = {
  bash: () => import('shiki/langs/bash.mjs'),
  c: () => import('shiki/langs/c.mjs'),
  cpp: () => import('shiki/langs/cpp.mjs'),
  csharp: () => import('shiki/langs/csharp.mjs'),
  css: () => import('shiki/langs/css.mjs'),
  docker: () => import('shiki/langs/docker.mjs'),
  go: () => import('shiki/langs/go.mjs'),
  html: () => import('shiki/langs/html.mjs'),
  java: () => import('shiki/langs/java.mjs'),
  javascript: () => import('shiki/langs/javascript.mjs'),
  jsx: () => import('shiki/langs/jsx.mjs'),
  json: () => import('shiki/langs/json.mjs'),
  kotlin: () => import('shiki/langs/kotlin.mjs'),
  less: () => import('shiki/langs/less.mjs'),
  markdown: () => import('shiki/langs/markdown.mjs'),
  mdx: () => import('shiki/langs/mdx.mjs'),
  php: () => import('shiki/langs/php.mjs'),
  python: () => import('shiki/langs/python.mjs'),
  ruby: () => import('shiki/langs/ruby.mjs'),
  rust: () => import('shiki/langs/rust.mjs'),
  sass: () => import('shiki/langs/sass.mjs'),
  scss: () => import('shiki/langs/scss.mjs'),
  sql: () => import('shiki/langs/sql.mjs'),
  svelte: () => import('shiki/langs/svelte.mjs'),
  swift: () => import('shiki/langs/swift.mjs'),
  toml: () => import('shiki/langs/toml.mjs'),
  tsx: () => import('shiki/langs/tsx.mjs'),
  typescript: () => import('shiki/langs/typescript.mjs'),
  vue: () => import('shiki/langs/vue.mjs'),
  xml: () => import('shiki/langs/xml.mjs'),
  yaml: () => import('shiki/langs/yaml.mjs'),
};

function detectLanguage(filePath: string): string | null {
  const ext = filePath.split('.').pop()?.toLowerCase();
  return ext ? LANG_MAP[ext] ?? null : null;
}

export type LineTokens = ThemedToken[];

/** Theme for the syntax tokens, picked by the app's `data-theme` attribute.
 *  Coffee CLI has 9 ThemeColor values (dark/light/cappuccino/sakura/...)
 *  but only `light` is a true light palette; the rest are dark accent
 *  variants — so a binary light/dark split is the right mapping. */
export function getShikiTheme(dataTheme: string | null): 'github-light-default' | 'github-dark-default' {
  return dataTheme === 'light' ? 'github-light-default' : 'github-dark-default';
}

/** Tokenize `text` for syntax-highlighted rendering. Returns one
 *  ThemedToken[] per line. Resolves to `null` when the file's language
 *  isn't in LANG_MAP (caller should fall back to plain text) or when
 *  Shiki fails to load that grammar (rare — bundle missing). */
export async function tokenizeFile(
  text: string,
  filePath: string,
  theme: 'github-light-default' | 'github-dark-default',
): Promise<LineTokens[] | null> {
  const lang = detectLanguage(filePath);
  if (!lang) return null;
  return tokenizeAt(text, lang, theme);
}

// `tokenizeByLang(text, langHint, theme)` used to live here for Markdown
// fenced code blocks. Nothing ever called it, and the explicit LANG_LOADERS
// table changed what it would do with an unrecognised fence tag, so it went
// rather than ship an untested export. Bring it back from git history if a
// Markdown renderer needs it; it was a two-line wrapper over tokenizeAt.

async function tokenizeAt(
  text: string,
  lang: string,
  theme: 'github-light-default' | 'github-dark-default',
): Promise<LineTokens[] | null> {
  const highlighter = await getHighlighter();
  if (!highlighter.getLoadedLanguages().includes(lang)) {
    const load = LANG_LOADERS[lang];
    // A language we don't ship a grammar for (an exotic ``` fence tag) falls
    // through to plain text, same as an unrecognised file extension.
    if (!load) return null;
    try {
      await highlighter.loadLanguage((await load()) as any);
    } catch {
      return null;
    }
  }
  try {
    return highlighter.codeToTokens(text, { lang: lang as any, theme }).tokens;
  } catch {
    return null;
  }
}
