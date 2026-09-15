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

---

# 修正轮 R1

依据 `tasks/fix-09b-r1.md`（7 组，来自盲审 `tasks/review-09b-ds.md`）。
`tasks/review-09b-glm.md` 在本轮做到一半时才出现，读完后按「不重复的必修项」
另处理，见下面「GLM 审查的增量」一节。Rust 只动了 `src/resources.rs`，
`git diff --name-only` 里没有 `src/channels/` 与 `src/dispatch.rs`。
改动的 12 个文件里 11 个是 LF（CR 字节数 0），`docs/PRODUCT.md` 仍是 CRLF
（239 CR / 245 LF，与改前一致，只换了一句话、没增删行）。

## 1. 浅色主题压暗色真正生效（DS A1）

**已修**：已知四家的 `--agent-tone` 从组件内联挪进样式表。
`resources.css:52-83` 新增基础规则（`.agent-badge-claude/codex/kimi/pi`）
加上原有的 `[data-theme="light"]` 覆盖；`AgentBadge.tsx:19-22` 改成
`hasCssTone(emblem.slug) ? undefined : {'--agent-tone': vendorColor(...)}`，
内联只留给未收录的 agent；`brand-marks.ts:56-67` 新增
`CSS_TONED_SLUGS` / `hasCssTone`，让「哪几个 slug 归样式表管」只有一处事实源。

DS 的判断成立：内联自定义属性在层叠里压过作者样式表，
`[data-theme="light"] .agent-badge-claude` 命中的是同一个 `<span>`，
永远压不过它自己的 `style` 属性——上一轮那套压暗色等于没写。

**已补**（能红的断言，不再是「测 CSS 文本」）：
- `resources-tab-09.test.tsx` `ResourceAgentList > 已知 agent 的徽标不带内联 --agent-tone，色由样式表按主题给`
  （断言 markup 里既没有 `--agent-tone`，也没有 `vendorColor('codex')` /
  `vendorColor('pi')` 的值；class 上仍有 slug）
- 同文件 `ResourceAgentList > 未收录的 agent 没有主题分档，仍内联 vendor 兜底色`
- 同文件 `resources.css 的视觉纪律（修正轮 R1）> 四家徽标深浅主题各有一套色，都写在样式表里`
- 同文件 `resources.css 的视觉纪律（修正轮 R1）> claude / codex / pi 的深色调用的就是 graph/vendor.ts 的厂商原色`
  （把「徽标色与对话图色点同源」这条口径也钉住了）
- 同文件 `brand-marks 的逃生门 > 色相由样式表给的 slug 与标志表 / 字形表对得上`

**已删**：上一轮那条假绿 `徽标色相从 graph/vendor.ts 取，不另立一套`——
它断言 markup 里含 `vendorColor(...)` 的值，恰好是在证明内联还在。

**顺带**：解析 CSS 的测试统一先 `stripComments()`。原来注释里举的
`[data-theme="light"] .agent-badge-claude { --agent-tone }` 例子会被规则正则
先命中（写这条测试时当场撞上），注释文字也改掉了。

## 2. 对比度（DS A2、A3）

**已修 A2（未验证灰点）**：`resources.css:95` 新增 `--verify-gray: var(--text-2)`，
`:117` 的 `.verify-dot` 底色从 `var(--text-3)` 改成 `var(--verify-gray)`。
本轮把状态文字删掉之后这个点是唯一可见标识，而 `--text-3` 在 cappuccino
只有 **2.06:1**（DS 估的 2.2 接近）、obsidian 2.74、moss 2.98、cobalt 3.03 贴线；
`--text-2` 最低一档（cappuccino）**3.88:1**，16 套主题全过。选 token 不选
1px 描边：6px 的点加描边会变成一个圈，三色区分反而弱。

