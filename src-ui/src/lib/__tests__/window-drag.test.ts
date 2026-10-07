import { describe, expect, it } from 'vitest';
import { INTERACTIVE_SELECTOR, startsWindowDrag } from '../window-drag';

// 测试环境没有 DOM：用最小的节点链模拟 Element.closest，只认标签名与
// [role="button"] 两种选择器写法（INTERACTIVE_SELECTOR 只用到这两种）。
interface FakeNode {
  tag: string;
  role?: string;
  parent?: FakeNode;
}

function matches(node: FakeNode, selector: string): boolean {
  const roleMatch = selector.match(/^\[role="(.+)"\]$/);
  if (roleMatch) return node.role === roleMatch[1];
  return node.tag === selector;
}

function element(node: FakeNode): Pick<Element, 'closest'> {
  return {
    closest: (selectors: string) => {
      const list = selectors.split(',').map((item) => item.trim());
      for (let current: FakeNode | undefined = node; current; current = current.parent) {
        if (list.some((selector) => matches(current as FakeNode, selector))) return {} as Element;
      }
      return null;
    },
  } as Pick<Element, 'closest'>;
}

describe('页头拖动窗口（fix-49-r4）', () => {
  const header: FakeNode = { tag: 'header' };

  it('按钮、下拉、链接、输入框与按钮里的图标上按下不拖动', () => {
    const button: FakeNode = { tag: 'button', parent: header };
    expect(startsWindowDrag(element(button))).toBe(false);
    expect(startsWindowDrag(element({ tag: 'svg', parent: button }))).toBe(false);
    expect(startsWindowDrag(element({ tag: 'select', parent: header }))).toBe(false);
    expect(startsWindowDrag(element({ tag: 'a', parent: header }))).toBe(false);
    expect(startsWindowDrag(element({ tag: 'input', parent: header }))).toBe(false);
    expect(startsWindowDrag(element({ tag: 'div', role: 'button', parent: header }))).toBe(false);
  });

  it('标题文字、页面图标与空白处可拖动', () => {
    expect(startsWindowDrag(element({ tag: 'h1', parent: { tag: 'div', parent: header } }))).toBe(true);
    expect(startsWindowDrag(element({ tag: 'span', parent: header }))).toBe(true);
    expect(startsWindowDrag(element(header))).toBe(true);
  });

  it('没有目标时不拖动；选择器覆盖常见可点元素', () => {
    expect(startsWindowDrag(null)).toBe(false);
    for (const tag of ['button', 'a', 'input', 'select', 'textarea', 'label', '[role="button"]']) {
      expect(INTERACTIVE_SELECTOR.split(',').map((item) => item.trim())).toContain(tag);
    }
  });
});
