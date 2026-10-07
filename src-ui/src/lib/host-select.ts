// 当前主 agent 的选中记忆与厂商显示（TASK-51）。选中的主 agent 存在
// localStorage；读写都包 try/catch，读不到或已不在列表里时选第一个。

import type { HostEntry } from '../tauri';

export const HOST_STORAGE_KEY = 'aicross.host';

type HostStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** 记住的主 agent 仍在列表里就用它，否则用列表第一个；列表为空时为空串。 */
export function readHost(storage: HostStorage | null, hosts: Pick<HostEntry, 'id'>[]): string {
  let saved: string | null = null;
  try {
    saved = storage?.getItem(HOST_STORAGE_KEY) ?? null;
  } catch {
    saved = null;
  }
  if (saved !== null && hosts.some((host) => host.id === saved)) return saved;
  return hosts[0]?.id ?? '';
}

export function writeHost(storage: HostStorage | null, id: string): void {
  try {
    storage?.setItem(HOST_STORAGE_KEY, id);
  } catch {
    /* 存不进去只是下次回到第一个，不影响当前页面 */
  }
}

/** 实际厂商的显示：空串按「未知」显示；「未知」用异常色。 */
export function vendorText(host: Pick<HostEntry, 'vendor'>): { text: string; alert: boolean } {
  const vendor = host.vendor.trim();
  return vendor === '' || vendor === '未知' ? { text: '未知', alert: true } : { text: vendor, alert: false };
}

/** 厂商下拉的取值，与资源库的 vendor 写法一致。 */
export const KNOWN_VENDORS: readonly string[] = [
  'Anthropic', 'OpenAI', 'Google', 'Moonshot', 'Zhipu', 'DeepSeek', 'Alibaba', 'MiniMax', 'StepFun',
];

/** 下拉选项：自动推定（值为空串，提交时传 auto）+ 各已知厂商；用户设定过的
 *  厂商不在已知表里时补在末尾。 */
export function vendorOptions(host: Pick<HostEntry, 'vendor' | 'vendorSource'>): { value: string; label: string }[] {
  const options = [{ value: '', label: '自动推定' }, ...KNOWN_VENDORS.map((vendor) => ({ value: vendor, label: vendor }))];
  const current = vendorSelectValue(host);
  if (current !== '' && !KNOWN_VENDORS.includes(current)) options.push({ value: current, label: current });
  return options;
}

/** 下拉当前值：用户设定过就是那个厂商，否则为自动推定（空串）。 */
export function vendorSelectValue(host: Pick<HostEntry, 'vendor' | 'vendorSource'>): string {
  return host.vendorSource === '用户设定' ? host.vendor.trim() : '';
}
