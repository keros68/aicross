// 首启引导的判定（TASK-05 C 节）。
//
// 判定与渲染分开：这里是纯函数，组件只负责画。引导只在「一张图都没有、
// 一个资源都没接入」时出现，任一条不满足就不出现——已经用起来的人不该
// 再看见入门条。读取中（null）同样不显示，避免闪一下又消失。

export interface FirstRunFacts {
  /** 已保存的对话图数量；null = 还在读。 */
  graphCount: number | null;
  /** 可派发模型数量；null = 还在读。 */
  resourceCount: number | null;
  /** 当前会话的工作目录；null = 没选。 */
  folderPath: string | null;
}

export type FirstRunStepKey = 'folder' | 'resources' | 'preset';

export interface FirstRunStep {
  key: FirstRunStepKey;
  /** 动作名，2–6 字动词短语。 */
  action: string;
  /** 一句话说明这步在干什么。 */
  hint: string;
  /** 这步是否已经做过（只影响样式，不影响是否显示）。 */
  done: boolean;
}

export function shouldShowFirstRun(facts: FirstRunFacts): boolean {
  if (facts.graphCount === null || facts.resourceCount === null) return false;
  return facts.graphCount === 0 && facts.resourceCount === 0;
}

export function firstRunSteps(facts: FirstRunFacts): FirstRunStep[] {
  return [
    {
      key: 'folder',
      action: '打开目录',
      hint: '选一个工作目录，派发记录写在它的 .dispatch/ 里。',
      done: !!facts.folderPath,
    },
    {
      key: 'resources',
      action: '接入资源',
      hint: '申报本机已装的 agent CLI 和能派的模型。',
      done: (facts.resourceCount ?? 0) > 0,
    },
    {
      key: 'preset',
      action: '选预设',
      hint: '在右栏挑一个编排预设，定下谁出方案、谁执行、谁验收。',
      done: false,
    },
  ];
}
