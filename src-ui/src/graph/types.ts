// 对话图数据模型（TASK-02 A 节契约，字段名与 Rust 侧 serde snake_case 镜像）

export type NodeKind = 'material' | 'turn' | 'accept';
export type ReplyStatus = 'draft' | 'pending' | 'done' | 'failed';
export type Thinking = '关' | '低' | '中' | '高';
export type VisibilityKind = '共享' | '盲' | '指定上游';
export type WriteMode = '只读' | '单写者' | 'worktree';

export interface Message {
  text: string;
  attachments: string[]; // 文件路径
}

export interface Reply {
  status: ReplyStatus;
  text: string;
  model_reported?: string;
  tokens_in?: number;
  tokens_out?: number;
  // TASK-07 契约：缓存命中的输入 token。派发结果与对账都写它，结果屏的
  // cache_read 列优先读这个字段。旧图没有，按缺失处理。
  tokens_cache_read?: number;
  ms?: number;
  session_id?: string;
  runner?: 'acp' | 'oneshot' | 'unknown';
  adopted?: boolean;
  artifacts?: ReplyArtifacts;
  error_kind?: string;
  skipped_reason?: string;
  // TASK-06 契约：结果由备选重跑产生时记主/备路由（"agent/model"）。
  // 正文首行「（已切备选：…）」标记不再写入，读旧图时仍由 fallbackNoticeOf 识别。
  fallback_from?: string;
  fallback_to?: string;
}

export interface ChangedFile {
  path: string;
  content: string;
}

export interface ReplyArtifacts {
  diff: string;
  changed_files: ChangedFile[];
  worktree?: string;
  stdout_summary: string;
}

// 备选目标（TASK-05B 主备/熔断）：主目标 error/timeout/identity_mismatch 时
// 由通道层用备选重跑一次。图契约本期唯一的字段追加。
export interface FallbackTarget {
  agent: string;
  provider?: string;
  model: string;
}

export interface Target {
  agent: string; // agent id，如 'claude'/'codex'/'kimi'/'pi'；未填为 ''
  provider?: string;
  model: string; // 模型 id；未填为 ''
  role: string; // '出方案' | '执行' | '验收' | '验证' | '顾问' | '仲裁' | 自定义文本
  tier?: string; // 档位
  thinking: Thinking;
  visibility: VisibilityKind;
  upstreams?: string[]; // 仅 visibility='指定上游' 时有意义，上游节点 id 列表
  write: WriteMode;
  validation_command?: string;
  fallback?: FallbackTarget; // 备选；不设则主目标失败即终态
  preset_text?: string; // 内置预设盖章时写入 user.text，由 Rust 冻结材料提供
}

// 蜂群（TASK-08）：拆分清单的一件活落在执行节点上。item 是件号（从 1 起，
// 决定合并顺序），files 是这件活声明的文件范围（glob），accept 是这件的验收
// 命令（空则合并时回退到设置里的验证命令）。旧图没有这个字段，按缺失处理。
export interface SwarmItem {
  item: number;
  files: string[];
  accept: string;
}

// 遗留（TASK-12）：修正轮到顶后未闭合的条目清单，挂在最后一个验收节点下。
// 只读、不可派发（dispatchableNodeIds 排除）；source_node_id 是来源验收节点。
export interface Leftover {
  source_node_id: string;
  items: string[];
}

export interface GraphNode {
  id: string;
  kind: NodeKind;
  parents: string[];
  user: Message;
  reply?: Reply;
  target?: Target;
  swarm?: SwarmItem;
  leftover?: Leftover;
  created_at: string;
}

export interface Graph {
  version: 1;
  id: string;
  title: string;
  root: string;
  nodes: Record<string, GraphNode>;
  created_at: string;
  updated_at: string;
}

export interface Preset {
  id: string;
  name: string;
  builtin: boolean;
  stages: Stage[];
}

export interface Stage {
  nodes: Target[]; // 同阶段并行，阶段间串行
}

// 可派发模型（对应 Rust list_dispatchable_models 返回的 ResourceModel 子集，字段全量保留以兼容）
export interface DispatchableModel {
  id: string;
  agent: string;
  provider: string;
  vendor: string;
  billing_source: string;
  tier: string;
  thinking: string;
  status: string;
  // 09-15 口径：native / unverified / verified / failed（旧图的缓存可能没有）。
  verification?: string;
  source: string;
  smoked_at: string;
  smoke_model: string;
  smoke_failure: string;
  meta: string;
  note: string;
}

export function roleDefaults(role: string): { visibility: VisibilityKind; write: WriteMode } {
  if (role === '验收' || role === '验证' || role === '顾问' || role === '仲裁') {
    return { visibility: '盲', write: '只读' };
  }
  // 拆分（TASK-08 蜂群）：看得到根材料，只出清单不改文件。值与兜底相同，
  // 写成显式分支，兜底以后变了也不会把拆分一起改掉（R1-7）。
  if (role === '拆分') return { visibility: '共享', write: '只读' };
  return { visibility: '共享', write: '只读' };
}

export function defaultThinking(): Thinking {
  return '中';
}

export function newNodeId(): string {
  return crypto.randomUUID();
}

export function cloneTarget(target: Target): Target {
  return {
    ...target,
    upstreams: target.upstreams ? [...target.upstreams] : undefined,
    fallback: target.fallback ? { ...target.fallback } : undefined,
  };
}

// 标题置空：默认名「对话图」由展示层兜底（tab 标题 / 已有图列表），
// fillRoot 发第一句时取首行前 20 字作标题。
export function newGraph(): Graph {
  const now = new Date().toISOString();
  const rootId = newNodeId();
  const root: GraphNode = {
    id: rootId,
    kind: 'material',
    parents: [],
    user: { text: '', attachments: [] },
    created_at: now,
  };
  return {
    version: 1,
    id: newNodeId(),
    title: '',
    root: rootId,
    nodes: { [rootId]: root },
    created_at: now,
    updated_at: now,
  };
}
