import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import { dirname, resolve } from 'path';
import { fileURLToPath } from 'url';

const configDir = dirname(fileURLToPath(import.meta.url));

// xterm.js's WebGL addon (RectangleRenderer.updateBackgrounds) decides
// "draw a bg rect for this cell?" by `cell.bg !== 0`. But cell.bg is a
// 32-bit packed field whose top bits hold non-color flags (DIM, ITALIC,
// HAS_EXTENDED, ...). A cell that has DIM set but no actual bg color
// still has `cell.bg = 0x8000000 ≠ 0`, so the renderer paints
// `theme.background` over it — defeating Glass-mode transparency for
// any upstream that uses `\x1b[2m` (e.g. Codex's launcher box).
//
// The fix Warp's renderer arrived at independently: only consider the
// COLOR-MODE bits (Attributes.CM_MASK = 0x3000000). If color mode is
// DEFAULT (0), no rect — let the wallpaper bleed through. We string-
// replace the minified bundles' two equivalent guard expressions; both
// occurrences are inside `updateBackgrounds`. See
// https://github.com/xtermjs/xterm.js/issues/4212 (xterm wontfix).
function patchXtermWebglBgFlags(): Plugin {
  return {
    name: 'coffee-cli:patch-xterm-webgl-bg-flags',
    enforce: 'pre',
    transform(code, id) {
      if (!id.includes('@xterm/addon-webgl')) return null;
      const before = code;
      // Two minified pattern shapes (CJS vs ESM bundles use different
      // mangled names + literal-position).
      const after = code
        .replace(/u!==0\|\|d&&c!==0/g, '(u&50331648)!==0||d&&c!==0')
        .replace(/0!==a\|\|h&&0!==l/g, '0!==(a&50331648)||h&&0!==l');
      if (after === before) {
        // Don't fail the build if upstream changes its mangling — leave
        // a console hint and ship the unpatched bundle (degrades to
        // current behavior, doesn't break the app).
        this.warn(
          '[patch-xterm-webgl-bg-flags] guard pattern not found in ' +
          id +
          ' — xterm.js bundle layout changed, Glass dark plates will reappear until the regex is updated'
        );
        return null;
      }
      return { code: after, map: null };
    },
  };
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), patchXtermWebglBgFlags()],
  // The patch plugin above runs as a Vite transform — but Vite's dev
  // server pre-bundles node_modules with esbuild *before* plugin
  // transforms run, so the addon would ship un-patched in `tauri dev`.
  // Excluding it forces Vite to load it as raw ESM and apply our
  // transform on every dev import. Build mode is unaffected.
  optimizeDeps: {
    exclude: ['@xterm/addon-webgl'],
  },
  // Tauri dev server
  server: {
    port: 5173,
    strictPort: true,
  },
  // Allow Tauri IPC
  envPrefix: ['VITE_', 'TAURI_'],
  // Build-time platform constant. Each platform's installer is built on its
  // own CI runner (release.yml: ubuntu-22.04 / windows-latest / macos-13),
  // so process.platform here = the target platform of the binary being
  // produced. Far more reliable than navigator.userAgent — WebKit2GTK's UA
  // string is configurable and has been observed to omit "Linux" on some
  // distro/Tauri-version combos, defeating the runtime gate.
  define: {
    __IS_LINUX__: JSON.stringify(process.platform === 'linux'),
  },
  build: {
    // Tauri supports es2021
    target: ['es2021', 'chrome105', 'safari15'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    outDir: 'dist',
    rollupOptions: {
      input: {
        main: resolve(configDir, 'index.html'),
      },
      output: {
        // `codeSplitting` is a rolldown option. Vite 8's OutputOptions type
        // already accepts it, so no cast is needed here (the earlier
        // `as never` was unnecessary and is gone).
        //
        // Keeps every emitted chunk under the 500 kB warning line (TASK-05 C).
        // Two separate problems:
        //   1. The app bundle had swallowed React, xterm and the Markdown
        //      pipeline. They are big, independent, and change on their own
        //      release cadence, so they get their own chunks — the app chunk
        //      then re-downloads alone when only our code changed.
        //   2. Shiki grammars import each other (cpp pulls in glsl, sql,
        //      regexp, asm...). Left alone the bundler inlines those copies
        //      into every importer, so cpp came out at 660 kB with four other
        //      grammars duplicated inside it. One chunk per grammar file
        //      de-duplicates them; each is still fetched only on demand.
        // Rolldown's `codeSplitting.groups` rather than Rollup's
        // `manualChunks`: the compat path for the latter only honours the
        // requested name for modules that already start a chunk, which left
        // the shared grammars inlined.
        codeSplitting: {
          // On by default a group also swallows its dependencies, which is
          // exactly what inflated cpp (it dragged in cpp-macro) and graphql
          // (it dragged in the whole JS/TS family). Turning it off gives the
          // classic Rollup `manualChunks` semantics: a group takes the modules
          // it matches and nothing else.
          //
          // Rolldown suggests pairing this with `strictExecutionOrder: true`.
          // Do not: it promotes every lazily-imported grammar into the entry
          // chunk's static preload set, so index.html ends up with a
          // <link rel="modulepreload"> for all 45 grammars and the app
          // downloads every one of them at startup. The grammars are pure
          // data modules with no side effects, so the ordering guarantee buys
          // nothing here.
          includeDependenciesRecursively: false,
          groups: [
            {
              name: (id: string) => {
                const m = id.match(/@shikijs[\\/]langs[\\/]dist[\\/]([\w.@-]+)\.mjs/);
                return m ? `lang-${m[1]}` : null;
              },
              minSize: 0,
            },
            { name: 'xterm', test: /node_modules[\\/]@xterm[\\/]/ },
            {
              name: 'markdown',
              test: /node_modules[\\/](react-markdown|remark-[\w-]+|mdast-[\w-]+|micromark[\w-]*|hast-[\w-]+|unist-[\w-]+|vfile[\w-]*|property-information|comma-separated-tokens|space-separated-tokens|html-url-attributes|trim-lines|zwitch|longest-streak|ccount|markdown-table|devlop|bail|is-plain-obj|trough|unified|decode-named-character-reference|character-entities[\w-]*)[\\/]/,
            },
            { name: 'react', test: /node_modules[\\/](react|react-dom|scheduler)[\\/]/ },
            // Everything else from node_modules that is not already grouped.
            { name: 'vendor', test: /node_modules/ },
          ],
        },
      },
    },
  },
});