**已修 A3（kimi 徽标）**：深色那一套也进了样式表，kimi 从 `#6E56CF` 提亮到
同色相的 `#8574E0`（`resources.css:60-62`）。原色只差一点：dark 3.02、
obsidian 3.50 过，cappuccino **2.84**、mint **2.93**、teal **2.76** 不过。
claude / codex / pi 的原色对最亮的深色面板（teal `#102b30`）分别是
4.77 / 4.66 / 4.51，不动。

**已补**（按主题实算，不手抄）：
- `resources-tab-09.test.tsx` `徽标与状态点的对比度 ≥ 3:1（修正轮 R1）> 四家徽标在每套主题的面板底色上都 ≥ 3:1`
- 同 describe `徽标与状态点的对比度 ≥ 3:1（修正轮 R1）> 三色状态点在每套主题上都 ≥ 3:1`
- 同 describe `徽标与状态点的对比度 ≥ 3:1（修正轮 R1）> global.css 的主题表读得到（读不到就不是「全过」而是没测）`
  （防止解析失败导致 0 次循环、假绿）
- `resources.css 的视觉纪律（修正轮 R1）> 未验证灰点走 --verify-gray（--text-3 在 cappuccino 只有 2.06:1）`

这几条从 `src-ui/src/styles/global.css` 解析出每套主题的 `--bg-panel`、
`--text-2`、`--green`、`--red`，再从 `resources.css` 解析徽标色，按 WCAG
相对亮度公式算，不写死数字。**反向验过能红**：把 kimi 临时改回 `#6E56CF`，
该用例报 `cappuccino / kimi #6e56cf on #252525: expected 2.84 to be greater
than or equal to 3`，改回后转绿。

### 更正：对比度表（替换本报告「实现 1」那张）

上一版表里「深色」一列是**对白底**的对比度，被当成了对 `--bg-panel #202020`
的值，与「不确定处 3」自相矛盾（kimi 对任何深底都到不了 5.16，对纯黑上限
3.90）。DS 这条判断正确，错的是表。

计算基准：WCAG 相对亮度 + `(L1+0.05)/(L2+0.05)`；前景 = 徽标色，
背景 = 该主题 `global.css` 里的 `--bg-panel`（左栏与卡片的实际底色）。
判据 = WCAG 2.2 § 1.4.11 非文本对比度 3:1。列出四套有代表性的主题：
dark 是默认，cappuccino 是深色里面板最亮的一批，mint 是有色深底，
teal `#102b30` 是 16 套里面板亮度最高的、对深色前景最不利。

| agent | 深色调 | dark `#202020` | cappuccino `#252525` | mint `#142623` | teal `#102b30` | 浅色调 | light `#fafaf9` |
|---|---|---|---|---|---|---|---|
| claude | `#D97757` | 5.22 | 4.91 | 5.05 | **4.77** | `#a2491f` | 5.73 |
| codex | `#10A37F` | 5.10 | 4.79 | 4.93 | **4.66** | `#0a6b52` | 6.21 |
| kimi | `#8574E0` | 4.32 | 4.07 | 4.19 | **3.95** | `#4a35a8` | 8.50 |
| pi | `#4F9D69` | 4.93 | 4.64 | 4.78 | **4.51** | `#2f6b47` | 6.07 |

被替换掉的旧值与它们的真实含义：`claude 2.99 / codex 3.06 / kimi 5.16 /
pi 3.16` 是**厂商原色对浅色面板 `#fafaf9`** 的对比度——也就是「浅色主题
为什么必须压暗」的依据，不是深色主题的数据。旧 kimi `#6E56CF` 在深色主题
的真实值是 dark 3.02 / cappuccino 2.84 / mint 2.93 / teal 2.76。

状态点（同一算法，取 16 套主题里最低的一档）：

| 点 | 取色 | 最低主题 | 比值 |
|---|---|---|---|
| 未验证 灰 | `--verify-gray` = `--text-2` | cappuccino | 3.88 |
| 已验证 绿 | `--verify-green`（浅色 `#1f7a3d`，其余 `--green`） | light | 5.14 |
| 验证失败 红 | `--red` | light | 3.40 |

