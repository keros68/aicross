import { describe, expect, it } from 'vitest';
import {
  groupPickableModels,
  modelGroupLabel,
  modelKey,
  modelOptionLabel,
  modelSelectValue,
  modelStatusSuffix,
  parseModelSelectValue,
  targetSelectValue,
  type PickableModel,
} from '../model-picker';

function model(partial: Partial<PickableModel>): PickableModel {
  return { id: '', agent: '', provider: '', status: '已接入', ...partial };
}

describe('modelGroupLabel', () => {
  it('一家一个源的 agent 就用 agent id 当标签', () => {
    expect(modelGroupLabel(model({ agent: 'codex', id: 'gpt-5.6-luna' }))).toBe('codex');
  });

  it('带 provider 的写成 agent · provider（pi 一家挂多个 provider）', () => {
    expect(modelGroupLabel(model({ agent: 'pi', provider: 'zai-coding-cn' }))).toBe(
      'pi · zai-coding-cn',
    );
  });

  it('provider 只有空白算没有', () => {
    expect(modelGroupLabel(model({ agent: 'pi', provider: '   ' }))).toBe('pi');
  });
});

describe('modelStatusSuffix / modelOptionLabel', () => {
  it('已接入不加后缀', () => {
    const m = model({ agent: 'claude', id: 'fable', status: '已接入' });
    expect(modelStatusSuffix(m)).toBe('');
    expect(modelOptionLabel(m)).toBe('claude/fable');
  });

  it('待冒烟与冒烟失败都点明状态', () => {
    expect(modelOptionLabel(model({ agent: 'pi', id: 'glm-4.6', status: '待冒烟' }))).toBe(
      'pi/glm-4.6（待冒烟）',
    );
    expect(modelOptionLabel(model({ agent: 'pi', id: 'glm-4.6', status: '冒烟失败' }))).toBe(
      'pi/glm-4.6（冒烟失败）',
    );
  });

  it('status 为空（老 store 没写过这个字段）当作不加后缀，不显示空括号', () => {
    expect(modelOptionLabel(model({ agent: 'kimi', id: 'kimi-code/k3', status: '' }))).toBe(
      'kimi/kimi-code/k3',
    );
  });
});

describe('groupPickableModels', () => {
  const models = [
    model({ agent: 'codex', id: 'gpt-5.6-luna' }),
    model({ agent: 'pi', provider: 'zai-coding-cn', id: 'glm-5.3-flash' }),
    model({ agent: 'pi', provider: 'zai-coding-cn', id: 'glm-4.6', status: '待冒烟' }),
    model({ agent: 'pi', provider: 'moonshot', id: 'kimi-k2', status: '待冒烟' }),
  ];

  it('按 agent · provider 归组，同一 agent 的不同 provider 各成一组', () => {
    expect(groupPickableModels(models).map((g) => g.label)).toEqual([
      'codex',
      'pi · zai-coding-cn',
      'pi · moonshot',
    ]);
  });

  it('保持传入顺序，不二次排序（后端已按状态 + agent + provider + id 排好）', () => {
    const zai = groupPickableModels(models).find((g) => g.label === 'pi · zai-coding-cn');
    expect(zai?.models.map((m) => m.id)).toEqual(['glm-5.3-flash', 'glm-4.6']);
  });

  it('空清单返回空数组', () => {
    expect(groupPickableModels([])).toEqual([]);
  });
});

describe('modelKey', () => {
  it('带上 provider，两个 provider 下的同名模型不会撞 React key', () => {
    const a = model({ agent: 'pi', provider: 'zai-coding-cn', id: 'glm-4.6' });
    const b = model({ agent: 'pi', provider: 'other', id: 'glm-4.6' });
    expect(modelKey(a)).not.toBe(modelKey(b));
  });
});

