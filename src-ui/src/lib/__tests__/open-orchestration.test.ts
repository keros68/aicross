import { afterEach, describe, expect, it } from 'vitest';
import {
  consumeOrchestrationRequest,
  initialRightTab,
  OPEN_ORCHESTRATION_EVENT,
  requestOrchestrationPanel,
} from '../open-orchestration';

afterEach(() => {
  consumeOrchestrationRequest(); // 清掉残留，别让一条用例影响下一条
});

describe('initialRightTab', () => {
  it('honours the remembered tab when nothing is pending', () => {
    expect(initialRightTab('tasks', false)).toBe('tasks');
    expect(initialRightTab('orchestration', false)).toBe('orchestration');
  });

  it('falls back to 方案与编排 for a missing or junk value', () => {
    expect(initialRightTab(null, false)).toBe('orchestration');
    expect(initialRightTab('sessions', false)).toBe('orchestration');
  });

  // 这条是本次修的 bug：老用户上次停在「任务」，首启引导点「选预设」时右栏
  // 是收起的，展开后 RightPanel 才 mount —— 光靠事件它会读 localStorage 回到
  // 「任务」。待办请求必须压过记忆值。
  it('lets a pending request beat the remembered tab', () => {
    expect(initialRightTab('tasks', true)).toBe('orchestration');
  });
});

describe('requestOrchestrationPanel', () => {
  it('leaves a one-shot flag that the next mount can read', () => {
    requestOrchestrationPanel();
    expect(consumeOrchestrationRequest()).toBe(true);
    expect(consumeOrchestrationRequest()).toBe(false);
  });

  it('starts with nothing pending', () => {
    expect(consumeOrchestrationRequest()).toBe(false);
  });

  // 测试跑在 node 环境（本仓库没装 jsdom），用一个 EventTarget 冒充 window
  // 就够：被测代码只用到 dispatchEvent / addEventListener。
  it('also fires the event for an already-mounted panel', () => {
    const fake = new EventTarget();
    (globalThis as { window?: unknown }).window = fake;
    const seen: string[] = [];
    const onOpen = () => {
      // 已挂载的面板在回调里把标记吃掉，免得它下次重新 mount 又跳过来。
      consumeOrchestrationRequest();
      seen.push('handled');
    };
    fake.addEventListener(OPEN_ORCHESTRATION_EVENT, onOpen);
    try {
      requestOrchestrationPanel();
    } finally {
      fake.removeEventListener(OPEN_ORCHESTRATION_EVENT, onOpen);
      delete (globalThis as { window?: unknown }).window;
    }

    expect(seen).toEqual(['handled']);
    expect(consumeOrchestrationRequest()).toBe(false);
  });

  // 没有 window（测试环境、SSR）时标记照样留下，只是没有事件可发。
  it('still records the request when there is no window', () => {
    expect((globalThis as { window?: unknown }).window).toBeUndefined();
    requestOrchestrationPanel();
    expect(consumeOrchestrationRequest()).toBe(true);
  });
});
