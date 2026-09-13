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
};

export const UNKNOWN_VENDOR_COLOR = '#8c8c8c';

export function vendorColor(agent?: string, vendor?: string): string {
  const v = (vendor ?? '').trim().toLowerCase();
  if (v !== '' && BY_VENDOR[v]) return BY_VENDOR[v];
  const a = (agent ?? '').trim().toLowerCase();
  if (a !== '' && BY_AGENT[a]) return BY_AGENT[a];
  return UNKNOWN_VENDOR_COLOR;
}
