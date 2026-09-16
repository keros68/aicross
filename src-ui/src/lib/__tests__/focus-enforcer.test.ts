import { describe, expect, it } from 'vitest';
import { shouldEnforceTerminalFocus, type DialogRoot, type FocusCandidate } from '../focus-enforcer';

// 没有 jsdom，用最小替身：元素只需要 tagName 与 closest，根只需要 querySelector。
const el = (tagName: string, insideDialog = false): FocusCandidate => ({
  tagName,
  closest: (selector: string) => (insideDialog && selector === '[role="dialog"]' ? {} : null),
});

const root = (selectors: string[] = []): DialogRoot => ({
  querySelector: (selector: string) => (selectors.includes(selector) ? {} : null),
});

const MODAL = '[aria-modal="true"]';

describe('shouldEnforceTerminalFocus', () => {
  it('抢回焦点：焦点落在普通 div 上，页面没有弹窗', () => {
    expect(shouldEnforceTerminalFocus(el('DIV'), root())).toBe(true);
  });

  it('抢回焦点：焦点为空（落回 body）', () => {
    expect(shouldEnforceTerminalFocus(null, root())).toBe(true);
  });

  it('不抢：焦点在输入控件上', () => {
    expect(shouldEnforceTerminalFocus(el('INPUT'), root())).toBe(false);
    expect(shouldEnforceTerminalFocus(el('TEXTAREA'), root())).toBe(false);
  });

  it('不抢：焦点落在对话框内部的按钮上', () => {
    expect(shouldEnforceTerminalFocus(el('BUTTON', true), root())).toBe(false);
  });

  it('不抢：页面上有 modal 弹窗时，哪怕焦点在弹窗外', () => {
    expect(shouldEnforceTerminalFocus(el('DIV'), root([MODAL]))).toBe(false);
  });

  it('非 modal 的侧边对话框不影响外部点击的抢回', () => {
    expect(shouldEnforceTerminalFocus(el('DIV'), root(['[role="dialog"]']))).toBe(true);
  });
});
