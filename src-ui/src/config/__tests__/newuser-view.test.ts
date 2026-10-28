import { describe, expect, it } from 'vitest';
import {
  batchSmokeButton,
  batchSmokeLabel,
  batchSmokeTargets,
  detectButtonLabel,
  optionStateText,
  seatRoleText,
  slashRowAction,
  unverifiedLines,
  versionText,
} from '../newuser-view';
import { agentStats, stateTone } from '../config-view';
import { newStageNode } from '../preset-edit';
import { autoUpdateHosts, canUpgradeHere, outdatedText, staleExternal } from '../skill-view';
import type { ResourceModelEntry, SkillLocation } from '../../tauri';

function model(id: string, provider: string, state: string): ResourceModelEntry {
  return { id, agent: 'pi', provider, resolved_id: '', state, note: '' };
}

describe('资源页初始态（TASK-58 #1、#2）', () => {
  it('「未探测」是中性灰标签，不是异常', () => {
    expect(stateTone('未探测')).toBe('muted');
    expect(stateTone('未安装')).toBe('alert');
    expect(agentStats({ models: [], state: '未探测' })).toEqual([
      { text: '0 个模型', alert: false },
      { text: '未探测', alert: false },
    ]);
  });

  it('版本「未检测」或为空时不显示', () => {
    expect(versionText('未检测')).toBe('');
    expect(versionText(' ')).toBe('');
    expect(versionText('2.1.270')).toBe('2.1.270');
  });

  it('主按钮：从未探测为「探测」，探测过为「重新探测」', () => {
    expect(detectButtonLabel({ detecting: false, progress: null, everDetected: false })).toBe('探测');
    expect(detectButtonLabel({ detecting: false, progress: null, everDetected: true })).toBe('重新探测');
  });
});

describe('探测进度（TASK-58 #3）', () => {
  it('有进度事件时「探测中 2/4」，没有时「探测中…」', () => {
    expect(detectButtonLabel({ detecting: true, progress: { done: 2, total: 4 }, everDetected: false })).toBe('探测中 2/4');
    expect(detectButtonLabel({ detecting: true, progress: null, everDetected: true })).toBe('探测中…');
    expect(detectButtonLabel({ detecting: true, progress: { done: 0, total: 0 }, everDetected: true })).toBe('探测中…');
  });
});

describe('全部测试（TASK-58 #4）', () => {
  it('目标是未验证、验证失败、连接超时的第三方模型，原生与其他状态不算', () => {
    const targets = batchSmokeTargets({
      models: [
        model('a', 'zai', '未验证'),
        model('b', '', '未验证'),
        model('c', 'zai', '已验证'),
        model('d', 'qwen', '验证失败'),
        model('e', 'opencode-go', '未验证'),
        model('f', 'anthropic', '冒烟超时'),
        model('g', 'zai', '额度用尽'),
      ],
    });
    expect(targets.map((item) => item.id)).toEqual(['a', 'd', 'e', 'f']);
  });

  it('有第三方模型就显示按钮；没有可测的置灰并说明原因（TASK-66）', () => {
    expect(batchSmokeButton({ state: '可用', models: [model('a', '', '可用')] })).toEqual({ show: false, reason: '' });
    expect(batchSmokeButton({ state: '可用', models: [model('a', 'zai', '验证失败')] })).toEqual({ show: true, reason: '' });
    expect(batchSmokeButton({ state: '可用', models: [model('a', 'zai', '已验证')] })).toEqual({ show: true, reason: '没有待测的模型' });
    expect(batchSmokeButton({ state: '未安装', models: [model('a', 'zai', '未安装')] })).toEqual({ show: true, reason: '未安装，无法测试' });
  });

  it('按钮文字：未运行「全部测试」，运行中「测试中 12/33」', () => {
    expect(batchSmokeLabel(null)).toBe('全部测试');
    expect(batchSmokeLabel({ done: 12, total: 33 })).toBe('测试中 12/33');
  });
});

describe('推荐出路（TASK-58 #5）', () => {
  it('列出各 agent 未验证的模型数，0 的不列；字段缺失为空', () => {
    expect(unverifiedLines([{ agent: 'pi', count: 33 }, { agent: 'codex', count: 0 }])).toEqual([
      { agent: 'pi', text: 'pi 有 33 个模型未验证' },
    ]);
    expect(unverifiedLines(undefined)).toEqual([]);
  });
});

describe('评审视角（TASK-58 #7）', () => {
  it('有视角时「验证 · 安全」，空或缺失时只写角色', () => {
    expect(seatRoleText('验证', '安全')).toBe('验证 · 安全');
    expect(seatRoleText('验证', '')).toBe('验证');
    expect(seatRoleText('执行', undefined)).toBe('执行');
  });
});

