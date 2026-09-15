# REPORT-09b 资源页第二轮：官方图标、Claude 真实模型 ID、长列表整理

任务书 `tasks/TASK-09b-resources-polish.md`，口径依据 `docs/PRODUCT.md`
「视觉纪律」「资源冒烟口径」，上一轮见 `tasks/REPORT-09.md`。
分支 `ui/phase9b`，未 commit、未 push。

用户原话（截图后）：「这边的图标我觉得用官方的图标更好更美观，然后 claude 没有模型
id，这个部分得看看如何美化一下，现在很乱」。「乱」= pi 下面几十行未验证模型堆在
一起、每行带 provider 后缀和「未验证」文字。

## 契约

**已做：`ResourceModel` 追加 `resolved_id: String`（`#[serde(default)]`）。**
`src/resources.rs:103-109` 字段与注释。别名表
`src/resources.rs:291-302 CLAUDE_ALIAS_IDS`：`fable→claude-fable-5-1`、
`opus→claude-opus-5`、`sonnet→claude-sonnet-5`、`haiku→claude-haiku-4-5-20251001`
（四条照任务书填）。查表 `:304 resolved_id_for`（只有 claude 有别名，大小写与
空白不敏感；其余 agent 与 claude 下的完整 ID 都原样返回，即 `resolved_id == id`）。
旧库补齐 `:331 ensure_resolved_id`（**只填空值，非空不覆盖**），由 `load()`
（`:244`）与 `ensure_agents()`（`:512`）两条入口调用，和 `ensure_verification`
并排。新探测条目在 `:1177 detected_model`、手动录入在 `:1286 add_model` 直接落值。

**已做：冒烟 / 首次派发拿到真身后覆盖。** `src/resources.rs:1509-1516`，在
`apply_smoke_outcome` 的成功分支里，用核对到的 `SmokeSuccess::identity` 覆盖
`resolved_id`；pi 的真身带 provider 前缀（`deepseek/deepseek-flash`），
`:318 resolved_id_from_identity` 只取模型那一半，`smoke_model` 仍存完整真身，
两个字段不互相替代。失败分支不动 `resolved_id`——上次核对出来的真身不该被一次
网络故障抹掉。首次派发自动验证走的是同一个 `apply_smoke_outcome`，因此一并覆盖，
`src/dispatch.rs` 一个字节没动。重新探测也不许退回静态表：
`:1218-1221 merge_detected_models` 里非空的旧 `resolved_id` 优先。

**约束核对：`src/channels/`、`src/dispatch.rs` 没动**（`git diff --name-only`
里只有 `src/resources.rs`），Rust 侧改动是纯追加（`git diff --stat`：
209 insertions, 0 deletions）。

## 实现 1–6

### 1. 官方图标（已做）

新文件 `src-ui/src/components/resources/brand-marks.ts`：路径数据取自
**simple-icons**（CC0-1.0），来源、tag 与许可写在文件头（`:1-17`）。

- `claude` → `icons/anthropic.svg` @ master（读取日期 2026-09-15）
- `kimi` → `icons/kimi.svg` @ master
- `codex` → `icons/openai.svg` @ **tag 13.21.0**。OpenAI 的标志已在
  simple-icons#13944（2025-11-29，closes #12739）从 master 移除，原因是上游只
  收录拿到品牌方许可的标志，OpenAI 的许可没拿到；这里用的是移除前那一版的 CC0
  路径数据。文件头把这件事写明了，并写了「要避开这一点就把 codex 一条删掉，
  `AgentBadge` 会自动退回字母徽标」。**这一条请你拍板**，我按任务书「Codex →
  OpenAI 标志」先做了。
- `pi` → simple-icons 没有，按任务书用 `π` 字形（`:47 GLYPHS`）。
- 未收录的 agent（手动添加的）退回 `agentMark` 的字母，形状仍是同一个方槽。

