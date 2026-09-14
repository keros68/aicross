import { describe, expect, it } from 'vitest';
import { stampMenuShouldClose } from '../stamp-menu';

describe('stamp menu', () => {
  it('R5-4 盖预设菜单响应 Escape 与外点', () => {
    expect(stampMenuShouldClose({ type: 'keydown', key: 'Escape' } as KeyboardEvent)).toBe(true);
    expect(stampMenuShouldClose({ type: 'keydown', key: 'Enter' } as KeyboardEvent)).toBe(false);
    expect(stampMenuShouldClose({ type: 'mousedown', target: null } as MouseEvent)).toBe(true);
    expect(stampMenuShouldClose({ type: 'mousedown', target: { closest: () => ({}) } } as unknown as MouseEvent)).toBe(false);
  });
});
