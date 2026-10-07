// skill 页与首次使用提示的展示纯函数（TASK-51）。数据契约在 ../tauri.ts
// （镜像 `aicross skill status`）；页面组件不掺逻辑，测试落在这层。

import type { ModeEntry, ResourcesOutput, SkillHostStatus, SkillLocation, SkillScope, SkillStatus } from '../tauri';
import type { StatItem } from './config-view';
import { needsFirstDetect } from './config-view';

export type LocationTone = 'muted' | 'ok' | 'alert' | 'plain';

/** 外部位置由谁管理（TASK-60）：skills.sh、skills-manager 写出名字，其余为其他软件。 */
export function managerText(manager: string | null | undefined): string {
  const value = (manager ?? '').trim();
  return value === 'skills.sh' || value === 'skills-manager' ? `由 ${value} 管理` : '由其他软件管理';
}

/** 位置行的状态胶囊：未安装 / 已安装 · 版本 / 可更新 / 由 <管理软件> 管理 · 版本。 */
export function locationStatus(location: Pick<SkillLocation, 'kind' | 'version' | 'manager'>, bundledVersion: string): { text: string; tone: LocationTone } {
  const version = location.version.trim();
  if (location.kind === 'none') return { text: '未安装', tone: 'muted' };
  if (location.kind === 'external') {
    const who = managerText(location.manager);
    return { text: version === '' ? who : `${who} · ${version}`, tone: 'plain' };
  }
  if (isOutdated(location, bundledVersion)) return { text: '可更新', tone: 'alert' };
  return { text: version === '' ? '已安装' : `已安装 · ${version}`, tone: 'ok' };
}

/** 我们装的、版本与内置包不同的算可更新；内置版本未知时不算。 */
export function isOutdated(location: Pick<SkillLocation, 'kind' | 'version'>, bundledVersion: string): boolean {
  const bundled = bundledVersion.trim();
  return location.kind === 'ours' && bundled !== '' && location.version.trim() !== bundled;
}

function normalizePath(path: string): string {
  return path.replace(/\\/g, '/').toLowerCase();
}

/** 是否是我们安装时写入的位置：以 skill_status 的 writeTarget 为准；旧版引擎没有
 *  这个字段时退回按路径判断（claude 写 .claude/skills，其余写 .agents/skills）。 */
export function isWriteTarget(host: string, location: Pick<SkillLocation, 'path' | 'writeTarget'>): boolean {
  if (typeof location.writeTarget === 'boolean') return location.writeTarget;
  const dir = host === 'claude' ? '/.claude/skills/' : '/.agents/skills/';
  return normalizePath(location.path).includes(dir);
}

export interface InstallAction {
  /** install / update 时页头是主按钮；installed / external 时只显示状态标签（TASK-58）。 */
  kind: 'install' | 'update' | 'installed' | 'external';
  label: string;
  disabled: boolean;
}

/** 页头（全局范围）：未安装为主按钮「安装到全局」；可更新为「更新」；我们装的且最新
 *  显示标签「已安装」；目标位置由其他软件管理时显示标签「由 <管理软件> 管理」。 */
export function globalInstallAction(hostStatus: SkillHostStatus | null | undefined, bundledVersion: string): InstallAction {
  const target = hostStatus?.locations.find((location) => location.scope === 'global' && isWriteTarget(hostStatus.host, location));
  if (!target || target.kind === 'none') return { kind: 'install', label: '安装到全局', disabled: false };
  if (target.kind === 'external') return { kind: 'external', label: managerText(target.manager), disabled: true };
  if (isOutdated(target, bundledVersion)) return { kind: 'update', label: '更新', disabled: false };
  return { kind: 'installed', label: '已安装', disabled: true };
}

/** 其他软件装的、版本低于内置时的标签（引擎的 outdated 字段）；否则为空串。 */
export function outdatedText(location: Pick<SkillLocation, 'kind' | 'outdated'>, bundledVersion: string): string {
  if (location.kind !== 'external' || location.outdated !== true) return '';
  const bundled = bundledVersion.trim();
  return bundled === '' ? '旧于内置' : `旧于内置 ${bundled}`;
}

