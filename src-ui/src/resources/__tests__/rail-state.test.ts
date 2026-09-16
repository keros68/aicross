import { beforeEach, describe, expect, it } from 'vitest';
import {
  emptyRailState,
  initialExpanded,
  parseRailState,
  readRailState,
  resetRailStateCache,
  toggleInList,
  writeRailState,
} from '../rail-state';

function fakeStorage(seed: Record<string, string> = {}) {
  const map = new Map(Object.entries(seed));
  return {
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => { map.set(key, value); },
    dump: () => Object.fromEntries(map),
  };
}

beforeEach(() => {
  resetRailStateCache();
});

describe('左栏折叠状态', () => {
  it('从没动过时按全展开：这是首次打开的样子', () => {
    expect(initialExpanded(emptyRailState(), ['claude', 'codex'])).toEqual(['claude', 'codex']);
  });

  it('用户折叠过就听用户的——哪怕一个都不展开', () => {
    // 这一格钉住用户报的缺陷：切到「工作区」再切回来，折叠不能被推翻。
    const state = { ...emptyRailState(), expanded: [] };
    expect(initialExpanded(state, ['claude', 'codex'])).toEqual([]);
  });

  it('只展开一部分时，不在清单里的 agent 被丢掉', () => {
    const state = { ...emptyRailState(), expanded: ['claude', '已经卸载的'] };
    expect(initialExpanded(state, ['claude', 'codex'])).toEqual(['claude']);
  });

  it('写进去再读出来是同一份（跨组件卸载靠这条）', () => {
    const storage = fakeStorage();
    writeRailState({ ...emptyRailState(), expanded: ['codex'], openUnverified: ['pi/zai'] }, storage);
    resetRailStateCache();
    const back = readRailState(storage);
    expect(back.expanded).toEqual(['codex']);
    expect(back.openUnverified).toEqual(['pi/zai']);
  });

  it('存储不可用时不抛错，只是掉持久化', () => {
    const broken = {
      getItem: () => { throw new Error('blocked'); },
      setItem: () => { throw new Error('blocked'); },
    };
    expect(() => writeRailState(emptyRailState(), broken)).not.toThrow();
    resetRailStateCache();
    expect(readRailState(broken)).toEqual(emptyRailState());
  });

  it('存坏了的 JSON 当没存过', () => {
    expect(parseRailState('{ 不是 json')).toEqual(emptyRailState());
    expect(parseRailState('null')).toEqual(emptyRailState());
    expect(parseRailState('{"expanded":[1,2,"codex"]}').expanded).toEqual(['codex']);
  });

  it('toggle 是加一个减一个', () => {
    expect(toggleInList([], 'a')).toEqual(['a']);
    expect(toggleInList(['a', 'b'], 'a')).toEqual(['b']);
  });
});
