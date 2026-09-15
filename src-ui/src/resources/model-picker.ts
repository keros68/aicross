// 节点面板 / `@` 弹层挑模型时的分组与标签（TASK-08b）。
//
// 背景：`list_dispatchable_models` 原来只返回「已接入」（冒烟通过）的模型，本机
// 只有 4 条，用户在节点面板里挑不到探测出来的其余模型（pi 一家三个 provider 就有
// 31 条）。现在后端改成返回全部已探测模型并带 status，这里只管「怎么显示」：
// 按 agent · provider 分组、未冒烟的在选项后面点明状态。
//
// 09-15 起状态后缀改用 verification 三态：原生与已验证不加后缀，未验证与
// 验证失败点明。旧的 status 字段留着给接入页的详情用。

import { verificationLabel, verificationOf } from './types';

/** 挑模型只需要这几个字段；`DispatchableModel` 与 `ResourceModel` 都满足。 */
export interface PickableModel {
  id: string;
  agent: string;
  provider: string;
  status: string;
  verification?: string;
}

/**
 * 分组标签：多数 agent 一家一个源，标签就是 agent id；
 * pi 这种一家挂多个 provider 的写成 `agent · provider`，否则 31 条全挤在「pi」一组里。
 */
export function modelGroupLabel(model: PickableModel): string {
  const provider = model.provider.trim();
  return provider === '' ? model.agent : `${model.agent} · ${provider}`;
}

/**
 * 状态后缀：原生（自家清单说有就是有）与已验证不加。未验证与验证失败一律
 * 点明，免得用户以为下拉里列出来的都已经核过真身。
 */
export function modelStatusSuffix(model: PickableModel): string {
  const verification = verificationOf({
    agent: model.agent,
    status: model.status,
    verification: model.verification ?? '',
  });
  if (verification === 'native' || verification === 'verified') return '';
  return `（${verificationLabel(verification)}）`;
}

/** 选项文本：`agent/id` + 状态后缀。 */
export function modelOptionLabel(model: PickableModel): string {
  return `${model.agent}/${model.id}${modelStatusSuffix(model)}`;
}

/**
 * 按分组标签归组，保持传入顺序——后端已经按「原生 / 已验证 → 未验证 →
 * 验证失败，同一档内 agent → provider → id」排好，这里不再二次排序。
 */
export function groupPickableModels<T extends PickableModel>(
  models: T[],
): { label: string; models: T[] }[] {
  const groups = new Map<string, T[]>();
  for (const model of models) {
    const label = modelGroupLabel(model);
    groups.set(label, [...(groups.get(label) ?? []), model]);
  }
  return [...groups.entries()].map(([label, grouped]) => ({ label, models: grouped }));
}

/** React key：同一 agent 下不同 provider 可能有同名模型，只用 `agent/id` 会撞 key。 */
export function modelKey(model: PickableModel): string {
  return `${model.agent}/${model.provider}/${model.id}`;
}

/** 模型下拉写回 Target 时要动的三个字段。 */
export interface TargetSelection {
  agent: string;
  provider: string | undefined;
  model: string;
}

/**
 * 模型下拉的 option value。
 *
 * 用 JSON 三元组而不是 `agent/provider/id` 拼字符串，有两个原因：
 * 一是 kimi 的模型 id 自带斜杠（`kimi-code/k3`），拼出来切不回去；
 * 二是 provider 必须进 value —— pi 一家挂三个 provider，光靠 `agent/id`
 * 区分不开同名模型，选中后 `target.provider` 留空，派发 argv 的 `--provider`
 * 段就是空的，会派到错的 provider 上。`DispatchOverlay` 的目标下拉本来就是
 * 这个形状，两边对齐。
 */
export function modelSelectValue(model: PickableModel): string {
  return JSON.stringify([model.agent, model.provider ?? '', model.id]);
}

/** 当前 Target 对应的 option value，用来回选下拉。 */
export function targetSelectValue(target: {
  agent: string;
  provider?: string;
  model: string;
}): string {
  return JSON.stringify([target.agent, target.provider ?? '', target.model]);
}

/**
 * option value → 对 Target 的写回补丁。
 * provider 空串写回 `undefined`（Target 里 `provider` 就是「留空按通道默认」的语义）。
 * 解析不出三元组返回 null，调用侧原样不动。
 */
export function parseModelSelectValue(value: string): TargetSelection | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch {
    return null;
  }
  if (!Array.isArray(parsed) || parsed.length !== 3) return null;
  const [agent, provider, model] = parsed;
  if (typeof agent !== 'string' || typeof provider !== 'string' || typeof model !== 'string') {
    return null;
  }
  return { agent, provider: provider === '' ? undefined : provider, model };
}
