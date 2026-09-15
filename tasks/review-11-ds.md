## 总判

**可交付但有未闭合项** —— 逻辑面没查到回归（按显示名匹配的点、Rust 预设改名的前端硬编码、旧图/旧 `presets.json` 都核过，能读、没断），但有两类未闭合：Rust 侧还有 2 处**用户可见**错误串含本期明令消失的「留痕」；两处用户可见位置仍显示用户预设置信原名、与术语表「自定义模式」冲突，而术语表也没把这两处列进「出现位置」。另有一处报告与代码自相矛盾（写的是旧行为，代码已兼容）。

---

## 问题（按严重度）

### 1【中】Rust 用户可见错误串漏网「留痕」×2
- `src/dispatch.rs:3328` 旧＝新＝`format!("创建留痕目录失败：{error}")`；`src/dispatch.rs:3338` 旧＝新＝`format!("序列化留痕失败：{error}")`。
- 问题：违反 R5 与术语表（`docs/MANUAL.md:175`「派发记录」/178「界面上不出现的词：留痕…」）。这两串会进界面：`ensure_log_dir` 被 `append_swarm_record`（:726，错误进**合并结果行/结论**）与派发启动（:1499，命令错误进界面）调用；`serialize_frontmatter` 被 :743 调用。
- 复现：项目目录或 `~/.aicross/runs` 不可写 → 结果页出现「创建留痕目录失败：…」。
- 建议：改为「创建派发记录目录失败」「序列化派发记录失败」；本期其余 `留痕→派发记录` 已改（:761/763/1168/1170/1186/1261/1265/1267/1309/1312），只漏了这两处（`scripts/copy-audit.py` 只扫 `src-ui/src`，Rust 靠 grep，属遗漏）。

### 2【中】「盖预设」菜单用原始预设名，用户自建预设在两处两词
- `src-ui/src/components/center/graph/GraphTab.tsx:641` 旧＝新＝`{preset.name}`。
- 问题：同一概念与右栏下拉不一致 —— 下拉走 `presetDisplayName()`（`OrchestrationPanel.tsx:227`）显示「自定义模式 · 名称」，这里显示裸名。
- 复现：存一个名为「手测三家流水线」的预设 → 右栏显示「自定义模式 · 手测三家流水线」，图上「盖预设」菜单显示「手测三家流水线」。
- 建议：菜单同用 `presetDisplayName(preset, userPresetCount)`；并把该位置补进术语表（见问题 5）。

### 3【中】图上「当前节点」行三套词
- `src-ui/src/components/center/graph/GraphTab.tsx:763` 旧＝`当前节点：{nodeLabel(endNode)} · {stampName ?? '手动'}` 新＝`… · {stampName ?? '自定义'}`。
- 问题：`stampName` 是 `recordStamp(..., preset.name)`（:376、`OrchestrationPanel.tsx:148`）记下的**原始名**：内置→「经济模式」（可接受）、用户自建→用户起的名（应为「自定义模式」/「自定义模式 · 名称」）、没盖过预设→「自定义」（术语表无此词，与「自定义模式」近义撞车）。
- 复现：用自建预设「手测三家流水线」盖章 → 该行显示「… · 手测三家流水线」；未盖章节点显示「… · 自定义」。
- 建议：抽一个「编排来源」显示函数统一走 `presetDisplayName`，未盖章那一路要么隐去要么在术语表里定词。

### 4【低】报告与代码矛盾（旧图「测试红」）
- `tasks/REPORT-11.md:219` 写「旧图的该件会被读成『已跳过 / 待合并』而不是 `test_failed`…如需兼容旧文本再加一行识别」，但 `src-ui/src/graph/swarm.ts:469` 已是 `if (last.includes('验收未通过') || last.includes('测试红')) return 'test_failed';`（:468 有注释）。
- 问题：报告陈旧，结论方向偏保守（代码是好的）；同时 :126/:219 引用的 `swarmMergeStatusOf` 在仓库里不存在（实际是 `swarmRecordedResult`，:458），建议一并改正，否则后来人按报告去找兼容点会误判。

### 5【低】术语表与实际显示不符、且漏列出现位置
- `docs/MANUAL.md:169`「把资源纳入清单 | 接入（状态：已接入 ↔ 移除）」：`移除` 在 `ResourcesOverlay.tsx:453` 有；`已接入` 界面上已不存在（左栏/接入页显示的是 未验证/已验证/验证失败/原生，`resources/types.ts:112-116`），只剩机器值 `resources/types.ts:80` 与设置页占位符 `SettingsModal.tsx:649`「默认取已接入的低档模型」。
- `docs/MANUAL.md:157`「用户自建预设…出现位置：右栏预设下拉、编辑编排的默认名」漏了 `GraphTab.tsx:641`（盖预设菜单）与 `GraphTab.tsx:763`（当前节点行）。
- 建议：按实际显示改这两行，或按问题 2/3 改代码后再对齐。

### 6【低】同一要点说两遍：「未接入资源（未找到 resources.json，未接入资源）」
- `OrchestrationPanel.tsx:289`、`GraphTab.tsx:771` 的「未接入资源（{modelsError}）」＋ `src/orchestration.rs:608`（及测试断言 :1337）的 `"未找到 resources.json，未接入资源"`。
- 问题：R4「同一要点只说一遍」；两边本轮都改成了同一个词，重复没消掉。
- 建议：Rust 侧串只留事实（如「未找到 resources.json」），前缀由前端出。