`brand-marks.ts:63 agentEmblem` 是纯函数（mark / glyph / letters 三分支），
组件只负责画。`AgentBadge.tsx` 整份重写：内联 `<svg>` + `fill="currentColor"`，
**不引任何图标运行时依赖**；`aria-hidden`（旁边就是 agent 名，标志不重复播报）。

尺寸与色：`resources.css:5-44` 徽标 18px（任务书指定），`sm` / `md` 同尺寸，
只有字母 / 字形兜底才留薄底方块。色相仍从 `graph/vendor.ts` 取（内联
`--agent-tone`），深浅主题各一套——`resources.css:46-63` 给四家在
`[data-theme="light"]` 下压暗：

| agent | 深色（vendor 原色） | 浅色 | 浅色对 `--bg-panel #fafaf9` |
|---|---|---|---|
| claude | `#D97757` (2.99:1) | `#a2491f` | **5.73:1** |
| codex | `#10A37F` (3.06:1) | `#0a6b52` | **6.21:1** |
| kimi | `#6E56CF` (5.16:1) | `#4a35a8` | **8.50:1** |
| pi | `#4F9D69` (3.16:1) | `#2f6b47` | **6.07:1** |

原色在白底上做 18px 实心图形只有 3.0 左右，压边（WCAG 1.4.11 非文字 3:1），
所以才要这一套。只压暗不换色相——和对话图上的厂商色点仍是同一家的颜色。

**已补测试**：
- `src-ui/src/resources/__tests__/resources-tab-09.test.tsx`
  `agentEmblem > claude / codex / kimi 走品牌标志，pi 走 π 字形，未知退回字母`
- 同文件 `agentEmblem > 标志名与路径数据都在（漏抄一条会渲染成空徽标）`
- 同文件 `ResourceAgentList > agent 行带品牌标志与弱化版本号；徽标不再用字母`
  （断言 markup 里有 Anthropic / OpenAI 的路径数据、有 `agent-badge-pi
  agent-badge-glyph`、可见文字里有 `π` 且没有 `CO` / `CL` / `PI`）
- 同文件 `resources.css 的视觉纪律（修正轮 R1）> 徽标 18px 单色随 --agent-tone；
  字号仍在六级阶梯上，没有 9px`
- 同文件 `… > 四家徽标在浅色主题各有一套压暗色`

### 2. Claude 模型行显示真实 ID（已做）

`src-ui/src/resources/types.ts:162 resolvedIdOf`（后端值优先，空串退回 `id`）与
`:170 modelSecondaryId`（**别名与 ID 相同时返回空串**，只显示一次）。
左栏行 `ResourcesTab.tsx:269-271`：主文本 `.resource-model-id`（别名），次文本
`.resource-model-resolved`（真身）弱化——`resources.css:308-318` 用 `--text-3`
+ 11px 等宽 + 右侧省略。悬停仍给来源，并把真身与状态词一起收进去：
`types.ts:258-268 modelHint` 现在是
`别名 / 模型 ID：真身 / 状态：… / provider / 来源 / 最近验证 / 失败原因`。

实测渲染（静态渲染 dump，临时用例已删）：

```
<span class="resource-model-id">fable</span>
<span class="resource-model-resolved">claude-fable-5-1</span>
```

**已补测试**：
- `resources.test.ts` `resolvedIdOf / modelSecondaryId > 后端给的真身优先，空串退回 id`
- 同 `… > 别名与 ID 相同的模型不重复显示`
- `resources-tab-09.test.tsx`
  `ResourceAgentList > claude 别名行显示别名 + 真实 ID，非别名模型只显示一次`
- 同文件 `modelHint > 别名的真身与状态词都进悬停提示`

### 3. 长列表整理（已做）

- **按 provider 分小组**：`types.ts:205 providerGroups`。多 provider 时按
  provider 名排序分组，组内保留输入顺序（`modelsByAgent` 已按 验证档 →
  provider → id 排过）；**只有一个 provider 时返回单个无名组，不渲染组头**——
  原生 agent 的 provider 全是空串，一个组挂一个组头是纯噪声。组头 =
  provider 名 + 计数（`ResourcesTab.tsx:198-203`，样式 `resources.css:236-265`）。
