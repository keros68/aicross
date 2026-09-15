# REPORT-11 界面文案与命名统一

对应 `tasks/TASK-11-naming.md`。分支 `ui/phase11-naming`（基于 main 269b9d1），未 commit、未 push。

## 规则编号（本报告用）

按 `AGENTS.md`「界面文案与命名规范」与 `~/.codex/guidance/copywriting.md` 编号：

- R1 同类项一套模板：预设「X模式」、自建「自定义模式」、规划中「X模式（规划中）」；角色两字。
- R2 一个概念一个词：派发 / 合并 / 采纳 / 重派 / 叫停 / 接入 / 移除 / 验证。
- R3 按钮 = 动词短语 2–6 字；标签 = 名词短语；状态词与动作词成对。
- R4 说明句 = 事实 + 后果，一句说完；同一要点只说一遍；条件→能力「……后可……」；能删则删。
- R5 不用内部词（留痕、对账、口径、判定、闸门、门禁、落库、wire、G1–G4、任务编号）。
- R6 标点全角；半角只留在路径、命令、代码标识符、版本号。
- R7 不拟人化、不用 emoji、不用英文式破折号。
- R8 中性说明体：不用第一 / 第二人称、口语与劝导。

## 术语表

写在 `docs/MANUAL.md` 末尾「术语表」一节（概念 → 唯一用词 → 出现位置）。摘要：

| 概念 | 唯一用词 |
|---|---|
| 内置预设 | 共享模式 / 经济模式 / 盲审模式 / 擂台模式 / 评审模式 / 蜂群模式 |
| 用户自建预设 | 自定义模式（多个时「自定义模式 · 名称」） |
| 规划中的预设 | 红蓝模式（规划中） / 群聊模式（规划中） |
| 角色 | 出方案 / 执行 / 验收 / 验证 / 拆分 / 顾问 / 仲裁（已一致，未改） |
| 动作 | 派发 / 合并 / 采纳 / 重派 / 叫停 / 接入 / 移除 / 验证 |
| 擂台里选一路继续 | 选定（已选定） |
| 派发写下的文件 | 派发记录（原「留痕」） |
| 崩溃后按记录恢复 | 核对（原「对账」） |
| 蜂群件验收命令非零 | 验收未通过（原「测试红」） |
| 新建阶段 / 节点 / 图 / 任务 | 新建（原「添加」） |
| 记录一条前提 | 记录（原「添加」） |

## 体检脚本

`scripts/copy-audit.py`（移植自 skillmanager，扫 `src-ui/src`，规则按本仓 AGENTS.md 扩了内部词、任务编号、冒烟 / 添加、预设命名四组；只报告不改；默认跳过 `__tests__` 与非中文 i18n，`--tests` 连测试一起扫）。

| 范围 | 改前 | 改后 |
|---|---|---|
| 默认（不含测试） | 77 处 / 14 文件 | 4 处 / 4 文件 |
| `--tests` | 255 处 / 33 文件 | 180 处 / 24 文件 |

改后默认扫描剩余 4 处，逐条说明：

1. `DispatchOverlay.tsx` `—`：用量表里 tokens 为空时的占位符（`'—'`），不是破折号，保留。
2. `GraphTab.tsx` `—`：同上，节点行 in/out tokens 为空的占位符，保留。
3. `SettingsModal.tsx` `—`：前提「记录」日期为空时的占位符，保留。
4. `resources/types.ts` `STATUS_FAILED = '冒烟失败'`：`resources.json` 里 `status` 字段的机器值，与 Rust `STATUS_PENDING/STATUS_FAILED` 一一对应，落在用户磁盘上；界面不直接显示它（显示走 `verificationLabel` 的 未验证 / 已验证 / 验证失败 / 原生）。改值等于改存储格式，需要迁移，超出本期「只改显示名」的范围，保留。

`--tests` 剩余 176 处全部在测试文件：测试名里的期次 / 修正轮编号（06B、R1、G1…）、测试数据（`'你好'`、`status: '待冒烟'`）、注释。不是用户可见字符串，不改。

Rust 侧用 grep 补盘点：`src/server.rs` 返回前端的错误串均为英文，没有中文命中（中文只在测试数据里），未改。

## 逐文件改动

### 前端 `src-ui/src`

**components/right/orchestration/OrchestrationPanel.tsx**
- 预设下拉 `{p.name}{p.builtin ? '' : '（我的）'}` → `presetDisplayName(p, userPresetCount)`：内置预设原名；自建预设显示「自定义模式」，多个时「自定义模式 · 名称」（R1）
- `FUTURE_PRESETS = ['红蓝', '群聊']` + 选项文字「{n} · 后续期次」→「红蓝模式（规划中）」「群聊模式（规划中）」（R1、R5：去掉「期次」）；`key`/`value` 改用固定 id `redblue` / `chat`
- 「尚未接入资源（…）」→「未接入资源（…）」（R4）