/** 位置行能否卸载：只有我们装的。 */
export function canUninstall(location: Pick<SkillLocation, 'kind'>): boolean {
  return location.kind === 'ours';
}

/** skill 页概览：内置版本 · 已安装位置 · 待处理（重复安装 + 可更新）。 */
export function skillOverview(status: SkillStatus | null | undefined, hostStatus: SkillHostStatus | null | undefined): StatItem[] {
  const bundled = status?.bundledVersion.trim() ?? '';
  const locations = hostStatus?.locations ?? [];
  const installed = locations.filter((location) => location.kind !== 'none').length;
  const outdated = locations.filter((location) => isOutdated(location, bundled)).length;
  const attention = (hostStatus?.duplicate ? 1 : 0) + outdated;
  return [
    { label: '内置版本', value: bundled === '' ? '未知' : bundled, alert: false },
    { label: '已安装位置', value: String(installed), alert: false },
    { label: '待处理', value: String(attention), alert: attention > 0 },
  ];
}

/** 重复安装时列出读得到的各处路径；不重复时为空。 */
export function duplicatePaths(hostStatus: SkillHostStatus | null | undefined): string[] {
  if (!hostStatus?.duplicate) return [];
  return hostStatus.locations.filter((location) => location.kind !== 'none').map((location) => location.path);
}

/** 当前主 agent 读得到的位置（readByThisHost 缺省视为读得到），全局在前。 */
export function hostLocations(hostStatus: SkillHostStatus | null | undefined): SkillLocation[] {
  const locations = (hostStatus?.locations ?? []).filter((location) => location.readByThisHost !== false);
  const order: Record<SkillScope, number> = { global: 0, project: 1 };
  return [...locations].sort((a, b) => order[a.scope] - order[b.scope]);
}

export function scopeText(scope: SkillScope): string {
  return scope === 'global' ? '全局' : '项目';
}

export function findHostStatus(status: SkillStatus | null | undefined, host: string): SkillHostStatus | null {
  return status?.hosts.find((item) => item.host === host) ?? null;
}

/** 当前主 agent 读得到的位置里有没有 ai-cross（我们装的或其他软件管的都算）。 */
export function skillInstalled(hostStatus: SkillHostStatus | null | undefined): boolean {
  return (hostStatus?.locations ?? []).some((location) => location.kind !== 'none');
}

// ── 首次使用提示 ─────────────────────────────────────────────────────

/** 当前主 agent 的所有模式都没有本模式人选。没有模式时不算。 */
export function hasNoSeats(modes: Pick<ModeEntry, 'stages'>[]): boolean {
  return modes.length > 0 && modes.every((mode) => mode.stages.every((stage) => stage.every((node) => node.seat === null)));
}

export type FirstUsePrompt = 'detect' | 'install' | 'recommend' | null;

/** 按「资源 → 主 agent → skill → 模式」的顺序，取模式页顶部该给的第一条提示：
 *  资源未探测 → detect；skill 状态已读到且没装 → install；所有模式都没人选 → recommend。 */
export function modesPrompt(input: {
  resources: ResourcesOutput | null | undefined;
  skillHost: SkillHostStatus | null | undefined;
  skillLoaded: boolean;
  modes: Pick<ModeEntry, 'stages'>[];
}): FirstUsePrompt {
  if (needsFirstDetect(input.resources)) return 'detect';
  if (input.skillLoaded && !skillInstalled(input.skillHost)) return 'install';
  if (hasNoSeats(input.modes)) return 'recommend';
  return null;
}

/** 推荐返回的 notes 统一成按条的数组，去掉空条。 */
export function recommendNotes(notes: string[] | string | null | undefined): string[] {
  const list = Array.isArray(notes) ? notes : typeof notes === 'string' ? notes.split('\n') : [];
  return list.map((note) => note.trim()).filter((note) => note !== '');
}