**A4（焦点环在 cappuccino / obsidian 只有约 1.7:1）不采纳**：DS 自己也写了
「既有约定问题，不算本轮引入」。`--focus-ring` token 要动 `global.css` 的
16 套主题和所有用 `outline: 2px solid var(--accent)` 的组件，超出本轮范围。
记在这里，留给专门做焦点环的一轮。

## 3. 单 provider 的 pi 显示 provider 名（DS 3）

**已修**：`types.ts:214-224 providerGroups` 不再把单组的 provider 抹成空串；
新增 `types.ts:228 showsGroupHeads(groups)`（`groups.length > 1`），
「是否渲染组头」从数据判据改成渲染判据。
- 左栏：新增 `ResourcesTab.tsx:170-199 ResourceModelGroups` 算一次
  `showsGroupHeads`，`ResourcesTab.tsx:231` 的组头条件变成
  `showHead && group.provider !== ''`；provider 名仍在行悬停 /
  `aria-describedby`（`modelHint` 里本就单列一行）。
- 接入页：`ResourcesOverlay.tsx:353` 同一判据；provider 单列进详情块，
  详情那几行抽成 `ResourcesOverlay.tsx:302 AgentCardModelMeta`。

**已补**：
- `resources.test.ts` `providerGroups > 单 provider 的组保留 provider 名，只是不渲染组头`
  （顺带钉住折叠键仍是 `pi/bigmodel` 而不是 `pi/`）
- 同文件 `providerGroups > 两组以上才渲染组头`
- 同文件 `providerGroups > 只有一个 provider 时也只有一组；原生 agent 那一组的 provider 是空串`
  （替换原来断言 `groups[0].provider === ''` 的那条）
- `resources-tab-09.test.tsx` `ResourceAgentList > 单 provider 的 agent 不出组头，provider 仍在行悬停里`
- `resources-overlay-09b.test.tsx` `AgentCardModelMeta（接入页详情）> provider 单列一行，原生 agent（provider 为空）不出这一行`
- 同文件 `AgentCardModelMeta（接入页详情）> 来源、最近验证、失败原因与备注仍在`
  （抽组件不许把别的行弄丢）
- 同文件 `AgentCard（接入页）> 多 provider 的卡片按 provider 分组，组头带计数；单 provider 不出组头`
  扩了一段：单 provider 的 pi 也不出组头

**顺带（DS 2）**：组序 `localeCompare(right)` 改成 `localeCompare(right, 'en')`。
后端 `detected()` 用字节序，组序跟着系统 locale 走会让左栏与下拉对不上。
测试 `resources.test.ts` `providerGroups > 组序不跟随系统 locale`。

## 4. 键盘可达（DS A5）

**已修**：`ResourcesTab.tsx:302` `disabled={!graphOpen}` 改成
`aria-disabled={!graphOpen}`，`ResourcesTab.tsx:305-309` 的 `onClick` 在
未开对话图时直接返回（no-op）；`resources.css:314-325` 的置灰样式从
`:disabled` 换成 `[aria-disabled='true']`。这样整行仍可 Tab 聚焦、
读屏也念得到 `aria-describedby` 指向的「先打开对话图」。

**已补**：
- `resources-tab-09.test.tsx` `ResourceAgentList > 没开对话图时用 aria-disabled，不用 disabled，行仍可聚焦`
  （断言有 `aria-disabled="true"`、`class="resource-model-row"` 上没有
  `disabled` 属性、提示文本仍挂在 `resources-sr-only` 节点上）
- 同文件 `resources.css 的视觉纪律（修正轮 R1）> 未开对话图的模型行用 aria-disabled 而不是 :disabled`

## 5. `resolved_source`（DS 7）

**已修**：`ResourceModel` 追加 `resolved_source: String`（`#[serde(default)]`，
`src/resources.rs:110-115`），取值 `alias-table` / `identity`
（常量 `src/resources.rs:303-304`），空串按 `alias-table` 处理。
- `src/resources.rs:352 ensure_resolved_id` 的判据从「`resolved_id` 非空就不
  覆盖」换成「`resolved_source == identity` 才不覆盖」——静态值按当前别名表
  重算，于是下一版修正别名表时旧库跟着更新。
