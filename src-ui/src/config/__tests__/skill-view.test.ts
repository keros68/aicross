import { describe, expect, it } from 'vitest';
import {
  canUninstall,
  duplicatePaths,
  globalInstallAction,
  hasNoSeats,
  hostLocations,
  isWriteTarget,
  locationStatus,
  modesPrompt,
  recommendNotes,
  skillInstalled,
  skillOverview,
} from '../skill-view';
import type { ModeSeatNode, ResourcesOutput, SkillHostStatus, SkillLocation } from '../../tauri';

const BUNDLED = '1.21.0';

function loc(overrides: Partial<SkillLocation> = {}): SkillLocation {
  return { scope: 'global', path: 'C:\\Users\\u\\.agents\\skills\\ai-cross', kind: 'none', version: '', linkTarget: null, readByThisHost: true, ...overrides };
}

function hostStatus(host: string, locations: SkillLocation[], duplicate = false): SkillHostStatus {
  return { host, locations, duplicate, updateAvailable: false };
}

describe('skill 位置的状态文字（TASK-51）', () => {
  it('none / ours 最新 / ours 可更新 / external', () => {
    expect(locationStatus(loc(), BUNDLED)).toEqual({ text: '未安装', tone: 'muted' });
    expect(locationStatus(loc({ kind: 'ours', version: BUNDLED }), BUNDLED)).toEqual({ text: `已安装 · ${BUNDLED}`, tone: 'ok' });
    expect(locationStatus(loc({ kind: 'ours', version: '1.20.0' }), BUNDLED)).toEqual({ text: '可更新', tone: 'alert' });
    expect(locationStatus(loc({ kind: 'external', version: '1.19.0' }), BUNDLED)).toEqual({ text: '由其他软件管理 · 1.19.0', tone: 'plain' });
    expect(locationStatus(loc({ kind: 'external' }), BUNDLED)).toEqual({ text: '由其他软件管理', tone: 'plain' });
  });

  it('内置版本未知时我们装的不算可更新', () => {
    expect(locationStatus(loc({ kind: 'ours', version: '1.0.0' }), '')).toEqual({ text: '已安装 · 1.0.0', tone: 'ok' });
  });

  it('只有我们装的能卸载', () => {
    expect(canUninstall('codex', loc({ kind: 'ours' }))).toBe(true);
    expect(canUninstall('codex', loc({ kind: 'external' }))).toBe(false);
    expect(canUninstall('codex', loc())).toBe(false);
  });

  it('两处都有我们的安装标记时，只有写入位置能卸载（卸载命令删的是写入位置）', () => {
    const old = loc({ path: '/home/u/.codex/skills/ai-cross', kind: 'ours', version: BUNDLED, writeTarget: false });
    const target = loc({ path: '/home/u/.agents/skills/ai-cross', kind: 'ours', version: BUNDLED, writeTarget: true });
    expect(canUninstall('codex', old)).toBe(false);
    expect(canUninstall('codex', target)).toBe(true);
    // 旧版引擎缺 writeTarget 时按路径判断。
    expect(canUninstall('codex', { ...old, writeTarget: undefined })).toBe(false);
    expect(canUninstall('codex', { ...target, writeTarget: undefined })).toBe(true);
  });
});

describe('安装到全局按钮（TASK-51）', () => {
  const claudeTarget = loc({ path: 'C:\\Users\\u\\.claude\\skills\\ai-cross' });

  it('写入位置以 writeTarget 字段为准，不看路径', () => {
    expect(isWriteTarget('claude', loc({ writeTarget: true }))).toBe(true);
    expect(isWriteTarget('claude', { ...claudeTarget, writeTarget: false })).toBe(false);
    expect(isWriteTarget('codex', loc({ path: '/home/u/.codex/skills/ai-cross', writeTarget: true }))).toBe(true);
  });

  it('旧版引擎缺 writeTarget 时退回路径：claude 写 .claude/skills，其余写 .agents/skills', () => {
    expect(isWriteTarget('claude', claudeTarget)).toBe(true);
    expect(isWriteTarget('claude', loc())).toBe(false);
    expect(isWriteTarget('codex', loc())).toBe(true);
    expect(isWriteTarget('codex', loc({ path: '/home/u/.codex/skills/ai-cross' }))).toBe(false);
  });

  it('按钮跟随 writeTarget：标为写入位置的那处决定状态', () => {
    const status = hostStatus('codex', [
      loc({ path: '/home/u/.agents/skills/ai-cross', kind: 'external', writeTarget: false }),
      loc({ path: '/home/u/custom/ai-cross', kind: 'ours', version: BUNDLED, writeTarget: true }),
    ]);
    expect(globalInstallAction(status, BUNDLED)).toEqual({ kind: 'installed', label: '已安装', disabled: true });
  });

  it('none 安装；ours 最新为禁用的「已安装」；ours 可更新为「更新」；external 禁用', () => {
    expect(globalInstallAction(hostStatus('claude', [claudeTarget]), BUNDLED)).toEqual({ kind: 'install', label: '安装到全局', disabled: false });
    expect(globalInstallAction(hostStatus('claude', [{ ...claudeTarget, kind: 'ours', version: BUNDLED }]), BUNDLED))
      .toEqual({ kind: 'installed', label: '已安装', disabled: true });
    expect(globalInstallAction(hostStatus('claude', [{ ...claudeTarget, kind: 'ours', version: '1.0.0' }]), BUNDLED))
      .toEqual({ kind: 'update', label: '更新', disabled: false });
    expect(globalInstallAction(hostStatus('claude', [{ ...claudeTarget, kind: 'external' }]), BUNDLED))
      .toEqual({ kind: 'external', label: '由其他软件管理', disabled: true });
  });

  it('只看写入位置：codex 的旧位置已装不影响按钮；状态还没读到时可安装', () => {
    const status = hostStatus('codex', [
      loc({ path: '/home/u/.codex/skills/ai-cross', kind: 'external', version: '1.0.0' }),
      loc({ path: '/home/u/.agents/skills/ai-cross' }),
    ]);
    expect(globalInstallAction(status, BUNDLED)).toEqual({ kind: 'install', label: '安装到全局', disabled: false });
    expect(globalInstallAction(null, BUNDLED)).toEqual({ kind: 'install', label: '安装到全局', disabled: false });
  });
});