describe('skill 页（TASK-58 #10、#11）', () => {
  const loc = (overrides: Partial<SkillLocation>): SkillLocation => ({
    scope: 'global', path: 'C:\\Users\\u\\.agents\\skills\\ai-cross', kind: 'external', version: '1.21.0', ...overrides,
  });

  it('其他软件装的且低于内置时标「需要升级到 <内置版本>」，其余不标', () => {
    expect(outdatedText(loc({ outdated: true }), '1.21.1')).toBe('需要升级到 1.21.1');
    expect(outdatedText(loc({ outdated: true }), '')).toBe('需要升级');
    expect(outdatedText(loc({ outdated: false }), '1.21.1')).toBe('');
    expect(outdatedText(loc({}), '1.21.1')).toBe('');
    expect(outdatedText(loc({ kind: 'ours', outdated: true }), '1.21.1')).toBe('');
  });
});

describe('新建模式编辑器（TASK-58 #12）', () => {
  it('新增阶段默认「验证 · 盲」', () => {
    expect(newStageNode()).toEqual({ role: '验证', visibility: '盲', write: '只读', thinking: '' });
  });
});

describe('模型下拉（TASK-58 #13）', () => {
  it('可用、已验证不写状态，其余照写', () => {
    expect(optionStateText('可用')).toBe('');
    expect(optionStateText('已验证')).toBe('');
    expect(optionStateText('未验证')).toBe('未验证');
    expect(optionStateText('未发现')).toBe('未发现');
  });
});

describe('已存在的命令文件（TASK-58 #14）', () => {
  const base = { suggestedName: 'ax-blind', command: null };

  it('已登记的显示命令', () => {
    const command = { name: 'ax-blind', mode: 'builtin-blind', modeName: '盲审模式', note: '', path: '', state: '已写入', trigger: '/ax-blind' };
    expect(slashRowAction({ ...base, command })).toEqual({ kind: 'command' });
  });

  it('文件已存在且是我们写的：「登记」，名称取已有文件', () => {
    expect(slashRowAction({ ...base, existing: { name: 'ax-blind-old', state: '已写入', path: '/p' } }))
      .toEqual({ kind: 'register', name: 'ax-blind-old' });
  });

  it('被占用：不给按钮', () => {
    expect(slashRowAction({ ...base, existing: { name: 'ax-blind', state: '被占用', path: '/p' } })).toEqual({ kind: 'occupied' });
  });

  it('都没有（含旧版引擎没有 existing）：「生成」建议名', () => {
    expect(slashRowAction(base)).toEqual({ kind: 'generate', name: 'ax-blind' });
    expect(slashRowAction({ ...base, existing: null })).toEqual({ kind: 'generate', name: 'ax-blind' });
  });
});

describe('skill 升级', () => {
  const external = (overrides: Partial<SkillLocation>): SkillLocation => ({
    scope: 'global', path: 'C:/u/.codex/skills/ai-cross', kind: 'external', version: '1.12.1',
    outdated: true, manager: '', linkTarget: null, readByThisHost: true, writeTarget: false, ...overrides,
  });

  it('主 agent 读得到的别人装的旧版要升级；skills-manager 管理的不在这里替换', () => {
    const host = { host: 'codex', locations: [external({})], duplicate: false, updateAvailable: false };
    expect(staleExternal(host)?.version).toBe('1.12.1');
    expect(staleExternal({ ...host, locations: [external({ outdated: false })] })).toBeNull();
    expect(canUpgradeHere(external({}))).toBe(true);
    expect(canUpgradeHere(external({ manager: 'skills-manager' }))).toBe(false);
  });

  it('启动时只自动更新我们装在全局写入位置的旧版', () => {
    const ours = (version: string): SkillLocation => external({ kind: 'ours', version, outdated: false, writeTarget: true, path: 'C:/u/.claude/skills/ai-cross' });
    const status = {
      bundledVersion: '1.23.0',
      hosts: [
        { host: 'claude', locations: [ours('1.22.0')], duplicate: false, updateAvailable: true },
        { host: 'codex', locations: [external({})], duplicate: false, updateAvailable: false },
        { host: 'kimi', locations: [ours('1.23.0')], duplicate: false, updateAvailable: false },
      ],
    };
    expect(autoUpdateHosts(status)).toEqual(['claude']);
    expect(autoUpdateHosts({ ...status, bundledVersion: '' })).toEqual([]);
  });
});