- `src/resources.rs:1246 merge_detected_models` 同一判据：重新探测只保留
  `identity` 来源的值。
- `src/resources.rs:1540-1545 apply_smoke_outcome` 成功分支把来源标成 `identity`。
- `src/resources.rs:1200 detected_model`、`:1312 add_model` 落 `alias-table`。
- 前端 `types.ts:19-22` 同步加字段（界面不显示；三个测试 fixture 补了默认值）。

**已补**：
- `resources::tests::task09b_resolved_source_lets_alias_table_fix_reach_old_stores`
  （静态值被新表覆盖 / 实测真身不被压回去，一条里两个方向）
- 改写 `resources::tests::task09b_legacy_store_backfills_resolved_id`：
  旧库里三条——无字段、有值但无来源标记（现在会被重算）、标了 `identity`（不动）
- `resources::tests::task09b_redetect_keeps_verified_resolved_id` 加上
  `resolved_source` 断言
- `resources::tests::task09b_smoke_success_overwrites_resolved_id_with_identity`
  加上「覆盖后来源 = identity」

**口径变化要写清**：旧库里「有 `resolved_id` 但没有 `resolved_source`」的条目
现在会被别名表重算。TASK-09b 没有发布过，本机库里不该有这种条目；真有的话，
上次冒烟的完整真身还在 `smoke_model` 里，不丢信息。

## 6. 许可与逃生门（DS 8）

**已修**：`NOTICE:25-49` 新增「THIRD-PARTY ASSETS ADDED BY AICROSS」段，
写明 simple-icons 的来源、CC0-1.0、三个图标各自的文件与 tag、读取日期，
以及 OpenAI 那条的来龙去脉（simple-icons#13944 从 master 移除、CC0 不可撤回、
风险是商标不是版权、删掉 `BRAND_MARKS.codex` 即可退回字母），末尾一条写三个
商标的指称性使用。段落加在 aicross 自己那一段里、`UPSTREAM NOTICE` 之前——
按 `AGENTS.md`，上游那段逐字保留，不能往里塞。`brand-marks.ts:12-19` 的文件头
改成指向 NOTICE 与这条逃生门测试。

**已补**：
- `resources-tab-09.test.tsx` `brand-marks 的逃生门 > 删掉 BRAND_MARKS.codex，徽标回退字母`
  （删键后 `agentEmblem` 返回 letters、渲染出的 markup 里没有 `<svg>`、
  class 是 `agent-badge-codex agent-badge-letters`、可见文字是 `CO`；
  `finally` 里还原，末尾再断言还原后确实是 `mark`）

## 7. 小项（DS 9、10）

**已修 越阶字号**：`resources.css:414` `.resources-page h1` 20px 改 18px
（六级阶梯的页标题档）。
**已修 间距**：`resources.css:477` `.res-dialog` `gap: 10px` 改 `12px`。
测试 `resources.css 的视觉纪律（修正轮 R1）> 页标题回到六级阶梯，弹窗间隙回到 4 的倍数`。

**已修 `BrandMark.title` 死字段**：删掉。徽标是 `aria-hidden`（旁边就是
agent 名），没有任何渲染路径消费它；品牌名改成每条上面一行注释，归属在
NOTICE 里。测试 `agentEmblem > 路径数据都在（漏抄一条会渲染成空徽标）`
去掉 title 断言，改成钉住表里就这三条。

**已修 真身前缀剥离**：`src/resources.rs:333 resolved_id_from_identity`
改成大小写不敏感（`Zai/glm-4.7` 配 provider `zai` 也剥）。测试
`resources::tests::task09b_resolved_id_from_identity_strips_only_matching_prefix`
覆盖 8 个边界：同形 / 大小写不同（两个方向）/ 前缀不符 / provider 为空 /
无前缀 / 只有前缀没有模型名 / 空真身。

