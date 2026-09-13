# TASK-07 资源接入并回：把第 1 期的接入页与左栏「资源」tab 移进 fork

先读 `AGENTS.md`、`docs/PRODUCT.md`（「形态」表的左栏与接入页两段、「通道架构定案（09-13）」）、`tasks/TASK-01-resources.md` 与 `tasks/REPORT-01.md`（第 1 期做了什么、怎么验的）、`tasks/REPORT-03.md`「改了什么·Rust」（第 3 期已把 `resources.rs` 的只读部分与 `process.rs` 移进 `src/`）。第 1 期的完整代码在 `port/src-tauri-old/src/`（`resources.rs`、`tools.rs`、`config.rs`、`lib.rs` 的命令注册）与 `port/src-ui-old/src/`（`components/Resources.tsx`、`api.ts`、`types.ts`、`App.tsx` 的左栏资源 tab）。`port/` 不进 git，只是搬运来源。

## 目标

左栏多一个「资源」tab，只列 `status == 已接入` 的模型，按 agent 分组，点模型把 `@agent/模型` 插进中栏输入框；栏底一行「N 个待冒烟 · M 个未接入 · 管理 →」进接入页。接入页是独立视图（顶栏齿轮旁或左栏底部入口），agent 卡片网格：标记、名称、版本、检测状态、该 agent 下全部模型与状态，卡底「探测 / 冒烟 / 手动输入」。冒烟通过的模型才进左栏。这就是 `docs/PRODUCT.md`「形态」表定的东西，第 1 期在旧壳里已实现并通过三轮审查，本期是**搬进 fork 并接上第 3 期的 `resources.json` 读取**，不是重做。

## 要做的

### A. Rust

1. 把 `port/src-tauri-old/src/resources.rs` 的探测、合并、手动录入、冒烟（含真身核对）、`resources.json` v1 读写并进 `src/resources.rs`，与第 3 期已有的只读加载共用同一份 `ResourceStore/ResourceAgent/ResourceModel`（字段名不改）。冒烟走第 3 期的 `src/process.rs` 与 `src/channels/`（隐藏窗口、超时、脱敏都已在那里），不要再复制一份进程代码。
2. 事实源按 `docs/PRODUCT.md`「各 agent 的探测事实源」表：claude `--version`；codex `~/.codex/models_cache.json`；kimi `~/.kimi-code/config.toml` `[models]`（模型 id 就是完整别名 `kimi-code/…`）；pi `~/.pi/agent/models-store.json` + `pi --list-models`（三个 provider：deepseek / qwen-token-plan-cn / zai-coding-cn）+ `pi auth check --provider <p> --json`。**cc-switch 降级为发现提示**：读到已配 provider 时只在接入页显示「cc-switch 里有 X，建议加进 pi」，不再作为可派发通道（09-13 定案）。
3. 冒烟判据不变：「回答的是不是它」——pi 比对事件里的 `responseModel`（第 3 期 `channels::parse_pi` 已实现，复用），claude 第三方端点比对响应体 `model`；`--list-models` 有不等于有权限，冒烟必须真发一次请求（09-13 实测 qwen plan 下 deepseek-v4-flash 403）。
4. 命令注册进 `src/server.rs` 的 `generate_handler!`（只追加）。密钥只按子进程环境变量传；不读 `~/.pi/auth.json`、`~/.cc-switch/*.db` 之外的任何凭据文件；cc-switch 读取沿用 `cc_switch.py list`（路径取第 3 期设置项 `ccSwitchPath`，空则跳过发现提示）。

### B. 前端

1. 左栏 `Explorer` 加「资源」tab（与「工作区」「会话记录」并列，默认选中「资源」）：按 `port/src-ui-old` 的 1B 期形态——只列已接入模型、按 agent 折叠分组、组头 agent 名与版本、模型行显示模型 id 与来源、点击把 `@agent/模型` 插进当前对话图列的输入框（用现有的 `aicross:` 自定义事件或 store 动作，别直接操作 DOM）。栏底汇总行与「管理 →」。
2. 接入页：把 `port/src-ui-old/src/components/Resources.tsx` 的卡片网格搬成新组件（放 `src-ui/src/components/resources/`），入口在标题栏设置齿轮左侧加一个「接入」图标按钮，视图为全屏覆盖层（与「讲解」同形态）。文案按 AGENTS.md 纪律：状态与动作成对（待冒烟 ↔ 冒烟、已接入 ↔ 移除）。
3. `tauri.ts` 加命令 wrapper，类型放 `src-ui/src/resources/types.ts`，与 Rust 字段一致。
4. 对话图的 `@` 弹层与右栏节点面板已经读 `list_dispatchable_models`，本期不动它们；接入完成后它们自然有数据。

### 不做

- 不动 `src-ui/src/graph/`、`components/center/graph/`、`components/right/orchestration/`、`src/dispatch.rs`、`src/channels/`（第 4 期正在另一分支改）。只允许调用它们已导出的函数。
- 不做额度条、不做美元核算、不做自动探测本机全部 CLI（申报制：探测按钮由用户点）。
- 不改主题与壳子（TASK-06 已定）。

## 验收

1. `cargo check`、`cargo test`、`cd src-ui && npx vitest run`（沙箱起不来用 `--configLoader native --pool threads` 并注明）、`cd src-ui && npm run build`，命令与输出原文进 `tasks/REPORT-07.md`。
2. 测试：第 1 期的 Rust 测试随代码一起搬回并全绿（fixtures 在 `port/src-tauri-old/fixtures/`，搬到 `fixtures/resources/`）；新增：pi 三个 provider 的 `--list-models` 解析、kimi 别名保留完整前缀、cc-switch 只产生发现提示不产生可派发条目。
3. 真机（≤ 4 次请求，沙箱发不出就写「未验证」留给编排者）：探测四个 agent；对 `pi/zai-coding-cn/glm-5.3-flash` 冒烟通过并进左栏；对 `pi/qwen-token-plan-cn/deepseek-v4-flash` 冒烟应失败并显示 403 原因。
4. 不起窗口；报告里列出改动文件与「与 phase/4 分支可能冲突的文件」（`server.rs`、`tauri.ts`、`SettingsModal.tsx`、`app-state.tsx` 改了哪几行）。不 commit，不 push。
