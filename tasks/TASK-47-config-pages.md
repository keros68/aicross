# TASK-47 窗口三页：资源、模式、最近的派发（10-06）

**只改 `src-ui/**`。不改 Rust（`src/**`），不改 `docs/`。不 commit，不 push。**

## 背景

`docs/PRODUCT.md`「编排配置器」：窗口里定编排，宿主里执行。窗口现在只有首页「最近的派发」与查看页（TASK-40）。本期加「资源」「模式」两页，与「最近的派发」并列成三页。引擎侧的命令由 TASK-46 并行实现，契约以 `tasks/TASK-46-modes-config.md` 第 1、3、4 节为准；本期按契约写前端，Rust 侧未合并前用 vitest 与浏览器模式下的假数据验证。

## 1. 外壳

- 标题栏下方一行页签：`资源` · `模式` · `最近的派发`。启动默认进「模式」；`aicross open <run_id>` 进查看页的现有行为不变，查看页「关闭」回到「最近的派发」。
- 选中的页签记在 `localStorage`（读写包 try/catch，读不到就用默认）。
- 「最近的派发」即现有 `Home`，内容不改；它的副标题保留。

## 2. 资源页

数据：`config_snapshot()` 的 `resources`（结构见 TASK-46 第 4 节，即 `aicross resources` 的输出：`agents[]` 每项 `id`、`name`、`version`、`state`、`note`、`models[]`；`models[]` 每项 `id`、`agent`、`provider`、`resolved_id`、`state`、`note`）。

- 每个 agent 一块：名称、版本、状态；`note` 非空时在状态下方显示一行。下面列它的模型：模型 id（等宽字体）、provider（有时显示）、实际模型 `resolved_id`（与 id 不同时显示）、状态、`note`。
- 状态词原样显示；`验证失败`、`未登录`、`额度用尽`、`未安装` 用异常色胶囊，其余用纯文字（视觉纪律见 `docs/PRODUCT.md`「视觉纪律」）。
- 页头一个「重新探测」按钮 → `resources_detect()`；第三方模型（状态为 `未验证`、`验证失败`、`冒烟超时` 的）每行一个「冒烟」按钮 → `resources_smoke(agent, model, provider)`。执行中按钮显示「探测中…」/「冒烟中…」并禁用，同一时间只跑一个；结束后重新取 `config_snapshot()`。失败把返回的说明显示在该行（或页头），不弹窗。

## 3. 模式页

数据：`config_snapshot()` 的 `config.modes` 与 `config.defaultMode`（TASK-46 第 3 节）。

- 左侧模式列表（名称；默认模式标「默认」）。右侧是选中模式的详情。
- **流程图**：阶段从左到右排成列，列之间用箭头；一列里的位置上下排（同阶段并行）。每个位置一个小卡片，写：角色、可见性（`盲` / `共享`）、写权限（`只读` / `单写者` / `worktree`）、当前人选。人选显示 `effective` 的 `agent/model`（等宽字体），下方一行小字写来源：`source` 为 `位置` →「本模式指定」，`角色` →「按角色默认」，`预设` →「预设写定」，`未指定` → 卡片标异常色、写「未指定，派发会被拒绝」。
- 点位置卡片展开编辑：
  - 「人选」下拉：列出资源页里 agent 状态为 `可用` 的全部模型，选项文字 `agent/model`（有 provider 时加 `· provider`）+ 状态；状态为 `验证失败`、`未登录` 的选项禁用。
  - 「思考」下拉：`沿用预设` / `关` / `低` / `中` / `高`（`沿用预设` 存空串）。
  - 「备选」下拉：`不设` + 同「人选」的列表。
  - 「保存」→ `config_set_seat(mode, stage, index, seat)`（stage、index 从 1 开始）；「清除」→ `config_clear_seat(mode, stage, index)`。完成后用返回的模式项刷新本模式。
- 详情页头：「设为默认」按钮 → `config_set_default_mode(id)`；已是默认时按钮换成文字「默认模式」。
- 页底一行说明：「宿主里的 ai-cross 按这里的设定派发；单次例外在对话里说即可。」

## 4. 命令封装

`src-ui/src/tauri.ts` 的 `commands` 加 TASK-46 第 4 节的六个命令与对应的 TypeScript 类型（字段名与 Rust 输出一致：资源与 `config show` 里是 snake_case 的保持 snake_case，`modes.json` 人选对象是 camelCase）。不在浏览器模式下（`isTauri` 为 false）时，资源页与模式页显示「请在 aicross 窗口中打开」，不报错。

## 5. 文案

界面文字遵守：中性简短说明体；按钮是 2–6 字动词短语；同一概念只用一个词（「人选」「备选」「默认模式」「冒烟」「重新探测」）；不出现英文内部词（seat、source、effective 等只在代码里）。

## 6. 测试（vitest）

纯函数抽到 `src-ui/src/config/` 下单独的 `.ts` 文件再测，不测 DOM：

- 人选下拉的选项生成：只含可用 agent 的模型；禁用规则；provider 拼接。
- 位置卡片的来源文字四种。
- 流程图的阶段 / 位置排布（给 `stages` 得到列与行）。
- 状态词 → 胶囊或纯文字的映射。
- 页签的 localStorage 读写：读写抛错时回默认。

## 验证

```
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
```

每个改动文件保持原有换行（`src-ui/src/tauri.ts` 是混合换行，只在改动处沿用邻近行的换行），新文件 LF。写 `tasks/REPORT-47.md`：改动与新增文件、测试名、两条验证命令的输出摘要、未做与理由。

## 编排者真机核（实现者不做，TASK-46 合并后）

dev 窗口里：三个页签切换、记住页签；资源页重新探测、对一个 pi 模型冒烟；模式页给盲审模式三个位置选人、设备选、设为默认，再用 `aicross config show` 核对写入；清除一个位置；`aicross open <run_id>` 仍直接进查看页，关闭回到「最近的派发」。
