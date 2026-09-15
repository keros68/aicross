// 厂商色点取色（PRODUCT.md 视觉纪律：厂商色只做小面积标识，未知厂商灰色）。
// vendor 名以 resources.json 的取值为准（Anthropic/OpenAI/Moonshot/Zhipu/
// DeepSeek/MiniMax/Alibaba/StepFun/其他），agent id 作为兜底线索。

const BY_VENDOR: Record<string, string> = {
  anthropic: '#D97757',
  openai: '#10A37F',
  google: '#3186FF',
  moonshot: '#6E56CF',
  zhipu: '#3D7FFF',
  deepseek: '#4D6BFE',
  minimax: '#E25563',
  alibaba: '#6336E7',
  stepfun: '#F2A33C',
  xiaomi: '#FF6900',
  xai: '#A8A8A8',
};

const BY_AGENT: Record<string, string> = {
  claude: '#D97757',
  codex: '#10A37F',
  opencode: '#10A37F',
  mimocode: '#FF6900',
  antigravity: '#3186FF',
  qwen: '#6336E7',
  grok: '#A8A8A8',
  kimicode: '#6E56CF',
  // 资源页的 agent 徽标也从这里取色（修正轮 R1）：同一个 agent 在对话图上的
  // 色点与资源页的徽标必须同色，否则「一眼看出这段是谁跑的」就不成立。
  kimi: '#6E56CF',
  pi: '#4F9D69',
};

export const UNKNOWN_VENDOR_COLOR = '#8c8c8c';

export function vendorColor(agent?: string, vendor?: string): string {
  const v = (vendor ?? '').trim().toLowerCase();
  if (v !== '' && BY_VENDOR[v]) return BY_VENDOR[v];
  const a = (agent ?? '').trim().toLowerCase();
  if (a !== '' && BY_AGENT[a]) return BY_AGENT[a];
  return UNKNOWN_VENDOR_COLOR;
}
