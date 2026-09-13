// 资源接入契约类型（TASK-07）：与 src/resources.rs 的 serde 输出一一对应，
// 字段保持 snake_case，不改名。

export interface ResourceModel {
  id: string;
  agent: string;
  provider: string;
  vendor: string;
  billing_source: string;
  tier: string;
  thinking: string;
  status: string;
  source: string;
  smoked_at: string;
  smoke_model: string;
  smoke_failure: string;
  meta: string;
  note: string;
}

export interface ResourceAgent {
  id: string;
  name: string;
  kind: string;
  detected: boolean;
  version: string;
  models: ResourceModel[];
}

export interface ResourceStore {
  version: number;
  agents: ResourceAgent[];
}

export interface CcSwitchHint {
  name: string;
  has_token: boolean;
  models: string[];
}

export interface ResourceOperation {
  store: ResourceStore;
  errors: string[];
  cc_switch_hints: CcSwitchHint[];
}

export interface ResourceModelRequest {
  agent_id: string;
  id: string;
  provider: string;
  vendor: string;
  tier: string;
  note: string;
}

export interface ResourceModelKey {
  agent_id: string;
  id: string;
  provider: string;
}

export interface AddManualAgentRequest {
  name: string;
  binary: string;
  args: string[];
}

// ── 纯函数（vitest 覆盖） ────────────────────────────────────────────────

export const STATUS_CONNECTED = '已接入';
export const STATUS_PENDING = '待冒烟';

/** 左栏只列已接入模型，按 agent 分组；没有已接入模型的 agent 不出现。 */
export function connectedByAgent(store: ResourceStore): ResourceAgent[] {
  return store.agents
    .map((agent) => ({
      ...agent,
      models: agent.models.filter((model) => model.status === STATUS_CONNECTED),
    }))
    .filter((agent) => agent.models.length > 0);
}

/** 栏底汇总：待冒烟数 + 未接入数（状态为空或「未接入」）。 */
export function resourceSummary(store: ResourceStore): { pending: number; disconnected: number } {
  let pending = 0;
  let disconnected = 0;
  for (const agent of store.agents) {
    for (const model of agent.models) {
      if (model.status === STATUS_PENDING) pending += 1;
      else if (!model.status || model.status === '未接入') disconnected += 1;
    }
  }
  return { pending, disconnected };
}

export function resourceSummaryText(store: ResourceStore): string {
  const { pending, disconnected } = resourceSummary(store);
  return [
    pending ? `${pending} 个待冒烟` : '',
    disconnected ? `${disconnected} 个未接入` : '',
    '管理 →',
  ]
    .filter(Boolean)
    .join(' · ');
}

/** 接入页卡片里的模型按 provider 分组（无 provider 的并成一组，不显示组头）。 */
export function groupModelsByProvider(
  models: ResourceModel[],
): { label: string; models: ResourceModel[] }[] {
  const groups = new Map<string, ResourceModel[]>();
  for (const model of models) {
    const label = model.provider.trim();
    groups.set(label, [...(groups.get(label) ?? []), model]);
  }
  return [...groups.entries()].map(([label, grouped]) => ({ label, models: grouped }));
}

/** 卡片左上角标记：agent id 前两位字母数字大写。 */
export function agentMark(id: string): string {
  return id.replace(/[^A-Za-z0-9]/g, '').slice(0, 2).toUpperCase() || 'AI';
}

/** 启动参数文本 → argv（支持单双引号；引号未闭合抛错）。 */
export function parseCommandArgs(text: string): string[] {
  const args: string[] = [];
  let current = '';
  let quote: "'" | '"' | null = null;
  let started = false;
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quote) {
      if (character === quote) {
        quote = null;
      } else if (quote === '"' && character === '\\' && (text[index + 1] === '"' || text[index + 1] === '\\')) {
        current += text[index + 1];
        index += 1;
      } else {
        current += character;
      }
      started = true;
    } else if (/\s/.test(character)) {
      if (started) {
        args.push(current);
        current = '';
        started = false;
      }
    } else if (character === '"' || character === "'") {
      quote = character;
      started = true;
    } else {
      current += character;
      started = true;
    }
  }
  if (quote) throw new Error('启动参数引号未闭合');
  if (started) args.push(current);
  return args;
}

export function errorText(value: unknown): string {
  const message = value instanceof Error ? value.message : String(value);
  return message.replace(/^Error:\s*/, '');
}
