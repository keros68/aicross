import type { DispatchSettings } from '../tauri';

export interface UiDispatchSettings extends DispatchSettings {
  classificationModel: string;
}

export const DEFAULT_DISPATCH_SETTINGS: UiDispatchSettings = {
  timeoutSecs: 600,
  concurrency: 3,
  maxRequests: 8,
  classificationModel: '',
  redact: true,
  writeProjectDispatch: true,
  ccSwitchPath: '',
  validationCommand: '',
  acpEnabled: false,
};

const KEY = 'aicross-dispatch-settings';

export function loadDispatchSettings(): UiDispatchSettings {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULT_DISPATCH_SETTINGS };
    const parsed = JSON.parse(raw) as Partial<UiDispatchSettings>;
    return {
      timeoutSecs: Number.isFinite(parsed.timeoutSecs)
        ? Math.max(1, Math.min(3600, Number(parsed.timeoutSecs)))
        : 600,
      concurrency: Number.isFinite(parsed.concurrency)
        ? Math.max(1, Math.min(3, Number(parsed.concurrency)))
        : 3,
      maxRequests: Number.isFinite(parsed.maxRequests)
        ? Math.max(1, Math.min(100, Number(parsed.maxRequests)))
        : 8,
      classificationModel: typeof parsed.classificationModel === 'string' ? parsed.classificationModel : '',
      redact: parsed.redact !== false,
      writeProjectDispatch: parsed.writeProjectDispatch !== false,
      ccSwitchPath: typeof parsed.ccSwitchPath === 'string' ? parsed.ccSwitchPath : '',
      validationCommand: typeof parsed.validationCommand === 'string' ? parsed.validationCommand : '',
      acpEnabled: parsed.acpEnabled === true,
    };
  } catch {
    return { ...DEFAULT_DISPATCH_SETTINGS };
  }
}

export function saveDispatchSettings(value: UiDispatchSettings): void {
  localStorage.setItem(KEY, JSON.stringify(value));
  window.dispatchEvent(new CustomEvent('aicross:dispatch-settings', { detail: value }));
}
