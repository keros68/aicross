import { describe, expect, it } from 'vitest';
import {
  HOST_STORAGE_KEY,
  KNOWN_VENDORS,
  readHost,
  vendorOptions,
  vendorSelectValue,
  vendorText,
  writeHost,
} from '../host-select';
import type { HostEntry } from '../../tauri';

function memoryStorage(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return {
    data,
    getItem: (key: string) => (key in data ? data[key] : null),
    setItem: (key: string, value: string) => { data[key] = value; },
  };
}

const throwing = {
  getItem: () => { throw new Error('denied'); },
  setItem: () => { throw new Error('denied'); },
};

function host(overrides: Partial<HostEntry> = {}): HostEntry {
  return { id: 'claude', name: 'Claude Code', vendor: 'Anthropic', vendorSource: '推定', vendorBasis: '', ...overrides };
}

const hosts = [host(), host({ id: 'codex', name: 'Codex', vendor: 'OpenAI' }), host({ id: 'pi', name: 'pi agent', vendor: '未知', vendorSource: '未知' })];

describe('主 agent 列表与选中记忆（TASK-51）', () => {
  it('首次没有记录时选第一个；列表为空时为空串', () => {
    expect(readHost(memoryStorage(), hosts)).toBe('claude');
    expect(readHost(memoryStorage(), [])).toBe('');
  });

  it('写入后读回同一个', () => {
    const storage = memoryStorage();
    writeHost(storage, 'codex');
    expect(storage.data[HOST_STORAGE_KEY]).toBe('codex');
    expect(readHost(storage, hosts)).toBe('codex');
  });

  it('记住的已不在列表里时回到第一个', () => {
    expect(readHost(memoryStorage({ [HOST_STORAGE_KEY]: 'kimi' }), hosts)).toBe('claude');
  });

  it('存储读写抛错时不向外抛', () => {
    expect(readHost(throwing, hosts)).toBe('claude');
    expect(() => writeHost(throwing, 'pi')).not.toThrow();
    expect(() => writeHost(null, 'pi')).not.toThrow();
  });
});

describe('实际厂商的显示（TASK-51）', () => {
  it('已知厂商常规色，「未知」与空串用异常色', () => {
    expect(vendorText(hosts[0])).toEqual({ text: 'Anthropic', alert: false });
    expect(vendorText(hosts[2])).toEqual({ text: '未知', alert: true });
    expect(vendorText(host({ vendor: '' }))).toEqual({ text: '未知', alert: true });
  });

  it('下拉：自动推定在首位，其后是各已知厂商；只有用户设定时当前值才是厂商', () => {
    const options = vendorOptions(hosts[0]);
    expect(options[0]).toEqual({ value: '', label: '自动推定' });
    expect(options.slice(1).map((option) => option.value)).toEqual([...KNOWN_VENDORS]);
    expect(vendorSelectValue(hosts[0])).toBe('');
    expect(vendorSelectValue(host({ vendor: 'Zhipu', vendorSource: '用户设定' }))).toBe('Zhipu');
  });

  it('用户设定的厂商不在已知表里时补在末尾', () => {
    const options = vendorOptions(host({ vendor: '自建网关', vendorSource: '用户设定' }));
    expect(options.at(-1)).toEqual({ value: '自建网关', label: '自建网关' });
  });
});
