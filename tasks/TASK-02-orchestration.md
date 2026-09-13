# TASK-02 对话图与编排：节点、分叉、三列面板、预设图章、一行记法、讲解页

先读 `AGENTS.md`、`docs/PRODUCT.md`（「形态」「三个引擎、两个开关」「模式清单」「自定义工作流」四节，以 09-13 定案「对话即图」为准）、`tasks/README.md`。原型 `docs/proto/index.html` 只作视觉与右栏参考，它的中栏还是线性聊天，已过时，不照抄。

本期只做**对话图的数据模型、界面与预设编排**，不执行任何派发（第 3 期）。

## 代码现状（动手前要知道的）

- 本仓库是 Coffee CLI 的 fork（见 `AGENTS.md`）。Rust 在根目录 `src/`（`Cargo.toml` 在根目录，不在 `src-tauri/`），前端在 `src-ui/`。中栏现在是终端 tab（`src-ui/src/components/center/CenterPanel.tsx`），右栏只有任务板（`src-ui/src/components/right/Compiler.tsx` → `TaskBoard`）。全局状态在 `src-ui/src/store/app-state.tsx`。
- 第 0/1 期写的资源接入代码在 `port/`（`port/src-tauri-old/src/resources.rs`、`port/src-ui-old/src/types.ts`），不参与编译，第 3 期再并回。本期**不移植它**，只复用它的 `resources.json` 契约（`ResourceStore` / `ResourceAgent` / `ResourceModel`，见 `port/src-ui-old/src/types.ts`）。
- 用户级状态目录是 `~/.aicross`（F2 期定），本期的持久化文件都放这里。
- 界面文案与视觉按 `AGENTS.md`「纪律」与 `docs/PRODUCT.md`「视觉纪律」。厂商色只做色点与细边线。

## A. 数据模型（定了就是契约，后续期不改字段名）

```
Graph    { version: 1, id, title, root: NodeId, nodes: Record<NodeId, Node>, created_at, updated_at }
Node     { id, kind, parents: NodeId[], user: Message, reply?: Reply, target?: Target, created_at }
kind     = material | turn | accept
Message  { text, attachments: string[] }          // attachments 是文件路径
Reply    { status, text, model_reported?, tokens_in?, tokens_out?, ms? }
status   = draft | pending | done | failed         // 本期只产生 draft
Target   { agent, provider?, model, role, tier?, thinking, visibility, write }
role       = 出方案 | 执行 | 验收 | 验证 | 自定义文本
thinking   = 关 | 低 | 中 | 高
visibility = 共享 | 盲 | 指定上游（上游节点 id 列表）
write      = 只读 | 单写者 | worktree
Preset   { id, name, builtin: bool, stages: Stage[] }
Stage    { nodes: Target[] }                        // 同阶段并行，阶段间串行
```

规则：

1. **根节点** `kind=material`，`parents=[]`，无 `target`，装用户的第一条消息与附件。一张图只有一个根。
2. **turn** 恰好一个父节点。**accept** 至少两个父节点，这是唯一的扇入形态。没有别的连边方式：父子关系只能由「分叉」或「盖预设」产生，界面上没有自由连线。
3. **上下文 = 根到该节点的路径**，按 `visibility` 过滤：
   - 共享：路径上全部用户消息 + 全部模型回复。
   - 盲：路径上只留用户消息与附件，模型回复全部剔除。
   - 指定上游：用户消息全留，模型回复只留列表里那些节点的。
   - accept 节点：路径按每个父节点分别算，取并集去重，父节点自身的回复始终保留（验收要看产物）。
   - 实现为纯函数 `contextOf(graph, nodeId): Message[]`，前端单元测试覆盖。这是第 3 期真正外发的内容，本期必须能在界面上预览。