**已修 `hintId` 撞 id**：`ResourcesTab.tsx:295` 改用 `useId()`。原来
`glm-4.7` 与 `glm-4-7` 会被规范化成同一个 DOM id，`aria-describedby` 指到
第一条。测试 `ResourceAgentList > 每行的 aria-describedby 是各自的 id，不会撞`
（两条只差一个字符的模型名，断言两个 id 不同且各自都有对应节点）。

**已修 报告「sm/md 同尺寸」**：不准。`resources.css:44-47` 的
`.agent-badge-md` 是 **20px**（接入页卡片头），`.agent-badge-sm` 是 18px。
任务书只写了 18px；这里保留 20px：接入页卡片头的名称字号更大，18px 的标志
在那一行偏小。本报告「实现 1」那句「`sm` / `md` 同尺寸」作废。

**顺带（DS 2）已补**：点一行模型插进对话图的 `@` 引用必须带 provider，
上一轮这条链路没有测试（静态渲染里 `onPick` 是 no-op）。payload 拎成纯函数
`types.ts:259 modelPickTarget`，组件里只剩一行转发。测试
`modelPickTarget > 把 provider 一起交出去，agent 缺省退回所在 agent 的 id`
与 `modelPickTarget > 拼出来的 @ 引用带 #provider`。
**限制**：这套前端用例没有 DOM（没装 jsdom / testing-library，本轮不加依赖），
点击本身触发不了，钉住的是 payload 与 `mentionRef` 的拼装，不是 click 事件。

## GLM 审查的增量

`tasks/review-09b-glm.md` 在本轮做到一半时才出现（`git status` 里从未跟踪，
第一次 `ls tasks/` 时确实没有）。读完之后逐条比对，结论是 GLM 的缺项清单里
有 4 条与 DS 重复、已在上面修完：

| GLM 条目 | 状态 |
|---|---|
| 2. `resolved_id_from_identity` 大小写不一致 / 无前缀保留整串，缺直测 | 与 DS 10 同，见第 7 节（已改成大小写不敏感 + 8 个边界断言） |
| 4. kimi 徽标深色 3.0:1 贴线 | 与 DS A3 同，见第 2 节（提亮到 `#8574E0`，最低一档 3.95） |
| 5a. `hintId` 消毒可撞出重复 DOM ID | 与 DS 10 同，见第 7 节（`useId()`） |
| 5c. `BrandMark.title` 注释称进 aria-label 实则从未使用 | 与 DS 9 同，见第 7 节（删字段） |

不重复的三条，处理如下。

### GLM 1（中低）原生 claude 没有真身覆盖路径 → 已改文档口径

**成立**。原生模型不给「验证」按钮，首派自动验证只对 `unverified` 生效
（native 直接 Skipped），而 `resolved_id` 的实测覆盖只在 `apply_smoke_outcome`
里发生——官方订阅下的 claude 走不到任何一条覆盖路径，静态别名表就是永久事实源。
`docs/PRODUCT.md:236` 原文「静态表只是起点，冒烟 / 首次派发核对到响应体
`model` 后以实测值覆盖」对原生 claude 不可达，是口径与实现不符。

**已修**：把那一句改成分两档写——第三方端点（pi 的各 provider、挂了
`ANTHROPIC_BASE_URL` 的 claude）核对后覆盖并记 `resolved_source`；原生端点
没有覆盖路径，真身以静态表为准，表漂了只能改表。同时把本轮新增的
`resolved_source` 口径一并写进去。`docs/PRODUCT.md` 是 CRLF，按字节改，
只换了一句话，改后 CR 239 / LF 245 与改前一致。

**没做**：给原生行加一个手动「核对」动作。那是新交互，不在本轮 7 组里。

### GLM「最该补的测试 1」`add_model` 手动录入 → 已补

