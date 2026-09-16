# REPORT-22 控制台就是对话窗 · 第一期

## 结果

TASK-22 已按任务书“本期要做”1–3 完成。新建 tab 现在只进入控制台；选定 agent、模型和目录后，中栏显示 aicross 自己的消息流，单线发送和模式编排都复用现有派发路径。未启动窗口，未发送真机请求，未调用真实模型。

## 1. 改名

- 新增 `CONTROL_CONSOLE_LABEL` 作为主 tab 名称定义，新建菜单、tab、启动器和文档统一使用“控制台”。
- `docs/MANUAL.md` 已改为消息流、`@` 改派、续聊和右栏编排的当前用法，术语表增加“主对话 tab → 控制台”。
- `docs/PRODUCT.md` 的当前形态、09-16 定案和历史段落已对齐；旧称扫描测试排除当前任务书后无残留。
- `commander.json`、`commander*` 内部字段、`source: "console"` 和 `aicross:console-*` 事件名均未改名。

## 2. 控制台消息流

- 新建菜单只保留“控制台”。系统终端、对话图的代码和 tab 类型保留，仅从新建菜单撤下；启动器中的次级入口仍可进入。
- `ConsoleChat` 提供用户消息、模型回复、系统提示三类消息和多行输入框；回车发送，Shift+回车换行。
- 单线发送通过单节点 `dispatchGraphStage` 请求复用既有派发引擎，未新增执行器或 Rust HTTP 接口。
- 第一期控制台聊天按只读权限派发；需要改文件时使用模式派发。界面空态与手册均明确说明这一限制。
- tab 会话状态追加 `resumeId`、`messages`、`targetAgent`，均使用前端 camelCase。续聊 id 按 agent / provider / model 分开存储，并作为 `sessionId` 交给既有通道；Codex、Claude Code、pi agent 和 Kimi Code 的具体续聊方式继续由已有 Rust 通道实现。
- `@<显示名>` 只改派当前一条，大小写不敏感；未知名称保留为普通正文。补全只列本机原生或已验证的 agent。

## 3. 编排对当前控制台生效

- 控制台 tab 激活时，“应用”不再依赖 `graphFocus` 或已选节点。待发内容优先取当前输入框，为空时取最近一条用户消息。
- 模式派发用既有 `newGraph` / `fillRoot` / `stamp` 建图，用 `startPipelineHeadless` 在后台执行；图保持 `source: "console"`。
- 每路结果各回一条消息，携带 agent、模型、角色和耗时。控制台空态改为“选一个模式，或直接打字单线聊”；“在图上选一个节点”只在对话图 tab 出现。

## 关键测试先红

定向命令：

```text
cd src-ui && npx vitest run src/__tests__/console-chat-22.test.ts --configLoader native --pool threads
```

临时撤掉对应实现后，同一次运行得到 `5 tests | 5 failed`：

- 撤掉已知 `@` 前缀解析后，Kimi Code 用例实际返回 `recognized: false`、目标为 `null`。
- 撤掉续聊 id 读取后，实际返回空串，期望 `thread-1`。
- 撤掉控制台的编排放行后，`graphFocus=false` 时实际为 `false`，期望 `true`。
- 在新建菜单恢复一个终端项后，实际为两项，期望只有控制台。
- 在任务书外重新引入旧称后，扫描结果精确指向 `src-ui/src/console-session.ts`，期望空列表。

所有临时故障均已恢复；恢复后定向测试 `5 passed (5)`，完整前端测试 `590 passed (590)`。

## 最终验证

以下命令均由本次实现直接运行：

```text
> cargo check --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 45s
```

零 warning。

```text
> cargo test
running 462 tests
test result: ok. 451 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 23.34s
```

11 项 ignored 均为仓库既有的真机、真实模型或本地事实源测试，本次按硬性要求未启用。

```text
> cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  40 passed (40)
Tests  590 passed (590)
Duration  1.18s
```

```text
> cd src-ui && npm run build
✓ 347 modules transformed.
✓ built in 753ms
```

```text
> python scripts/copy-audit.py
共 6 处命中，5 个文件（含误报，需人工复核）
```

审计退出码为 0。6 处均为既有文案或兼容值：`DispatchOverlay.tsx`、`SettingsModal.tsx`、`console-channel.ts`、`stamp.ts`、`resources/types.ts`。本期新增的消息流文案未留审计命中。

换行复核：`src/server.rs` 保持纯 CRLF，其他已修改 Rust 文件保持纯 LF，`CenterPanel.tsx` 保持纯 CRLF，`src/main.rs` 保持纯 CRLF，`src-ui/src/tauri.ts` 未修改并保持原有混合换行。未运行 `cargo fmt`。

## 本期明确未做

- 未改造对话图：原有图 tab、节点与结果交互均保留，本期只把它从新建菜单撤下。
- 未转译原生 CLI 的 `/model`、权限审批或其他交互；原生 tab 类型和现有入口代码未删除。
- 未修改 `src/dispatch.rs` 或派发执行逻辑，未改留痕格式、控制台通道协议、HTTP 路由或建议条实现。
- 未改 `commander.json` 字段、`source: "console"` 值或 `aicross:console-*` 事件名。
- 未启动窗口，未发真机请求，未调真实模型；因此界面仅做静态实现、类型检查和自动测试，没有窗口视觉手测结论。
- 未运行 `cargo fmt`，未 commit，未 push。

## 补充（09-16 复审后）

**本期控制台的单线聊天按只读派发**（codex `-s read-only`、claude 只读工具白名单、pi 只读工具集）。
这是第一期的有意取舍：控制台没有审批 UI，放开写权限要连审批路径一起做，留第二期。
界面空态与 `docs/MANUAL.md` 都已写明，需要改文件时用模式派发。首版报告漏写了这条，补在这里（复审 Gemini 3）。