### 7【低】`docs/MANUAL.md:100` 「件与件是分工不是擂台」
- 问题：正文里预设简称「擂台」，与本仓自己的规则（`scripts/copy-audit.py:79` 的 `擂台(?!模式)`）和 R1「预设一律『X模式』」不一致；md 不在脚本扫描范围，所以没被体检抓到。
- 建议：改「不是擂台模式」或改成不含预设名的说法（如「件与件是分工不是竞争」）。

### 8【信息】体检脚本有盲区，报告「剩余 4 处」不是完备清单
- `scripts/copy-audit.py:39` 的模板串正则 `` `([^`\\]*[HAN][^`\\]*)` `` 里 `[^`\\]` 排除反斜杠 → **含 `\n` 等转义的模板串整条被跳过**。例：`src-ui/src/graph/context.ts:72` 的 `stdout 摘要：… || '—'` 就没进清单（单引号正则同样排除反斜杠）。不影响交付结论（这些串都不是界面文案），但计数别当全覆盖。
- 另：`src-ui/index.html:6` 的 meta description 用了英文式破折号「—」（审计只扫 `src-ui/src`）；en/ja/ko/fr/de/es/pt/ru/vi 的 `...`、`Add notes...`、EchoBird 推荐语未同步（本期明确只做中文，报告已声明）。

---

## 核过没问题的点

- **预设名一致性**：`src/orchestration.rs:307-441` 六个内置名（共享/经济/盲审/擂台/评审/蜂群「模式」）与 `docs/MANUAL.md:156` 逐字、逐序一致；前端兜底 `BUILTIN_SWARM_PRESET.name='蜂群模式'`（`swarm.ts:65`）与后端同 id，`withSwarmPreset` 按 **id** 去重（`swarm.ts:84-88`），不靠名字；id `builtin-*` 未动。
- **未改过头**：机器值 `STATUS_PENDING/FAILED/CONNECTED`、字段名 `smoked_at/smoke_failure/smoke_model`、命令名 `smoke_resource_model`、i18n key（`menu.copy_session_id`、`task.notes_placeholder`、`heatmap.title_empty`…）、`graph/notation.ts` 的中文→机器值映射、className/id 均原样；`FUTURE_PRESETS` 只有 `<option value>` 从期次串改成 `future-redblue/future-chat`，全仓 grep 无引用（`OrchestrationPanel.tsx:29-30,231` 是唯一定义/使用处），无行为影响。
- **按显示名匹配的判断**：`DispatchOverlay.tsx:458` 仍比对 `presetNameOf(...) === '经济模式'`，该名未改，且后面还有角色兜底（`plannedRoles.has('出方案'&&'执行'&&'验收')`）；Rust 侧一律按 id 查（`orchestration.rs:500/917/1185…`）；`stamp.ts` 视角文本按 `preset.builtin && PRESET_NODE_TEXTS[preset.id]` 取（`stamp.ts:76-77`），不看名。
- **状态词 ↔ 动作词成对**：已接入↔移除、已并入↔合并、已采纳↔采纳/取消采纳、已选定↔选定（`DispatchBits.tsx:88`）、未验证/已验证/验证失败/原生↔验证·重新验证、已跳过、待核对、待派/运行中/完成。
- **旧数据仍可读**：`swarm.ts:469` 同时识别新「验收未通过」与旧「测试红」；`RECONCILE_UNKNOWN_TEXT`（`dispatch.ts:786`）与「（已切备选：…）」标记（`dispatch.ts:213/216`）未改，旧图判定不变；`presets.json` 用户预设按原样解析（`orchestration.rs:620-635`），旧名只影响显示、不影响读取；`STATE.md`/`.dispatch/` 文件名与 frontmatter 未动。
- **测试**：仓库确有 28 个测试文件（与报告 `28 passed` 一致，含新增 `graph/__tests__/glossary-11.test.ts`）；该用例的切片（`fn builtin_presets()` → 首个行首 `}`）、正则 `name: "…".into()`、顺序断言与三种 `presetDisplayName` 输出逐行核对成立；`?raw` 引外部路径（`docs/`、`src/`）与既有 `fixtures/prompts/*.txt?raw` 同款，路径深度正确；改文案处的断言已同步（`presets-05b-r2.test.tsx:167/171` 选定/已选定、`consistency-06b.test.tsx:314`、`idle-and-tail-07b.test.tsx:234` 派发记录核对、`swarm-08b.test.tsx:315/482` 验收未通过），`swarm-08c.test.tsx:146` 仍断言 `'测试红' → test_failed`（旧图兼容）也成立。
- **文案体**：`src-ui/src` 非测试代码里，本期涉及的中文串无第一/第二人称、无 emoji、无英文破折号、无半角标点混用（zh-CN/zh-TW 全量 grep `[中文][,;:?!]`、`...`、`——` 均为 0，仅剩注释）；已改文件里 `留痕/对账/口径/判定/闸门/落库/冒烟/添加/评审团/（我的）/期次` 的残留全部落在注释、测试数据、机器值（`resources/types.ts:81`）或刻意保留的兼容串（`swarm.ts:469`），无用户可见漏网。
- **zh-TW 已同步**（`copy_session_id`、`connecting`、`notes_placeholder`、`detect_help_tip`、`switch_model_tip`、`title_empty` 等 key 与 zh-CN 一一对应，key 未改名）。

## 我没能验证的

只读、无 shell：`vitest`、`npm run build`、`cargo check/test` 与 `scripts/copy-audit.py` 的前后计数（77→4、255→180）**没有复跑**，测试与构建结果按报告采信，上面的静态结论依据的是仓库现有代码与文档原文。