这条路径（`src/resources.rs:1312`）原来零测试，GLM 说得对：把那两行删掉现有
测试全绿。已补 `resources::tests::task09b_add_model_resolves_claude_alias`
（手动录入 `sonnet` 落 `claude-sonnet-5` + `resolved_source = alias-table`；
非别名的完整 ID 原样）。

### GLM「最该补的测试 3」组件级折叠隔离 → 已补

原来只有 `groupKey` 的纯函数断言，没走渲染路径——组件里把折叠键写成
`group.provider` 也不会红。已补 `resources-tab-09.test.tsx`
`ResourceAgentList > 折叠状态不跨 agent 串：同名 provider 挂两个 agent`
（pi 与 kimi 各挂一个 `zai` 组，只展开 `pi/zai`，断言 `pi-new` 出现、
`kimi-new` 不出现、同屏同时有「收起」与「展开」两行）。

### GLM 3（低，条件性）对外分发前的商标检查 → 记档，未改代码

GLM 的判断与 DS 一致：CC0 解决的是路径数据的版权，#13944 撤下的是商标许可，
私用桌面工具的指称性使用成立。GLM 多提了一个条件：**一旦对外分发安装包**，
性质就从私用指称滑向「分发他人商标的界面资产」。本轮把这条记进
`NOTICE` 的新段与下面的「待你拍板」，代码不动——删 `BRAND_MARKS.codex`
一行即可退回字母，已有测试保证。

### GLM 明确不记为缺陷、但本轮仍改了的两条

- `localeCompare` 不带 locale（GLM 判「单机桌面工具，不算问题，不记」）：
  DS 判为「与后端字节序不一致」，本轮按 DS 改成了 `localeCompare(x, 'en')`
  并补了测试。两家分歧，采纳更严的那一边，代价是一个参数。
- 模型行 `disabled`（GLM 判「现有交互下可接受，不记为缺陷」）：
  DS A5 判为键盘完全不可达，`fix-09b-r1.md` 第 4 条点名要改，已按 DS 改成
  `aria-disabled`。

### GLM 与 DS 对同一处的分歧，本轮的取舍

- **接入页 `md` 徽标 20px vs 任务书 18px**：两家都提了，都判「极低 / 偏差」。
  本轮保留 20px（理由见第 7 节末），改的是报告里那句不准的「sm / md 同尺寸」。
- **`localeCompare`**：见上，采纳 DS。

## 不采纳

- **DS A4 焦点环 token**：既有约定问题，改动面是全部 16 套主题，超出本轮范围
  （理由见第 2 节末）。
- **DS 9「pi / kimi 图标与仓库既有 `icons-inline` 资源两套并存」**：任务书写死
  「pi 用 π 字形」「kimi 用 simple-icons」，这一条是要改任务书口径，不是修 bug。
  事实记在这里：`src-ui/src/icons-inline/pi.svg` 与 `icons-inline/kimicode.png`
  是启动台在用的另一套，同一个 agent 在同一窗口里确实是两套标志。
- **DS 2「`expanded` 只在首次 load 初始化」**（新 agent 默认折叠、切 tab
  回来重置）：不在 `fix-09b-r1.md` 的 7 组里，也不是本轮引入的，没动。
- **DS 3「把「左栏」写进 `docs/PRODUCT.md` 的状态点那一条」**：没做。
  `docs/PRODUCT.md` 本轮只改了一句（GLM 1 的口径，见上），没顺手动别处。

## 验证（全部实跑，输出原文；下面是加完 GLM 增量后重跑的一遍）

```
$ cargo check
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.02s

$ cargo check --all-targets
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.29s
```

零 warning（两次都是；`--all-targets` 含 test target）。

```
$ cargo test

test result: ok. 325 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 9.08s

$ cargo test task09b

running 7 tests
test resources::tests::task09b_claude_aliases_resolve_to_known_model_ids ... ok
test resources::tests::task09b_resolved_id_from_identity_strips_only_matching_prefix ... ok
test resources::tests::task09b_resolved_source_lets_alias_table_fix_reach_old_stores ... ok
test resources::tests::task09b_redetect_keeps_verified_resolved_id ... ok
test resources::tests::task09b_add_model_resolves_claude_alias ... ok
test resources::tests::task09b_legacy_store_backfills_resolved_id ... ok
test resources::tests::task09b_smoke_success_overwrites_resolved_id_with_identity ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 329 filtered out; finished in 0.01s
```

