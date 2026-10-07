// 模式的种类、分组与标识（fix-49-r2）。种类决定标识块的图标与颜色（颜色变量
// --mode-<kind> 在 global.css，两套主题各一份），模式页与派发记录共用。

import type { IconName } from '../components/common/Icon';
import type { ModeEntry } from '../tauri';

export type ModeKind = 'shared' | 'economy' | 'blind' | 'arena' | 'jury' | 'swarm' | 'custom';

const KIND_BY_ID: Record<string, ModeKind> = {
  'builtin-shared': 'shared',
  'builtin-economy': 'economy',
  'builtin-blind': 'blind',
  'builtin-arena': 'arena',
  'builtin-jury': 'jury',
  'builtin-swarm': 'swarm',
};

const KIND_BY_NAME: Record<string, ModeKind> = {
  共享模式: 'shared',
  经济模式: 'economy',
  盲审模式: 'blind',
  擂台模式: 'arena',
  评审模式: 'jury',
  蜂群模式: 'swarm',
};

export const MODE_ICONS: Record<ModeKind, IconName> = {
  shared: 'chat',
  economy: 'coins',
  blind: 'eye-off',
  arena: 'trophy',
  jury: 'scale',
  swarm: 'swarm',
  custom: 'sliders',
};

/** 先按 id 认，再按名称认（派发记录只有模式名）；都认不出算自定义。 */
export function modeKind(id: string, name: string): ModeKind {
  return KIND_BY_ID[id.trim()] ?? KIND_BY_NAME[name.trim()] ?? 'custom';
}

export interface ModeGroup {
  title: string;
  modes: ModeEntry[];
}

const COMMON_KINDS: ReadonlySet<ModeKind> = new Set(['shared', 'economy', 'blind']);

/** 三组：常用（共享、经济、盲审）· 更多模式（其余内置）· 自定义（builtin 为
 *  false）；组内保持原顺序，空组不出。 */
export function groupModes(modes: ModeEntry[]): ModeGroup[] {
  const common: ModeEntry[] = [];
  const more: ModeEntry[] = [];
  const custom: ModeEntry[] = [];
  for (const mode of modes) {
    if (!mode.builtin) custom.push(mode);
    else if (COMMON_KINDS.has(modeKind(mode.id, mode.name))) common.push(mode);
    else more.push(mode);
  }
  return [
    { title: '常用', modes: common },
    { title: '更多模式', modes: more },
    { title: '自定义', modes: custom },
  ].filter((group) => group.modes.length > 0);
}
