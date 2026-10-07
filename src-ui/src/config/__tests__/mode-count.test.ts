import { describe, expect, it } from 'vitest';
import { canDecrease, canIncrease, decreaseConfirmText, seatsLostOnDecrease } from '../mode-count';
import type { ModeEntry, ModeSeatNode, Seat } from '../../tauri';

const seat: Seat = { agent: 'claude', provider: null, model: 'haiku', tier: '', thinking: '', fallback: null };

function node(role: string, withSeat: boolean): ModeSeatNode {
  return {
    role, visibility: '盲', write: '只读', seat: withSeat ? seat : null, effective: withSeat ? seat : null,
    source: withSeat ? '位置' : '未指定',
  };
}

describe('数量调节按钮（TASK-56）', () => {
  it('到下限时「−」不可点，到上限时「+」不可点', () => {
    expect(canDecrease({ value: 1, min: 1 })).toBe(false);
    expect(canDecrease({ value: 2, min: 1 })).toBe(true);
    expect(canIncrease({ value: 4, max: 4 })).toBe(false);
    expect(canIncrease({ value: 3, max: 4 })).toBe(true);
  });
});

describe('减少时需确认的判定与文字（TASK-56）', () => {
  // 盲审：1 个验收固定 + 4 个验证，第 3、4 个验证有人选。
  const blind: Pick<ModeEntry, 'stages'> = {
    stages: [[node('验收', true), node('验证', false), node('验证', false), node('验证', true), node('验证', true)]],
  };

  it('只数可调角色末尾被去掉的位置里有本模式人选的个数，不算固定的验收', () => {
    expect(seatsLostOnDecrease(blind, '验证', 3)).toBe(1);
    expect(seatsLostOnDecrease(blind, '验证', 2)).toBe(2);
    expect(seatsLostOnDecrease(blind, '验证', 4)).toBe(0);
  });

  it('末尾去掉的位置没有人选时为 0，不需确认', () => {
    const empty: Pick<ModeEntry, 'stages'> = { stages: [[node('执行', true), node('执行', false), node('执行', false)]] };
    expect(seatsLostOnDecrease(empty, '执行', 2)).toBe(0);
  });

  it('跨阶段按先后顺序数（经济模式第 3 阶段的验证）', () => {
    const economy: Pick<ModeEntry, 'stages'> = {
      stages: [[node('出方案', true)], [node('执行', true)], [node('验收', true), node('验证', true)]],
    };
    expect(seatsLostOnDecrease(economy, '验证', 0)).toBe(1);
  });

  it('确认文字', () => {
    expect(decreaseConfirmText('验证', 2, 1)).toBe('减少到 2 个验证，末尾 1 个人选将被清除');
  });
});