Rust 从 322 涨到 325（新增 3 条，其余是改写既有用例）。

```
$ cd src-ui && npx vitest run --configLoader native --pool threads
 RUN  v5.0.0 D:/work/aicross-ui/src-ui


 Test Files  26 passed (26)
      Tests  390 passed (390)
```

前端从 369 涨到 390。

```
$ cd src-ui && npm run build
dist/assets/main-DkgW5Fz2.js                                   441.20 kB │ gzip: 215.40 kB
dist/assets/xterm-C7jn2Fqg.js                                  465.79 kB │ gzip: 123.12 kB

✓ built in 612ms
```

反向验证（对比度那条测试不是假绿）：

```
$ # 把 resources.css 里 kimi 的深色调临时改回 #6E56CF
$ cd src-ui && npx vitest run --configLoader native --pool threads -t "对比度"
 FAIL  src/resources/__tests__/resources-tab-09.test.tsx > 徽标与状态点的对比度 ≥ 3:1（修正轮 R1） > 四家徽标在每套主题的面板底色上都 ≥ 3:1
AssertionError: cappuccino / kimi #6e56cf on #252525: expected 2.84 to be greater than or equal to 3
      Tests  1 failed | 2 passed | 386 skipped (389)
$ # 改回 #8574E0 后转绿
```

`npx eslint src/components/resources src/resources`：**1 error**，与改前同一条
同一处（`ResourcesTab.tsx:70` 既有的 `react-hooks/set-state-in-effect`）。
本轮没有新增 lint 问题。

## 约束核对

- Rust 只改 `src/resources.rs`：`git diff --name-only` 里没有 `src/channels/`、
  `src/dispatch.rs`。契约仍是只追加（`resolved_source` 带 `#[serde(default)]`，
  旧字段一个没删）；`ensure_resolved_id` / `merge_detected_models` 的**判据**
  变了，这是本轮有意的行为修正，见第 5 节。
- 换行：改动的 12 个文件里 11 个是 LF（CR 字节数 0），`docs/PRODUCT.md` 仍是
  CRLF（239 / 245，与改前一致）；`git diff` 逐文件看过，没有整文件换行翻转。
- 没起窗口、没发真机请求、没 commit、没 push、没写仓库以外的位置。
- 界面文案没有大改：新增的可见字符串只有接入页详情块里的 `provider：<名字>`
  一行。

## 仍然待你拍板 / 没做的事

1. **Codex 用 OpenAI 标志**：本轮把归属与逃生门都补齐了（NOTICE + 测试），
   但要不要留这个标志仍是你的决定。现在删 `BRAND_MARKS.codex` 一条即可，
   有测试保证退回字母不出错。
2. **kimi 徽标改成 `#8574E0`**：与对话图色点 `#6E56CF` 不再是同一个值
   （同色相、更亮）。要严格「徽标与色点同色」，就得改 `graph/vendor.ts` 的
   kimi 色——那会波及对话图，不在本轮范围，没动。
3. **对外分发安装包前要再决定一次 OpenAI 标志**（GLM 3）：私用口径下指称性
   使用成立，分发出去性质会变。建议记进发布前检查：分发前删
   `BRAND_MARKS.codex` 或换成文字标签。
4. **原生 claude 的真身没有观察纠正手段**（GLM 1）：本轮只把文档口径改准了。
   要真能纠正，得给原生行加一个手动「核对」动作，是下一期的事。
5. **界面仍然只做了代码、样式与静态渲染核对，没起窗口**（约束要求）。
   18px 标志配 12px 模型名的观感、单 provider 的 pi 少了组头之后左栏够不够
   清楚，需要真机看一眼。
