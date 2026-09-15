// agent 品牌标志的 SVG 路径数据（TASK-09b）。
//
// 来源：simple-icons（https://github.com/simple-icons/simple-icons），
// 图标数据以 CC0-1.0 发布（仓库 LICENSE.md）。路径原样复制，未改形。
//   - `anthropic`  icons/anthropic.svg @ master（读取日期 2026-09-15）
//   - `kimi`       icons/kimi.svg      @ master（读取日期 2026-09-15）
//   - `openai`     icons/openai.svg    @ **tag 13.21.0**
//     OpenAI 的标志已在 simple-icons#13944（2025-11-29）从 master 移除，
//     原因是上游只收录拿到品牌方许可的标志，OpenAI 的许可还没拿到。这里用的
//     是移除前那一版的 CC0 路径数据。
//
// 商标归各品牌方所有；本产品只把它们当「这一行是哪家 CLI」的标识用（指称性
// 使用），不做背书暗示，也不用作本产品自身的标志。若要避开这一点，把 codex
// 一条从表里删掉即可，AgentBadge 会自动退回字母徽标——这条逃生门有测试钉住
// （`brand-marks 的逃生门 > 删掉 BRAND_MARKS.codex，徽标回退字母`）。
//
// 归属同时写进仓库 NOTICE 的 aicross 段（「THIRD-PARTY ASSETS ADDED BY
// AICROSS」）——上游那段逐字保留，新条目不能加进去（AGENTS.md）。
//
// pi 没有公开标志，按任务书用 π 字形；未知 agent 退回 `agentMark` 的字母。

export interface BrandMark {
  /** SVG viewBox，simple-icons 一律 `0 0 24 24`。 */
  viewBox: string;
  /** 单条 path 的 `d`，按 currentColor 填充。 */
  path: string;
}

/**
 * agent id（小写）→ 品牌标志。没有条目的 agent 走字形或字母兜底。
 * 徽标 `aria-hidden`（旁边就是 agent 名），所以这里不留标志名字段——
 * 原来的 `title` 没有任何渲染路径消费（修正轮 R1，DS 9），品牌归属写在
 * 文件头与 NOTICE 里。
 */
export const BRAND_MARKS: Record<string, BrandMark> = {
  // Anthropic
  claude: {
    viewBox: '0 0 24 24',
    path: 'M17.3041 3.541h-3.6718l6.696 16.918H24Zm-10.6082 0L0 20.459h3.7442l1.3693-3.5527h7.0052l1.3693 3.5528h3.7442L10.5363 3.5409Zm-.3712 10.2232 2.2914-5.9456 2.2914 5.9456Z',
  },
  // OpenAI
  codex: {
    viewBox: '0 0 24 24',
    path: 'M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z',
  },
  // Kimi（Moonshot AI）
  kimi: {
    viewBox: '0 0 24 24',
    path: 'M21.765.351C22.998.351 24 1.353 24 2.586S22.998 4.82 21.765 4.82h-1.974c-.15 0-.26-.12-.26-.26V2.586A2.237 2.237 0 0 1 21.765.35M9.41 13.388l8.447-8.377c.16-.16.07-.471-.14-.471h-4.55s-.1.02-.14.06l-9.099 9.029c-.14.14-.35.02-.35-.21V4.81c0-.15-.1-.27-.221-.27H.22c-.12 0-.22.12-.22.27v18.57c0 .15.1.27.22.27h3.137c.12 0 .22-.12.22-.27v-3.79c0-.08.03-.16.08-.21l2.826-2.796c.07-.07.16-.08.241-.03l7.546 5.551a8.9 8.9 0 0 0 4.018 1.493c.12.01.23-.11.23-.27V19.76c0-.14-.08-.25-.19-.26a5.8 5.8 0 0 1-2.355-.942l-6.533-4.73c-.14-.09-.15-.32-.03-.441',
  },
};

/** agent id → 没有品牌标志时的字形。pi 按任务书用 π。 */
const GLYPHS: Record<string, string> = { pi: 'π' };

/**
 * 色相由 `resources.css` 按主题给的 slug（`.agent-badge-<slug>` + 浅色覆盖）。
 * 这几个 slug 的徽标**不许**内联 `--agent-tone`：内联自定义属性压过样式表，
 * 压暗规则会整条失效（修正轮 R1，DS A1）。未收录的 agent 没有主题分档，
 * 仍由组件内联 vendor 原色。
 */
export const CSS_TONED_SLUGS: readonly string[] = ['claude', 'codex', 'kimi', 'pi'];

/** 这个 agent 的徽标色是不是由样式表按主题给的。 */
export function hasCssTone(slug: string): boolean {
  return CSS_TONED_SLUGS.includes(slug);
}

export type AgentEmblem =
  | { kind: 'mark'; slug: string; mark: BrandMark }
  | { kind: 'glyph'; slug: string; glyph: string }
  | { kind: 'letters'; slug: string; letters: string };

/** agent id → 徽标槽位的 class 后缀（色相与标志都按它查）。 */
export function agentSlug(agentId: string): string {
  return agentId.trim().toLowerCase().replace(/[^a-z0-9]/g, '') || 'unknown';
}

/**
 * 徽标该画什么：品牌标志 → 字形 → 字母。
 * 纯函数，组件只负责把它渲染出来，vitest 直接断言分支。
 */
export function agentEmblem(agentId: string, letters: string): AgentEmblem {
  const slug = agentSlug(agentId);
  const mark = BRAND_MARKS[slug];
  if (mark) return { kind: 'mark', slug, mark };
  const glyph = GLYPHS[slug];
  if (glyph) return { kind: 'glyph', slug, glyph };
  return { kind: 'letters', slug, letters };
}
