// personalization.ts — shared, pure data for the appearance/language controls.
//
// Extracted from Explorer.tsx so the new SettingsModal and the file-tree's
// icon renderer can both consume one source of truth (DRY). Only static option
// tables + small pure helpers live here — all dispatch/persistence wiring stays
// in the components.

import type { ThemeColor, ThemeShape, IconTheme } from '../store/app-state';

// ─── Theme colours (swatch grid) ─────────────────────────────────────────────
export const THEME_COLORS: { code: ThemeColor; labelKey: string; swatch: string; ring: string }[] = [
  { code: 'dark',  labelKey: 'theme.color.dark',  swatch: '#181818', ring: '#c4956a' },
  { code: 'light', labelKey: 'theme.color.light', swatch: '#f5f5f4', ring: '#9a6740' },
];

// ─── Theme shapes (corner/surface treatment) ─────────────────────────────────
export const THEME_SHAPES: { code: ThemeShape; label: string }[] = [
  { code: 'soft',   label: 'Soft'   },
  { code: 'slab',   label: 'Slab'   },
  { code: 'sharp',  label: 'Sharp'  },
  { code: 'glass',  label: 'Glass'  },
  { code: 'panel',  label: 'Panel'  },
  { code: 'carbon', label: 'Carbon' },
];

// ─── Task board form (to-do list vs sticky notes) ────────────────────────────
// Two presentations of the same task data, chosen in the settings "Tasks"
// section. Icons are inlined in SettingsModal (mirrors the other sections).
export const TASK_VIEW_MODES: { code: 'list' | 'note' | 'prompt'; labelKey: string; subKey: string }[] = [
  { code: 'list', labelKey: 'task.view.list', subKey: 'task.view.list.sub' },
  { code: 'note', labelKey: 'task.view.note', subKey: 'task.view.note.sub' },
  { code: 'prompt', labelKey: 'task.view.prompt', subKey: 'task.view.prompt.sub' },
];

// ─── File-tree icon art themes ───────────────────────────────────────────────
export const ICON_ART_THEMES: { id: IconTheme; folderSrc: string }[] = [
  { id: 'outline',          folderSrc: '/icons/themes/outline/folder-closed.svg'          },
  { id: 'material',         folderSrc: '/icons/themes/material/folder-closed.svg'         },
  { id: 'vscode-icons',     folderSrc: '/icons/themes/vscode-icons/folder-closed.svg'     },
  { id: 'catppuccin-mocha', folderSrc: '/icons/themes/catppuccin-mocha/folder-closed.svg' },
  { id: 'devicon',          folderSrc: '/icons/themes/devicon/folder-closed.svg'          },
  { id: 'fluent',           folderSrc: '/icons/themes/fluent/folder-closed.svg'           },
  { id: 'symbols',          folderSrc: '/icons/themes/symbols/folder-closed.svg'          },
  { id: 'coffee',           folderSrc: '/icons/themes/coffee/folder-closed.svg'           },
];

// Themes whose SVGs use fill="currentColor" and should be tinted by the active
// theme's --accent (rendered via mask-image instead of <img>).
export const MASK_TINT_THEMES: IconTheme[] = ['devicon'];
export function isMaskTintTheme(theme: IconTheme): boolean {
  return MASK_TINT_THEMES.includes(theme);
}

// ─── Languages ───────────────────────────────────────────────────────────────
export const LANGUAGES = [
  { code: 'en',    label: 'English',    glyph: 'A'  },
  { code: 'zh-CN', label: '简体中文',   glyph: '文' },
  { code: 'zh-TW', label: '繁體中文',   glyph: '文' },
  { code: 'ja',    label: '日本語',     glyph: 'あ' },
  { code: 'ko',    label: '한국어',     glyph: '가' },
  { code: 'es',    label: 'Español',    glyph: 'Ñ'  },
  { code: 'fr',    label: 'Français',   glyph: 'Fr' },
  { code: 'de',    label: 'Deutsch',    glyph: 'De' },
  { code: 'pt',    label: 'Português',  glyph: 'Pt' },
  { code: 'ru',    label: 'Русский',    glyph: 'Я'  },
  { code: 'vi',    label: 'Tiếng Việt', glyph: 'Vi' },
];
