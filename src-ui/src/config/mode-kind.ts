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

/** 内置模式卡片上的一句说明（TASK-66）：先说适合什么任务，再说怎么分工。 */
const MODE_FIT: Record<Exclude<ModeKind, 'custom'>, string> = {
  shared: '适合问答、读代码、出主意；一个模型看完整对话后作答',
  economy: '适合要改代码的日常任务；高档模型出方案，低档模型改，另一家验收',
  blind: '适合核对方案、改动、结论等关键产出；几家各自审查，互相看不到',
  arena: '适合想要几个版本挑一个；几家各做一份，选定一份采纳',
  jury: '适合分头查问题；几家分别从安全、性能、需求覆盖角度审查',
  swarm: '适合能拆成互不重叠几块的大任务；各家并行完成后依次合并',
};

/** 模式卡片的说明与写入方式（TASK-66）。内置模式用写定的一句「适合……」，原有介绍作
 *  悬停提示；自定义模式用用户写的介绍。写入方式：内置蜂群在独立副本里改再合并，算改
 *  项目文件；其余按位置的写入方式推，有一个位置不是只读就算改项目文件。 */
export function modeGuide(mode: Pick<ModeEntry, 'id' | 'name' | 'builtin' | 'description' | 'stages'>): {
  text: string;
  hint: string;
  writes: '只读' | '改项目文件';
} {
  const kind = mode.builtin ? modeKind(mode.id, mode.name) : 'custom';
  const description = (mode.description ?? '').trim();
  const changes = kind === 'swarm' || mode.stages.some((stage) => stage.some((node) => node.write !== '只读'));
  const writes = changes ? '改项目文件' : '只读';
  return kind === 'custom'
    ? { text: description, hint: description, writes }
    : { text: MODE_FIT[kind], hint: description, writes };
}

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