- **组内不再带 `· provider` 后缀**：旧的 `ambiguousModelIds` / `modelRowLabel`
  连同它们的测试一并删掉（本次改动造成的无用代码，分组已经解决同名歧义）。
- **未验证默认折叠**：`types.ts:223 splitUnverified`（unverified 折，
  native/verified/failed 常显）+ `:236 unverifiedToggleLabel`
  （`未验证 N 个 · 展开` / `· 收起`）。折叠行本身就是按钮
  （`ResourcesTab.tsx:213-222`），带 `aria-expanded`、前面一个灰点；
  折叠键 `types.ts:241 groupKey` = `agentId/provider`（同名 provider 可能挂在
  两个 agent 下），状态在 `ResourcesTab.tsx:42 openUnverified`，默认全折。
- **状态只留一个点，文字进悬停 / `aria-label`**：`VerificationMark.tsx` 重写，
  `showLabel` 关时只渲染一个
  `<span class="verify-dot verify-…" role="img" aria-label="…" title="…">`——
  `role="img"` + `aria-label` 给读屏，`title` 给鼠标（WebView2 里 `title` 只响应
  hover），两者缺一都会漏掉一半用户。三色在 `resources.css:81-101`：verified 绿
  （浅色主题走既有的 `--verify-green` 深绿）、failed 红、unverified 灰；
  原生仍是无点无字。

实测渲染：pi 的 `zai` 组显示「zai 2」+ 已验证的 `glm-4.6` 一行 +
「未验证 1 个 · 展开」一行；claude 组没有组头。

**已补测试**：
- `resources.test.ts` `providerGroups > 多 provider 按名排序分组，组内保留输入顺序`
- 同 `… > 只有一个 provider（原生 agent 的 provider 全是空串）时不分组`
- 同 `… > 没有模型时不出空组`
- 同 `… > 组的折叠键带 agent：同名 provider 可能挂在两个 agent 下`
- 同 `splitUnverified / unverifiedToggleLabel > 未验证折叠，已验证 / 验证失败 / 原生常显`
- 同 `… > 折叠行的文字带计数与展开 / 收起`
- `resources-tab-09.test.tsx`
  `ResourceAgentList > pi 按 provider 分小组，未验证默认折叠，组内不带 provider 后缀`
  （快照式字符串断言：可见文字里有 `zai` / `bigmodel` / `未验证 1 个 · 展开`，
  没有 `glm-4.7 · zai`、没有折着的 `glm-4.7`）
- 同 `… > 展开某个小组后，只有该组的未验证模型出现`
- 同 `… > 状态点不跟文字：状态词只在 aria-label 与 title 里`
  （断言 `class="verify-dot verify-failed" role="img" aria-label="验证失败"`
  在 markup 里，而「已验证」「验证失败」「原生」都不在可见文字里）

### 4. 字体与密度（已做）

- 模型名改界面正文字体：`resources.css:296-305` 的 `.resource-model-id` 去掉
  `font-family: var(--font-mono)`；接入页 `.agent-card-model-id`（`:543`）同改。
  真身 ID 与组内计数仍留等宽（它们才是结构性文字，与视觉纪律「模型 ID 用 Mono」
  的本意一致——别名不是 ID）。
- 行高与间距按 4 的倍数：模型行 / 组头 / 折叠行统一 `padding: 4px 8px`、
  `gap: 8px`、`line-height: 16px`（`resources.css:243-252`、`:267-278`、
  `:320-334`），组间 `margin-top: 4px`。既有的「左栏的内边距与间隙都是 4 的
  倍数」那条测试的选择器清单里加进了 `.resource-group-head` 与
  `.resource-unverified-toggle`，机器核对。