4. **角色默认值**：`出方案`/`执行` → 共享 + 只读；`验收`/`验证` → 盲 + 只读。用户可改。
5. **盖预设**（stamp）在节点 N 上展开为子图：第 1 阶段的每个 Target 成为 N 的子节点（turn）；第 k 阶段的每个 Target 以第 k-1 阶段全部节点为父节点，父节点数 ≥2 时 `kind=accept`，否则 `turn`。展开后的节点是普通节点，可单独改、可再分叉。实现为纯函数 `stamp(graph, nodeId, preset): Graph`，单元测试覆盖。
6. **模型来源**：新增 Tauri 命令 `list_dispatchable_models`，读 `~/.aicross/resources.json`，返回 `status == "已接入"` 的 `ResourceModel`；文件不存在或解析失败返回空数组加一条错误文本，不抛异常。界面在空列表时显示「尚未接入资源」，并允许手填 `agent/model`，节点上标「未接入」。不在代码里手抄任何模型 ID。
7. **持久化**：`~/.aicross/presets.json` `{ version: 1, presets: [...] }`；`~/.aicross/graphs/<graph id>.json` 一图一文件。读写走 Rust 命令，原子写（先写临时文件再改名）。内置预设 `builtin=true`，不可删可复制，用户改了内置预设的字段就存成新预设。
8. 内置三个预设：
   - **共享对话**：一段一节点，role=执行，共享。默认。
   - **经济流水线**：`出方案(高档,thinking=高) → 执行(低档,thinking=低) → 验收(盲,换厂商)`，三段各一节点，模型留空由用户填。
   - **盲审**：一段三节点，role=验证，盲，只读。
   - 擂台 / 评审团 / 仲裁 / 前提抽查 / 主备 / 顾问 / 红蓝 / 群聊 / 蜂群 在预设下拉里置灰并标期次，不实现。

## B. 一行记法（预设的文本形式，与表单双向同步）

```
fable-5 → [gpt-5.6-terra | k3 | glm-5.2]@blind → glm-5.3
```

- `→` 分阶段，`[a | b]` 同阶段并行，后缀：`@blind`/`@shared`、`#off|#low|#mid|#high`、`:ro|:rw|:wt`、`!角色`（如 `!验收`）。模型名可写 `agent/model` 消歧，省略 agent 时按 `list_dispatchable_models` 唯一匹配，不唯一则报错列出候选；列表为空时按原样接受并标「未接入」。
- 实现 parser 与 printer，**往返测试**：任意合法 Preset → 一行 → 解析 → 与原 Preset 相等；错误输入给出位置与原因（中文）。
- **修订（09-13 修正轮 R1）**：一行记法只表达 agent/model、角色、thinking、可见性种类、写权限；`provider`、`tier`、指定上游的节点列表不进记法。往返相等只对可表达字段成立；printer 对含不可表达字段的预设返回 lossy 标记，构建器在记法行下提示「一行记法不表达：provider、档位、指定上游」。

## C. 界面

### C1. 中栏「对话图」tab

作为中栏 tab 条里的一种 tab（与终端 tab 并列，沿用现有 tab 机制，新增一个 tool 类型即可），新建时生成一张只有根节点的图，根节点的消息就是用户在输入框里发的第一句。

上半：**会话图**。
- 从左向右生长，按深度分层，同层竖排；父子之间画线（SVG 或 CSS 均可，不引新依赖）。
- 节点卡：厂商色点、`agent/model`、角色、状态（draft 显示「待派」）；根节点显示「材料」；accept 节点左侧有多条入线。
- 单击只高亮（高亮节点是右栏与图上命令的作用对象），双击把主聊天面板对齐到该节点，Ctrl+双击把该分支固定到并排的第 2 / 3 列（最多 3 列，同一分支只保留一个面板，后开的替换）。
- 高亮节点旁的小按钮：「分叉」（在此节点下新建一个 draft 子节点，弹出节点面板填 Target）、「盖预设」（打开预设下拉，选中即在此节点上 stamp）。
- 图区可横向滚动，不做缩放、不做拖拽移动节点。

