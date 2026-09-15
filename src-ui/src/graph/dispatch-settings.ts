import type { DispatchSettings } from '../tauri';
import { DEFAULT_FIX_ROUND_LIMIT, FIX_ROUND_LIMIT_MAX } from './dispatch.ts';

export interface UiDispatchSettings extends DispatchSettings {
  classificationModel: string;
  // 蜂群一次最多几件（TASK-08）：与并发上限同一处，默认 4，也是解析清单时的截断线。
  swarmLimit: number;
}

export const DEFAULT_DISPATCH_SETTINGS: UiDispatchSettings = {
  timeoutSecs: 600,
  concurrency: 3,
  maxRequests: 8,
  classificationModel: '',
  swarmLimit: 4,
  redact: true,
  writeProjectDispatch: true,
  ccSwitchPath: '',
  validationCommand: '',
  acpEnabled: false,
  fixRoundLimit: DEFAULT_FIX_ROUND_LIMIT,
};

const KEY = 'aicross-dispatch-settings';

export function fixRoundLimitFromInput(value: string): number {
  if (value.trim() === '') return DEFAULT_FIX_ROUND_LIMIT;
  const parsed = Number(value);
  return Number.isFinite(parsed)
    ? Math.max(0, Math.min(FIX_ROUND_LIMIT_MAX, Math.floor(parsed)))
    : DEFAULT_FIX_ROUND_LIMIT;
}

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
      swarmLimit: Number.isFinite(parsed.swarmLimit)
        ? Math.max(1, Math.min(4, Number(parsed.swarmLimit)))
        : 4,
      redact: parsed.redact !== false,
      writeProjectDispatch: parsed.writeProjectDispatch !== false,
      ccSwitchPath: typeof parsed.ccSwitchPath === 'string' ? parsed.ccSwitchPath : '',
      validationCommand: typeof parsed.validationCommand === 'string' ? parsed.validationCommand : '',
      acpEnabled: parsed.acpEnabled === true,
      // 修正轮上限（TASK-12）：0 = 不派修正轮，到顶直接生成遗留节点。
      fixRoundLimit: Number.isFinite(parsed.fixRoundLimit)
        ? Math.max(0, Math.min(FIX_ROUND_LIMIT_MAX, Math.floor(Number(parsed.fixRoundLimit))))
        : DEFAULT_FIX_ROUND_LIMIT,
    };
  } catch {
    return { ...DEFAULT_DISPATCH_SETTINGS };
  }
}

export function saveDispatchSettings(value: UiDispatchSettings): void {
  localStorage.setItem(KEY, JSON.stringify(value));
  window.dispatchEvent(new CustomEvent('aicross:dispatch-settings', { detail: value }));
}