// TargetEditor 的模型下拉：value 必须能区分 provider，选中后 provider 要一起写回
// Target。pi 一家挂三个 provider，写不回去的话派发 argv 的 --provider 段是空的。
describe('模型下拉 value 往 Target 的写回', () => {
  type TargetLike = { agent: string; provider?: string; model: string; role: string };
  const blank: TargetLike = { agent: '', provider: undefined, model: '', role: '执行' };

  /** 模拟 TargetEditor 的 onChange：value → patch → 新 Target。 */
  function select(target: TargetLike, value: string): TargetLike {
    const picked = parseModelSelectValue(value);
    return picked ? { ...target, ...picked } : target;
  }

  it('选中 pi 的某个 provider 模型后，target.provider 等于该 provider', () => {
    const zai = model({ agent: 'pi', provider: 'zai-coding-cn', id: 'glm-5.3-flash' });
    const next = select(blank, modelSelectValue(zai));
    expect(next).toMatchObject({
      agent: 'pi',
      provider: 'zai-coding-cn',
      model: 'glm-5.3-flash',
    });
    expect(next.role).toBe('执行'); // 其它字段不动
  });

  it('同 id 不同 provider 的两条不混：value 不同，写回的 provider 也不同', () => {
    const zai = model({ agent: 'pi', provider: 'zai-coding-cn', id: 'glm-4.6' });
    const qwen = model({ agent: 'pi', provider: 'qwen-token-plan-cn', id: 'glm-4.6' });
    expect(modelSelectValue(zai)).not.toBe(modelSelectValue(qwen));
    expect(select(blank, modelSelectValue(zai)).provider).toBe('zai-coding-cn');
    expect(select(blank, modelSelectValue(qwen)).provider).toBe('qwen-token-plan-cn');
  });

  it('换 provider 时旧 provider 被覆盖，不残留', () => {
    const before = select(
      blank,
      modelSelectValue(model({ agent: 'pi', provider: 'zai-coding-cn', id: 'glm-4.6' })),
    );
    const after = select(
      before,
      modelSelectValue(model({ agent: 'pi', provider: 'deepseek', id: 'deepseek-flash' })),
    );
    expect(after.provider).toBe('deepseek');
    expect(after.model).toBe('deepseek-flash');
  });

  it('没有 provider 的 agent 写回 undefined（Target 里「留空按通道默认」的语义）', () => {
    const next = select(blank, modelSelectValue(model({ agent: 'claude', id: 'fable' })));
    expect(next.provider).toBeUndefined();
  });

  it('模型 id 自带斜杠也能原样切回来（kimi 的 kimi-code/k3）', () => {
    const kimi = model({ agent: 'kimi', id: 'kimi-code/k3-256k' });
    expect(select(blank, modelSelectValue(kimi))).toMatchObject({
      agent: 'kimi',
      model: 'kimi-code/k3-256k',
    });
  });

  it('targetSelectValue 能回选下拉：同一模型来回一趟 value 一致', () => {
    const m = model({ agent: 'pi', provider: 'zai-coding-cn', id: 'glm-4.6' });
    const target = select(blank, modelSelectValue(m));
    expect(targetSelectValue(target)).toBe(modelSelectValue(m));
  });

  it('老图的 Target 没有 provider 时对不上清单里带 provider 的那条（所以要挂「当前值」选项）', () => {
    const legacy: TargetLike = { agent: 'pi', model: 'glm-5.3-flash', role: '执行' };
    const listed = model({ agent: 'pi', provider: 'zai-coding-cn', id: 'glm-5.3-flash' });
    expect(targetSelectValue(legacy)).not.toBe(modelSelectValue(listed));
  });

  it('解析不出三元组就返回 null，调用侧原样不动', () => {
    for (const bad of ['', '__manual', 'pi/glm-4.6', '["pi","zai"]', '{"agent":"pi"}', '[1,2,3]']) {
      expect(parseModelSelectValue(bad)).toBeNull();
    }
    expect(select(blank, '__manual')).toEqual(blank);
  });
});