- agent 行版本号弱化保持：`.resource-agent-version` 一个字节没改。
- 顺手给折叠行补了 `:focus-visible`（`resources.css:341-344`），与既有两种行
  同一套 2px 不透明强调色。

**已补测试**：`resources-tab-09.test.tsx`
`resources.css 的视觉纪律（修正轮 R1）> 左栏的内边距与间隙都是 4 的倍数`
（扩了选择器清单，原有断言逻辑不变）。

### 5. 接入页同步（已做）

`src-ui/src/components/resources/ResourcesOverlay.tsx`：
- 品牌图标自动跟上（同一个 `AgentBadge`，`:328 size="md"`）。
- 分组换成同一个 `providerGroups`（`:319`），组头也改成 provider 名 + 计数
  （`:338-344`，样式 `resources.css:507-529`）；单 provider 的卡片不出组头。
- Claude 卡片显示别名 + 真实 ID：`:357-361` 的 `.agent-card-model-resolved`。
- `AgentCard` 加了 `export`（`:295`），只为让 vitest 直接
  `renderToStaticMarkup`；它本来就是组件，不触发 `react-refresh` 那条 lint。
- 接入页的状态仍带文字（`showLabel`）：一张卡片就几条，有版面；failed 仍是
  红胶囊，符合「胶囊只给异常状态」。任务书第 3 条的「状态只留一个点」是针对
  左栏长列表说的，这里没跟着改。

**已补测试**（新文件 `src-ui/src/resources/__tests__/resources-overlay-09b.test.tsx`）：
- `AgentCard（接入页）> 卡片用与左栏同一套品牌标志`
- `… > Claude 卡片显示别名 + 真实 ID；两者相同的模型只显示一次`
- `… > 多 provider 的卡片按 provider 分组，组头带计数；单 provider 不出组头`

### 6. 测试（已做）

Rust 四条（`src/resources.rs:2747-2876`）：
- `resources::tests::task09b_claude_aliases_resolve_to_known_model_ids`
  四个别名的映射 + 大小写空白 + 非别名（claude 下的完整 ID、pi 下同名词、
  codex 模型）都返回 `id`。
- `resources::tests::task09b_legacy_store_backfills_resolved_id`
  旧库（无字段）读进来时按表补齐；库里已有的非空值不被静态表覆盖。
- `resources::tests::task09b_smoke_success_overwrites_resolved_id_with_identity`
  真身覆盖静态表；失败不抹掉；pi 的 `provider/model` 只取模型那一半，
  `smoke_model` 仍是完整真身。
- `resources::tests::task09b_redetect_keeps_verified_resolved_id`
  重新探测不许把核对出来的真身退回静态表。

TS：见上面各条的「已补测试」。前端从 304 条涨到 369 条。

## 文档（范围之外但必须改的两处）

- `docs/PRODUCT.md:234`「资源冒烟口径」那一条原文写着「统一风格字母徽标 + 各家
  色相，**不用商标图**」——本轮被用户推翻，改成记录新口径（官方品牌标志、
  simple-icons CC0、pi 用 π、未收录退回字母）并追加两条：09-15 第二轮的分组 /
  折叠 / 状态点，以及 `resolved_id` 的口径。原文件是 CRLF，按字节改，改后
  CRLF 239 / LF 6（改前 237 / 6，只多了新增的两行）。
- `docs/MANUAL.md`「接入资源」一节加了「左栏的读法」一段（标志、状态点三色、
  provider 小组、未验证折叠、claude 别名后面的真实 ID），并把「清单里先标
  「未验证」」改成「先记」——现在它不是一行文字了。

## 验证（全部实跑，输出原文）

```
$ cargo check
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.50s
```

零 warning。`cargo check --all-targets`（含 test target）也是零 warning：

```
$ cargo check --all-targets
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.20s
```

```
$ cargo test

test result: ok. 322 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 10.38s
```

新增四条全绿：