**graph/label.ts**
- 新增 `USER_PRESET_LABEL = '自定义模式'` 与 `presetDisplayName()`，供下拉与测试共用（R1）

**components/center/graph/GraphTab.tsx**
- 当前节点行「· 手动」→「· 自定义」（R1、R2：与「自定义模式」同词）
- @ 弹层「尚未接入资源…，可直接手输 agent/模型」→「未接入资源…，可手输 agent/模型」（R4）

**components/right/orchestration/PresetBuilder.tsx**
- 「添加节点」→「新建节点」、「添加阶段」→「新建阶段」（R2：与「新建对话图」「新建」任务同词；「接入」不适用于节点）
- 应用时的默认预设名「手动编排」→「自定义模式」（R1）
- 「内置预设不可改，保存会存成新预设。」→「内置预设不可改，保存后存为自定义模式。」（R1、R4）

**components/center/graph/DispatchOverlay.tsx**
- 「单步小任务，外派固定足迹不划算，建议直接 @ 一个执行者」→「单步小任务，建议直接 @ 一个执行者」（R4、R5：「固定足迹」是内部说法）
- 「…thinking 已调为中；可修改。」→「…thinking 已调为中。」（R4：旁边就是下拉框）
- 「单写者产物已在项目目录，标记采纳即可。」→「…采纳即可。」（R2）
- 「冲突与测试红交你裁决，不自动重派。」→「冲突与验收未通过的件由用户裁决，不自动重派。」（R5、R8）
- 「全部结束后才能合并。」→「全部结束后可合并。」（R4）
- 「这些件都已并入，没有要合并的了。」→「全部件已并入。」（R4）
- 「系统不判定分歧，双方由用户选」→「甲乙双方由用户选，分歧交第三家裁决」（R5、R4 正着说）
- 折叠区标题「留痕」→「派发记录」（R5）
- 按钮「交人裁决」→「汇总裁决」（R3）

**components/center/graph/DispatchBits.tsx**
- 擂台结果卡按钮「选它」/「已选中」→「选定」/「已选定」（R3：状态词与动作词成对；与「采纳」区分，两者可同时出现在同一张卡）

**components/center/graph/first-run.ts**
- 「派发留痕写在它的 .dispatch/ 里。」→「派发记录写在它的 .dispatch/ 里。」（R5）

**components/common/SettingsModal.tsx**
- 「已建图「X」：到中栏对话图起始页打开并派发，结论回本页自动并列」→「已建图「X」，在中栏对话图起始页打开并派发；结论自动回到本页」（R4）
- 归类模型 title「归类动作尚未接入」→「归类未接入」；说明「尚未接入，当前不可用」→「未接入，当前不可用」（R4）
- ACP 说明「当前只有 Kimi 使用长驻运行；Claude Code 与 Codex 仅探测适配器。ACP 下只读节点允许读类请求，拒绝写入与执行；一次性 Kimi 不受此策略限制。」→「当前只有 Kimi 长驻运行，Claude Code 与 Codex 仅探测适配器。长驻下只读节点只允许读类请求；一次性 Kimi 不受限。」（R4）
- 分组标题「外发与留痕」→「外发与记录」；开关「项目留痕」→「派发记录」（R5）
- 前提输入旁按钮「添加」→「记录」（R2、R3：前提条目本身叫「记录 {日期}」；「接入」不适用于前提）

**components/resources/ResourcesOverlay.tsx**
- 按钮 / 对话框标题「手动添加 agent」→「手动接入 agent」；错误「添加 agent 失败」→「接入 agent 失败」（R2）
- 按钮「手动输入」→「手动接入模型」；对话框标题「手动输入：X」→「手动接入模型：X」；错误「添加资源失败」→「接入模型失败」（R2）
- 两个对话框的确认按钮 `label="添加"` → `label="接入"`（R2、R3）

**components/center/SkillsPanel.tsx**
- 「添加技能市场」×2 →「接入技能市场」；确认「添加」→「接入」；「还没有添加任何市场。」→「还没有接入市场。」（R2）
- 「兼容 Codex 插件市场规则:仓库需含 …」半角冒号 → 全角（R6）

**components/right/orchestration/ExplainOverlay.tsx**
- 「只留你点名的那些上游节点的…」→「只留指定上游节点的…」（R8）
- 「…始终保留——验收要看产物。」→「…始终保留，验收要看产物。」（R7）
- 「默认的对话方式——在图上接着聊，大家都看得见。」→「默认的对话方式：在图上接着聊，全部可见。」（R7、R8）
- 「三段各一节点，模型留空由你填。」→「三段各一节点，模型由用户填。」（R8）
- 内置预设一节补「擂台模式」「评审模式」「蜂群模式」各一句（R1：讲解页与下拉的六个名字对齐）

