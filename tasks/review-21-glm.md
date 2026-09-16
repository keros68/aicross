# TASK-21 审查结论

**总判：可交付。** 四节均闭合，硬约束（加载路径不发任何真实模型请求）成立；以下均为低严重度的潜在隐患与补测建议，不阻塞交付。

## 问题清单（按严重度排序）

1. **`src-ui/src/resources/types.ts:97` — `cliVersionNumber` 对 `v` 前缀版本号会截掉主版本号（低，潜在）**
   正则 `/\b\d+(?:\.\d+)+…\b/` 的行首 `\b` 在「字母紧跟数字」处不成立：`\b` 要求词边界，`v` 与 `2` 都是词字符。复现：`cliVersionNumber('v2.1.272 (Claude Code)')` 返回 `'1.272'`（匹配从 `.` 后的边界重新开始）。当前四家真实值（`2.1.272 (Claude Code)` / `codex-cli 0.154.0` / `0.42.0` / `0.85.1`）都不带 `v` 前缀，四格测试全过；但 claude/codex 的输出格式不受我们控制，一旦加 `v` 前缀界面会静默显示错版本。建议：数字前加负向断言（如 `(?<![A-Za-z])`，WebView2/Node 均支持）或改用 `ver(?:sion)?\s*` 类锚定，并补一格 `v` 前缀测试。

2. **`src/resources.rs:564-567` — 重判 else 分支对「任何带 `smoke_failure` 的非原生记录」无条件改判 failed，不看当前 `verification`（低，收紧建议）**
   任务书原文是「命中……翻案；**没命中的保持原样**」。现实现里未命中的记录会被强制 `verification=failed` + `status=冒烟失败`，即使该记录当前已是 `verified`。复现条件：一条非原生模型记录同时有非空 `smoke_failure` 和 `verification=verified`。我核过现有全部写路径（`apply_smoke_outcome` 成功即清空 `smoke_failure`，`merge_detected_models` 成对克隆），这个状态在当前代码里造不出来，所以行为等价；但手工修过的库或将来新增写路径若留下「verified+旧失败」会被每次加载静默降档。建议：else 分支加 `model.verification != VERIFICATION_VERIFIED` 守卫（现状行为完全不变，仅消除隐患）。

3. **`src-ui/src/resources/__tests__/resources-tab-09.test.tsx`（fixture，60-104 行）— 左栏渲染测试没有带斜杠前缀的 kimi 模型，行内前缀去除的左栏接线未被渲染断言钉住（低，补测）**
   `modelDisplayName` 在左栏的调用点是 `ResourcesTab.tsx:440`，但 tab 测试 fixture 里 kimi 的 id 都是 `kimi-ok` 这类无斜杠词，claude 是 `fable`——没有任何一行走进前缀分支。目前靠纯函数测试 + 接入页卡片渲染测试（`resources-overlay-09b.test.tsx:81-87` 断言 `>k3-256k</span>`）覆盖同一函数；若左栏那行退回 `model.id`，现有测试不会红——正是本仓刚吃过的「只测纯函数、漏接线」形态。建议：往 tab 共享 fixture 加一条 `agent:'kimi', id:'kimi-code/k3-256k'`，断言可见文本含 `k3-256k` 且不含 `kimi-code/`。

4. **`src/resources.rs:543-547` — 重判比 `channels::known_model_route` 严一档：要求失败串里「响应的 provider」也等于路由表 provider（提示）**
   `KNOWN_MODEL_ROUTES` 的第三元只是模型名，channels 的 `known_model_route`（`src/channels/mod.rs:12-14`）不比对响应 provider；重判闭包里多了一条件 `*provider == actual_provider`。当前唯一路由（deepseek 两边同 provider）下两者一致；将来若出现跨 provider 路由，实跑冒烟能过、旧失败记录却翻不了案。方向保守，符合「没命中的保持失败」，仅记录不对称。

5. **`src/resources.rs:299` — 重判结果只在内存，落盘要等下一次任意写（提示，非缺陷）**
   `load()` 每次重算、`get_resources_at` 仅在额度过期时才回写。盘上 `resources.json` 长期保持旧值，但所有消费方（左栏、接入页、`detected()`、`verify_on_first_use`）都走 `load()`，行为正确；与 `ensure_verification`/`ensure_resolved_id` 同一既有约定。无用户动作需求，符合「显示层归一、不要求重新探测」。

## 核过没问题的要点