下半：**聊天面板**，1 到 3 列。
- 每列显示一条分支：根到该列末端节点的路径，每轮一段（用户消息 + 回复；draft 回复显示「待派」占位）。列头显示末端节点的 `agent/model` 与「上下文 N 条 · 共享/盲」，点击「上下文」展开该节点 `contextOf` 的结果预览（这是第 3 期外发的内容，本期就要能看）。
- 每列有自己的输入框。输入 `@` 弹出候选（来自 `list_dispatchable_models`，按 agent 分组）。发送 = 在该列末端节点下新建一个 turn 节点，Target 取 `@` 的模型与角色默认值，回复为 draft。没写 `@` 时沿用该列末端节点的 Target。
- 主面板输入框上方一行：「当前节点：<agent/model> · <预设名或"手动">」，点击切到右栏「方案与编排」。

### C2. 右栏「方案与编排」tab

右栏改为两个 tab：「任务」（现有 TaskBoard）与「方案与编排」。

1. **子图预览**：高亮节点及其全部后代，按 A5 的展开形态显示成阶段条（同阶段并排）。选了预设未应用时，虚线显示将要盖上去的子图；「应用」= 在高亮节点上 stamp。
2. 顶部预设下拉（内置 + 用户保存的）+「应用」+「编辑编排」+「讲解」。
3. **节点面板**：点图上或预览里的节点弹出，改 Target 各字段；模型下拉只列已接入，可手填；改动即写回图并持久化。
4. **构建器**（「编辑编排」）：阶段按行，行内「添加节点」「删除节点」，行间「添加阶段」「删除阶段」「上移/下移」；底部同步显示一行记法（可编辑，失焦时解析回表单，解析失败红字提示不覆盖表单）；按钮「保存为预设」（要名字）「应用」「关闭」。
5. **校验提示**（只标注，不阻止）：验收/验证节点的 vendor 与其任一祖先执行/出方案节点 vendor 相同 → 标「复核，非独立验证」；节点模型未接入 → 标「未接入」；accept 节点的父节点少于 2 → 标「扇入不足」。
6. **讲解**：独立视图（全屏覆盖层），分节：什么是节点与分叉、上下文怎么算（共享/盲/指定上游各一句）、每个预设做什么。内容从 `docs/PRODUCT.md` 抽，说明体。

## 不做

- 不执行、不派发、不写 `.dispatch/`、不拉起任何 CLI（第 3 期）。
- 不做 thinking 到各 CLI 参数的映射（第 3 期）。
- 不做自由连线、不做拖拽画布、不做缩放。
- 不移植 `port/` 里的资源接入界面。
- 不改终端 tab、任务板、工作区的既有行为。

## 验收

1. 命令与输出原文进 `tasks/REPORT-02.md`：`cargo check`、`cargo test`（根目录）、`cd src-ui && npm run build`。跑不了的写「未验证」，不写「已验证」。
2. 单元测试（前端 vitest 或 Rust，放哪层看实现落点）：
   - `contextOf`：共享 / 盲 / 指定上游 / accept 并集，至少 4 个用例。
   - `stamp`：`1 → 3 → 1` 在根上展开得到 1 个 accept 节点带 3 个父节点；`1 → 1 → 1` 链式；在非根节点上盖；至少 3 个用例。
   - 一行记法往返 ≥6 个用例，含消歧失败、非法后缀。
   - `presets.json` 读写、内置不可删；`graphs/<id>.json` 往返相等。
   - 校验提示：同厂商验收标「复核」。
3. 手动 smoke 清单（能跑窗口就跑，不能就如实写未验证）：新建对话图 → 发第一句得到根节点；`@` 一个模型发送 → 出现子节点、面板多一段「待派」；在同一节点再 `@` 两家 → 三个并列子节点；Ctrl+双击两个分支 → 三列并排；选「经济流水线」应用到根 → 图上出现 出方案 → 执行 → 验收 三层；点验收节点看「上下文」→ 只有用户消息与父节点回复；改一个节点模型 → 一行记法同步；保存为预设后下拉里出现；讲解页能开能关；重启后图与预设仍在。