**graph/dispatch.ts**
- 「完整内容见留痕文件」→「完整内容见派发记录」（R5）
- 「已按留痕对账 N 个节点」→「已按派发记录核对 N 个节点」；「未能对账，按上次的待派节点重跑」→「未能核对，…」（R5）
- 仲裁节点正文「请独立裁决，给出你的结论与理由。」→「…给出结论与理由。」（R8；该句既发给模型也显示为节点消息）

**graph/stamp.ts**
- 评审模式节点文本「你是评审团的安全评审。只从安全视角审查这批材料：指出…」→「安全评审：只从安全视角审查这批材料，指出…」；性能评审同改（R1「评审团」→评审模式、R8）

**graph/swarm.ts**
- 合并结果行与结论里的「测试红」→「验收未通过」（三处：`swarmRecordedResult` 识别、`swarmResultLine`、汇总标签）（R5）

**resources/types.ts**
- 「该模型上次验证失败，可在接入页手动重验。」→「…可在接入页重新验证。」（R2：按钮就叫「重新验证」）

**lib/terminal-output-scheduler.ts**
- 终端提示串里两个半角逗号 → 全角（R6）

**i18n/zh-CN.ts**（zh-TW.ts 同步改繁体）
- 「复制会话ID」→「复制会话 ID」；「连接中...」→「连接中…」；「搜索历史对话...」→「…」（R6）
- 「添加备注...」→「写备注…」（R2、R6）
- 「识别不到?」→「识别不到？」；detect_help_tip「若你曾经用过英文开发者制作的 AI 终端工具,可能…3种修复方案 ① … ③ 让 AI 帮你修复 PATH」→「含中日韩字符的 PATH 被英文 AI 终端工具改成乱码后，所有终端都识别不到。修复方式：重装该工具、手动修复 PATH，或交给 AI 修复 PATH。」（R6、R8）
- switch_model_tip「给所有的 AI 工具一键快速地切换模型,推荐使用 EchoBird,点击"切换模型"打开 Echobird.ai 下载安装即可。」→「打开 echobird.ai，安装后可给全部 AI 工具切换模型。」（R4、R6、R8：去营销与劝导）
- 「没有改动 — 工作区干净。」「未安装 Git — …」破折号 → 逗号（R7）
- 「文件较大,未渲染逐行差异」→ 全角逗号（R6）
- 「遇到 BUG 或建议，点击下方提交你的问题：」→「…点击下方提交：」（R8）
- 「默认(内置)」→「默认（内置）」；gambit 占位符与 send_empty_hint 的半角括号 / 逗号 → 全角（R6）
- 「故事还没开始 — 跟 AI 聊起来，点亮你的第一格」→「还没有会话记录」（R7、R8）

**测试（断言旧字符串的一并改）**
- `graph/__tests__/presets-05b-r2.test.tsx`：「选它」「已选中」→「选定」「已选定」
- `graph/__tests__/consistency-06b.test.tsx`、`idle-and-tail-07b.test.tsx`：「已按留痕对账 …」「未能对账，…」→ 新文案
- `graph/__tests__/swarm-08b.test.tsx`：「测试红」→「验收未通过」
- 新增 `graph/__tests__/glossary-11.test.ts`：`?raw` 读 `docs/MANUAL.md` 术语表「内置预设」一行与 `src/orchestration.rs` 的 `builtin_presets()`，断言六个名字都以「模式」结尾、顺序一致、前端兜底的蜂群预设名在表里，以及 `presetDisplayName` 的三种输出

### Rust `src/`

**orchestration.rs**（LF）
- 内置预设名「擂台」→「擂台模式」、「评审团」→「评审模式」（R1；id `builtin-arena` / `builtin-jury` 不动）
- 「未找到 resources.json，尚未接入资源」→「…，未接入资源」（R4；含同名断言的测试）

**dispatch.rs**（LF）
- 「写入合并留痕失败」×5 →「写入合并记录失败」；「读取派发留痕失败」→「读取派发记录失败」；「留痕缺少 frontmatter」「留痕 frontmatter 未结束」「解析留痕 frontmatter 失败」「不支持的留痕 schema」（含测试断言）「读取留痕目录失败」「读取留痕目录项失败」→ 一律「派发记录」（R5）

**resources.rs**（LF）
- 「冒烟 {}/{} 失败：{}」→「验证 {}/{} 失败：{}」；「冒烟任务失败」×2 →「验证任务失败」（R2）
- 「手动添加失败：名称和二进制名不能为空」「手动添加任务失败」→「手动接入…」（R2）