describe('重复安装与概览（TASK-51）', () => {
  const both = hostStatus('codex', [
    loc({ path: '/home/u/.codex/skills/ai-cross', kind: 'external', version: '1.19.0', linkTarget: '/home/u/.skills-manager/skills/ai-cross' }),
    loc({ path: '/home/u/.agents/skills/ai-cross', kind: 'ours', version: '1.20.0' }),
    loc({ scope: 'project', path: '/p/.agents/skills/ai-cross' }),
  ], true);

  it('duplicate 为 true 时列出读得到的各处路径；否则为空', () => {
    expect(duplicatePaths(both)).toEqual(['/home/u/.codex/skills/ai-cross', '/home/u/.agents/skills/ai-cross']);
    expect(duplicatePaths({ ...both, duplicate: false })).toEqual([]);
    expect(duplicatePaths(null)).toEqual([]);
  });

  it('概览：内置版本、已安装位置、待处理（重复 + 可更新），待处理非 0 标异常', () => {
    expect(skillOverview({ bundledVersion: BUNDLED, hosts: [both] }, both)).toEqual([
      { label: '内置版本', value: BUNDLED, alert: false },
      { label: '已安装位置', value: '2', alert: false },
      { label: '待处理', value: '2', alert: true },
    ]);
    expect(skillOverview(null, null)).toEqual([
      { label: '内置版本', value: '未知', alert: false },
      { label: '已安装位置', value: '0', alert: false },
      { label: '待处理', value: '0', alert: false },
    ]);
  });

  it('位置只列当前主 agent 读得到的，全局在前', () => {
    const status = hostStatus('pi', [
      loc({ scope: 'project', path: '/p/.pi/skills/ai-cross' }),
      loc({ path: '/x', readByThisHost: false }),
      loc({ path: '/home/u/.pi/agent/skills/ai-cross' }),
    ]);
    expect(hostLocations(status).map((item) => item.path)).toEqual(['/home/u/.pi/agent/skills/ai-cross', '/p/.pi/skills/ai-cross']);
  });
});

describe('首次使用提示（TASK-51）', () => {
  const detected: ResourcesOutput = { agents: [], detected_at: '2026-10-06T08:00:00Z' };
  const seatNode = (seat: ModeSeatNode['seat']): ModeSeatNode => ({
    role: '验收', visibility: '盲', write: '只读', seat, effective: seat, source: seat ? '位置' : '未指定',
  });
  const emptyModes = [{ stages: [[seatNode(null)]] }];
  const configured = [{ stages: [[seatNode({ agent: 'claude', provider: null, model: 'haiku', tier: '', thinking: '', fallback: null })]] }];
  const installed = hostStatus('claude', [loc({ kind: 'ours', version: BUNDLED })]);
  const notInstalled = hostStatus('claude', [loc()]);

  it('资源未探测时先提示去探测', () => {
    expect(modesPrompt({ resources: { agents: [] }, skillHost: installed, skillLoaded: true, modes: configured })).toBe('detect');
  });

  it('探测过、skill 没装时提示去安装；skill 状态没读到时不提示', () => {
    expect(modesPrompt({ resources: detected, skillHost: notInstalled, skillLoaded: true, modes: configured })).toBe('install');
    expect(modesPrompt({ resources: detected, skillHost: null, skillLoaded: true, modes: configured })).toBe('install');
    expect(modesPrompt({ resources: detected, skillHost: null, skillLoaded: false, modes: configured })).toBeNull();
  });

  it('skill 装好、所有模式都没人选时提示一键推荐；有人选时不提示', () => {
    expect(modesPrompt({ resources: detected, skillHost: installed, skillLoaded: true, modes: emptyModes })).toBe('recommend');
    expect(modesPrompt({ resources: detected, skillHost: installed, skillLoaded: true, modes: configured })).toBeNull();
  });

  it('其他软件管的也算装好；没有模式时不算「没人选」', () => {
    expect(skillInstalled(hostStatus('claude', [loc({ kind: 'external' })]))).toBe(true);
    expect(hasNoSeats([])).toBe(false);
    expect(hasNoSeats(emptyModes)).toBe(true);
  });
});

describe('推荐说明（TASK-51）', () => {
  it('数组或多行字符串都按条列出，去掉空条', () => {
    expect(recommendNotes(['经济模式 验收：没有满足条件的人选', ' '])).toEqual(['经济模式 验收：没有满足条件的人选']);
    expect(recommendNotes('a\n\nb')).toEqual(['a', 'b']);
    expect(recommendNotes(undefined)).toEqual([]);
  });
});