```
$ cargo test task09b

running 4 tests
test resources::tests::task09b_claude_aliases_resolve_to_known_model_ids ... ok
test resources::tests::task09b_redetect_keeps_verified_resolved_id ... ok
test resources::tests::task09b_legacy_store_backfills_resolved_id ... ok
test resources::tests::task09b_smoke_success_overwrites_resolved_id_with_identity ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 329 filtered out; finished in 0.01s
```

```
$ cd src-ui && npx vitest run --configLoader native --pool threads
 RUN  v5.0.0 D:/work/aicross-ui/src-ui

 Test Files  26 passed (26)
      Tests  369 passed (369)
   Start at  11:00:22
   Duration  653ms (transform 62%, tests 18%, import 17%, worker 3%)
```

```
$ cd src-ui && npm run build
dist/assets/main-ct3Up3Yx.js                                   440.97 kB │ gzip: 215.38 kB
dist/assets/xterm-C7jn2Fqg.js                                  465.79 kB │ gzip: 123.12 kB

✓ built in 642ms
```

额外跑了 `npx eslint src/components/resources src/resources`：**1 error**，与改前
同一条、同一处——`ResourcesTab.tsx:68` 既有的 `react-hooks/set-state-in-effect`
（REPORT-09 已记为与改动无关）。本轮没有新增 lint 问题，没有新增
`react-refresh/only-export-components`（`groupKey` 放在 `resources/types.ts`
而不是组件文件里）。

## 约束核对

- 换行：改动的 10 个文件 + 2 个新文件，LF 的仍是 LF（`src/resources.rs` 3381、
  `types.ts` 312、`resources.css` 773 等 CRLF 计数全为 0），CRLF 的
  `docs/PRODUCT.md` 仍是 CRLF。`git diff` 逐文件核过，没有整文件换行翻转。
- `src/channels/` 与 `src/dispatch.rs` 一个字节没动；Rust 只改了
  `src/resources.rs`，且是纯追加（209 insertions / 0 deletions）。
- 官方标志用 simple-icons 的开源路径数据，来源与许可在 `brand-marks.ts` 文件头；
  内联 SVG，`package.json` 没动，没引第三方运行时依赖。
- 没起窗口、没发真机请求（唯一的外发是取 simple-icons 的 SVG 路径数据，
  不是模型端点）。没 commit、没 push，没写仓库以外的位置。
- 文案中性简短，状态点不配文字。
- 没跑 `cargo fmt`。

## 不确定处 / 待你拍板

1. **Codex 用 OpenAI 标志这件事**。simple-icons 已把 OpenAI 移出 master，理由是
   拿不到品牌方许可（#12739 / #13944），我用的是 13.21.0 tag 的 CC0 数据。
   数据本身没问题，指称性使用也站得住，但上游是主动撤的。想稳一点就把
   `BRAND_MARKS.codex` 一条删掉，`AgentBadge` 会自动退回字母兜底，不用改别处。
2. **别名表的四个 ID 照任务书填**（`claude-fable-5-1` / `claude-opus-5` /
   `claude-sonnet-5` / `claude-haiku-4-5-20251001`）。claude 的事实源是
   `--help` 里的别名举例，拿不到完整 ID，这张表是唯一的本地事实源，会漂移；
   兜底是冒烟 / 首次派发核对到真身后覆盖。真机上跑一次验证就能看到实际值。
3. **kimi 的标志在深色主题上是 3.0:1**（`#6E56CF` 对 `--bg-panel #202020`），
   正好压在 3:1 线上。它是装饰性图形、旁边就是 agent 名，严格说不受 1.4.11
   约束；真机上看着暗就把深色那一套也压一下。
4. **界面只做了代码、样式与静态渲染核对，没起窗口看实际渲染**（约束要求
   不起窗口）。18px 标志配 12px 模型名的观感、以及 claude 行「别名 + 真身」
   在窄左栏里够不够放，需要真机看一眼。