**server.rs**（CRLF）：无用户可见中文串，未改。

### 文档 `docs/MANUAL.md`（LF）
- 首段「你接入资源…你裁决」→「接入资源…用户裁决」；「aicross 不扫你的机器，你说有什么它才用什么」→「不扫描本机，只用申报过的资源」（R8）
- 「换了端点要在接入页手动验」→「…在接入页验证」；「想提前验或者重验…」→「提前验证或重新验证：…」（R2）
- 「## 三个内置预设」→「## 内置预设」，补一句命名规则，补擂台模式 / 评审模式 / 蜂群模式三条（R1）
- 蜂群一节：「选它」→「选定」；「测试红」×2 →「验收未通过」；三处「——」→ 分号 / 逗号；「你自己去合」→「由用户合并」；「重派不重派由你定」删（R2、R5、R7、R8）
- `.dispatch/` 一节：「留痕」→「派发记录」；设置项指引改为「把「派发记录」切到「只写运行目录」」（R5，与设置页新文案一致）
- 密钥「先告诉你」→「先提示」；脱敏「它认不出来 ——」→「认不出来，」；删「自己先看一眼」（R7、R8）
- 末尾新增「术语表」

## 整句删除

- `DispatchOverlay.tsx`：「外派固定足迹不划算，」（R5、R4）
- `DispatchOverlay.tsx`：「；可修改」（R4）
- `DispatchOverlay.tsx`：「没有要合并的了」（R4，并入前半句）
- `docs/MANUAL.md`：「重派不重派由你定」（R8）
- `docs/MANUAL.md`：「自己先看一眼」（R8）

## 验证

`cd src-ui && npx vitest run --configLoader native --pool threads`

```
 Test Files  28 passed (28)
      Tests  402 passed (402)
```

`cd src-ui && npm run build`

```
> aicross-ui@0.0.0 build
> tsc -b && vite build
…
✓ built in 708ms
```

`cargo check --all-targets`（warning 计数 0）

```
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.18s
```

`cargo test`

```
test result: ok. 330 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 10.26s
```

换行核对（改前改后各数一次）：`src-ui/src/tauri.ts` CRLF 577 / LF 95（未改动）、`src/server.rs` CRLF 4620 / LF 0（未改动）、`src/orchestration.rs` LF 1386、`src/dispatch.rs` LF 6857、`src/resources.rs` LF 3513、`docs/MANUAL.md` LF 143 → 178（只增行，仍全 LF）。未 `cargo fmt`，未起窗口，未发真机请求，未写主工作树。

## 没做 / 保留

- `resources.json` 的 `status` 机器值「待冒烟 / 已接入 / 冒烟失败」（Rust `STATUS_*` 与前端 `STATUS_*` 常量）保留：是用户磁盘上的存储格式，改值需迁移；界面不直接显示它。
- 已存图里节点正文中的旧「测试红」字样：`swarmRecordedResult` 同时识别「验收未通过」与旧「测试红」（编排者补，`swarm-08c.test.tsx` 有断言），旧图状态标签不变。
- 测试名与测试数据里的期次 / 修正轮编号（06B、R1、G1…）与「你好」之类数据不改，不是用户可见字符串。
- 代码注释里的「擂台」「评审团」「冒烟」「口径」「G1」等未改（不进界面）。
- 「前提台账」「抽查」「归类模型」「长驻」「单写者」等既有功能名未改：不在任务书的改名清单里，且各处已一致。
- `commands.smoke*` / `smoke_resource_model` 等命令名与 `smoked_at` / `smoke_failure` 字段名不动（机器 id）。
- phase/10 正在改的 `CenterPanel.tsx` / `Explorer.tsx` / `TierTerminal.tsx` / `commander/*`：体检未发现需要改的中文串，本期没有碰这几个文件，不会产生冲突。
- `scripts/copy-audit.py` 的规则里 `—` 会把空值占位符也报出来，留作人工复核项，没有为它加白名单。

## 复审后补记（编排者）

DeepSeek 盲审（`tasks/review-11-ds.md`）判可交付但有未闭合项，编排者直接修：`src/dispatch.rs` 两处用户可见错误串「留痕」→「派发记录」；图上「盖预设」菜单与底栏「当前节点」行改走 `presetDisplayName`（未盖预设的节点不再显示「自定义」）；`src/orchestration.rs` 的「未找到 resources.json，未接入资源」去掉与前端前缀重复的后半句；手册「不是擂台」→「不是竞争」，术语表补两处出现位置并改「接入」一行的状态说明。体检脚本对含转义的模板串有盲区（DeepSeek 8），计数不是完备清单，已知。
