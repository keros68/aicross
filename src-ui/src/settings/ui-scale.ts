import { getCurrentWebview } from '@tauri-apps/api/webview';

export const UI_SCALE_LEVELS = [90, 100, 110, 125, 150] as const;
export type UiScale = (typeof UI_SCALE_LEVELS)[number];

export const UI_SCALE_STORAGE_KEY = 'aicross-ui-scale';
export const UI_SCALE_CHANGED_EVENT = 'aicross:ui-scale-changed';

type StorageReader = Pick<Storage, 'getItem'>;
type StorageWriter = Pick<Storage, 'setItem'>;
type ZoomRoot = { style: { zoom: string } };
type Shortcut = Pick<KeyboardEvent, 'key' | 'ctrlKey'> & Partial<Pick<KeyboardEvent, 'metaKey' | 'altKey'>>;
type ShortcutEvent = Shortcut & Pick<KeyboardEvent, 'preventDefault' | 'stopPropagation'> & Partial<Pick<KeyboardEvent, 'repeat'>>;
type ApplyScale = (scale: UiScale) => void | Promise<unknown>;
type EventTargetLike = Pick<EventTarget, 'dispatchEvent'>;

type UpdateUiScaleOptions = {
  storage?: StorageWriter;
  apply?: ApplyScale;
  eventTarget?: EventTargetLike;
};

type InitializeUiScaleOptions = {
  storage?: StorageReader;
  apply?: ApplyScale;
};

const appliedZoomModes = new WeakMap<ZoomRoot, 'webview' | 'css'>();

export function parseUiScale(raw: string | null | undefined): UiScale {
  if (raw === null || raw === undefined || raw.trim() === '') return 100;
  const value = Number(raw);
  if (!Number.isFinite(value)) return 100;
  return UI_SCALE_LEVELS.reduce((nearest, level) =>
    Math.abs(level - value) < Math.abs(nearest - value) ? level : nearest,
  );
}

export function serializeUiScale(scale: UiScale): string {
  return String(scale);
}

export function stepUiScale(scale: number, direction: -1 | 1): UiScale {
  const current = parseUiScale(String(scale));
  const index = UI_SCALE_LEVELS.indexOf(current);
  const next = Math.max(0, Math.min(UI_SCALE_LEVELS.length - 1, index + direction));
  return UI_SCALE_LEVELS[next];
}

export function uiScaleForShortcut(scale: number, shortcut: Shortcut): UiScale | null {
  if ((!shortcut.ctrlKey && !shortcut.metaKey) || shortcut.altKey) return null;
  if (shortcut.key === '0') return 100;
  if (shortcut.key === '=' || shortcut.key === '+') return stepUiScale(scale, 1);
  if (shortcut.key === '-' || shortcut.key === '_') return stepUiScale(scale, -1);
  return null;
}

export function readUiScale(storage: StorageReader = localStorage): UiScale {
  try {
    return parseUiScale(storage.getItem(UI_SCALE_STORAGE_KEY));
  } catch {
    return 100;
  }
}

export function persistUiScale(scale: UiScale, storage: StorageWriter = localStorage): void {
  try {
    storage.setItem(UI_SCALE_STORAGE_KEY, serializeUiScale(scale));
  } catch {
    // 缩放仍在本次运行中生效；存储不可用时下次启动回到默认值。
  }
}

export async function applyUiScale(
  scale: UiScale,
  setWebviewZoom: (factor: number) => Promise<void> = (factor) => getCurrentWebview().setZoom(factor),
  root: ZoomRoot = document.documentElement,
): Promise<'webview' | 'css'> {
  const factor = scale / 100;
  const previousMode = appliedZoomModes.get(root);
  try {
    await setWebviewZoom(factor);
    root.style.zoom = '';
    appliedZoomModes.set(root, 'webview');
    return 'webview';
  } catch {
    if (previousMode === 'webview') {
      try {
        await setWebviewZoom(1);
      } catch {
        // 不能把 CSS 倍率叠到残留的 WebView 倍率上；保留原 WebView 倍率。
        root.style.zoom = '';
        return 'webview';
      }
    }
    root.style.zoom = String(factor);
    appliedZoomModes.set(root, 'css');
    return 'css';
  }
}

export function updateUiScale(scale: UiScale, options: UpdateUiScaleOptions = {}): void {
  const storage = options.storage ?? localStorage;
  const apply = options.apply ?? applyUiScale;
  const eventTarget = options.eventTarget ?? window;
  persistUiScale(scale, storage);
  void apply(scale);
  eventTarget.dispatchEvent(new CustomEvent<number>(UI_SCALE_CHANGED_EVENT, { detail: scale }));
}

export function initializeUiScale(options: InitializeUiScaleOptions = {}): UiScale {
  const storage = options.storage ?? localStorage;
  const apply = options.apply ?? applyUiScale;
  const scale = readUiScale(storage);
  void apply(scale);
  return scale;
}

export function createUiScaleShortcutController(
  initialScale: UiScale,
  update: (scale: UiScale) => void = updateUiScale,
): {
  handleKeyDown: (event: ShortcutEvent) => void;
  sync: (scale: UiScale) => void;
} {
  let currentScale = initialScale;
  return {
    handleKeyDown(event) {
      const next = uiScaleForShortcut(currentScale, event);
      if (next === null) return;
      event.preventDefault();
      event.stopPropagation();
      if (event.repeat) return;
      currentScale = next;
      update(next);
    },
    sync(scale) {
      currentScale = scale;
    },
  };
}