- **第 1 节闭合**：左栏（`ResourcesTab.tsx:205-207`）与接入页（`ResourcesOverlay.tsx:473-475`）都只显示 `cliVersionNumber(agent.version)`，`--version` 原样输出保留在 `title`。四家真实值＋回退各有测试（`resources.test.ts:93-106`）；`未检测`/`已检测` 哨兵值无数字点串、原样透传，不会坏。接线有渲染级断言：tab `test:171-175`（`title="codex-cli 0.154.0">0.154.0</span>` 且可见文本无 `codex-cli`），overlay `test:81-87`。
- **第 2 节闭合**：`modelDisplayName`（`types.ts:224-234`）按「归一化后与 agent 显示名同义才去前缀」的通用规则（`kimi-code` ≡ `Kimi Code`；pi 的 `deepseek/…` 不动），有命中/不命中两格；派发词仍是 `model.id`（`modelPickTarget` 未动）。`modelDetailLines`（`types.ts:425-444`）前四行固定名字/模型 ID/状态/来源，`resources.test.ts:114-131` 用 `resolved_id == id` 的 codex 模型断言「模型 ID」行照样在；可选行顺序与任务书一致。`AgentCardModelMeta`（`ResourcesOverlay.tsx:339`）与左栏悬停（`modelHint` → 同一函数）确认共用唯一构造函数；`model.source` 在前端的唯一显示消费者就是它。
- **第 3 节闭合（老串归一）**：Rust 侧全部写入点收口到 `source_value`/`source_read`（codex 954、kimi 1019、pi 1140、claude help 1384、静态表 1392、手动申报 1712）；`shorten_home_path`（1532-1543）带 `([\\/]|$)`，路径止于主目录也缩成 `~`，且有专测（`task21_home_shortening_also_handles_paths_ending_at_home`）。显示层 `normalizeModelSource`（`types.ts:243-261`）三种老形状各一格测试全过，另覆盖了任务书没列的第四种老值 `静态别名表 (日期)`、无日期兜底（`· 日期未知`）与空值（`未记录`）；新形状幂等。`merge_detected_models` 的申报识别同时认 `申报(` 与 `来源：用户申报 · ` 两种前缀（resources.rs:1640-1642），手动条目不会被误标「未在事实源中」。
- **第 4 节闭合且不违反硬约束**：重判只挂在 `load()`（resources.rs:299），闭包内只有 Regex、静态表和 `Local::now()`，无任何进程启动；冒烟相关函数未被改动。顺序 `ensure_verification → rejudge → ensure_resolved_id` 正确：重判写的 `resolved_source=identity` 会被 `ensure_resolved_id` 跳过，真身不被静态表回退。翻案条件收紧到位：失败串正则 + 路由表三元组 + 库记录自身 `provider`/`id` 四者全对上才翻（`src/resources.rs:535-551`）；实测存量记录形状 `真身不符：请求 deepseek/deepseek-v4-flash，响应 deepseek/deepseek-flash` 与 channels 754 行的实际产出逐字同形，正则命中。普通冒烟失败写回走 `VERIFICATION_FAILED`（1938-1948），有专测。
- **既有口径未破坏**：`unverifiedCount` 仍只数 `unverified`；左栏折叠 `splitUnverified` 未改——failed 行从折叠桶移到常显是任务书要的数据归类结果，行结构/状态点组件（`VerificationMark`、`ResourceModelRow`）本身零改动。G1「登录受限默认不勾」只看 `loginOf`（`dispatch.ts:1048-1059`、`DispatchOverlay.tsx:137-156`），与验证档无关，未受影响。`verify_on_first_use` 仍只对 `unverified` 动手——存量 failed 记录首次派发不再重发请求，这正是 09-15 已定「失败不自动重试」规则的落地，不是回归。`verificationHint` 的 failed→红字、`modelStatusSuffix` 的「（验证失败）」后缀沿用同一 `verificationOf`，口径一致。
- **测试能红、非同义反复**：纯函数测试全是精确串断言；重判测试走真实 `load()` + 临时盘（3781-3786），并追加验证「重新探测不退回」（3797-3815）；REPORT 记录了逐条去实现变红的对照过程，与测试形态吻合。
- **文案**：新串（`来源：`、`未记录`、`日期未知`、`用户申报`、四个状态词）中性说明体、全角标点、半角只留在路径/日期/ID；`·` 分隔与既有「未验证 N 个 · 展开」一致。备注串「按已知路由别名重判（日期）」为任务书明文规定，且 `note` 不渲染进界面（overlay 测试断言无「备注：」行）。`docs/MANUAL.md` 既有词条（未验证/已验证/验证失败/原生）与新显示无冲突。
- 换行与未动文件：无法逐字节核验（只读环境无 shell），但 git status 显示 `src/server.rs`、`src/main.rs`、`src-ui/src/tauri.ts` 均未修改，与 REPORT 声明一致。