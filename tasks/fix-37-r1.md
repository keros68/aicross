# TASK-37 修正轮 R1（10-01）

依据：编排者复核（O）、`tasks/review-37-claude.md`（Claude 子代理，C）、`tasks/review-37-agy.md`（Antigravity Gemini，G）。标「已核」的条目编排者已对照代码确认。范围与禁区仍按 `tasks/TASK-37-viewer.md`；本轮另允许在 `src/engine/mod.rs` 的 run 文件结构上只追加一个字段（R1-4）。不 commit，不 push。完成后在 `tasks/REPORT-37.md` 末尾追加「R1」一节：逐条写改了什么、对应测试名；并把报告里「`compactRunNumber` 来自 `console-run-detail.ts`」改成实际来源。

## 必修

**R1-1 发布构建编译失败**（O，已核）
`cargo build --release` 报 `OpenRunRequest<'_>: Clone` 不满足（`src/server.rs` 约 3905 行，单实例转发 `open-run-request`）。这段代码在 `#[cfg(not(debug_assertions))]` 下，开发构建与 `cargo test` 都不编译它。修好，并把 `cargo check --release` 加进本期验证命令。

**R1-2 「取消采纳」无效**（C 高-1，已核）
引擎 `adopt` 只会写 `adopted = true`，查看页上「取消采纳」点了没有效果也不报错。第一版不做取消：已采纳的卡显示「已采纳」（不可点），未采纳的卡显示「采纳」。补一条前端纯函数测试钉住按钮文案与可点状态。

**R1-3 真身「与请求不符」误报**（C 高-2，已核）
`identityMismatch` 用字符串直接比较实际模型与请求模型；pi 的真身是 `provider/model`、claude 请求常写别名，正常结果被大面积标红。改为：只在引擎已判定时标「真身不符」——即节点 `errorKind === 'identity_mismatch'`；实际模型原样显示，不在前端做比较。删掉错误的期望（`run-viewer-37.test.ts` 里「请求 sonnet、真身 claude-sonnet-4-6 → 不符」那条），改为钉住新规则。备选路由的模型名不要用 `split('/')[1]` 拆（模型名本身可含 `/`）。

**R1-4 模式名不再靠推断**（C 中-1、G 中-1，已核；同时关闭积压 100）
- 引擎：`EngineRun` 只追加 `preset_id: String`（`serde(default)`，空串不写出），`start_dispatch` 写 run 时填入本次的预设 id；按 `targets` 直给时填空串。补 Rust 测试。
- 查看器：有 `preset_id` 时按预设清单取显示名（内置用自己的名字，用户自建为「自定义模式 · 名称」）；为空或找不到时显示「自定义模式」。删掉按角色序列推断的代码与测试。旧 run 文件没有这个字段，按空处理。
- `docs/CONSOLE-CHANNEL.md` 的 run 文件字段说明补 `preset_id`。

**R1-5 冷启动时 `aicross open` 要等窗口关闭才返回**（C 中-2，已核）
宿主里的 skill 调 `open` 时，冷启动那次会一直挂着。改为：冷启动时以分离方式另起窗口进程（参数带 `open <run_id>`，复用 TASK-36 的 `spawn_detached` 的做法：不继承句柄、不挂父控制台），自己立即退出 0。被拉起的窗口进程照常走 `pending_open`。补测试（参数构造与「不阻塞」可用注入方式测）。

**R1-6 查看页的层级与焦点**（C 中-3）
- 挂载时把焦点移到「关闭」按钮，Tab 焦点不出查看页。
- 不盖住自绘标题栏与窗口缩放边条（让出标题栏高度）。
- 查看页打开时位于对话图结果屏（`.dispatch-overlay`，z-index 12000）之上；Esc 只关最上层。

## 应修

| 编号 | 问题 | 来源 |
|---|---|---|
| R1-7 | 轮询出一次错不应永久停止：已拿到过数据时保留旧数据并继续按间隔重试；只有首次读取失败才显示「找不到这次派发」。命令行 `open` 只把「run 文件不存在」算退出 1，其它读取错误按 3 | C 低-1 |
| R1-8 | 竞态：`<RunViewer key={viewerRunId}>`；轮询与采纳的响应按请求序号与 runId 丢弃过期结果 | C 低-2、G 低-3 |
| R1-9 | 热启动打开同一个 run 要刷新：事件带递增序号，作为重取的依赖 | C 低-3 |
| R1-10 | `open-run-request` 监听在快速卸载时注销不掉：加 `disposed` 标志，异步注册完成时若已卸载立即注销 | G 低-2 |
| R1-11 | 视觉纪律：去掉 `var(--blue)`，运行中与失败这类异常状态用胶囊；token 行里中文标签用 Sans、数字用 Mono | C 低-5 |
| R1-12 | 删除 `run-viewer-37.test.ts` 中恒真的断言（约 103 行），换成有区分力的断言 | C |

## 不在本轮

- `read_run` 判死后整写 run 文件与引擎收尾之间的竞态（C 低-4）：属引擎，进积压。
- 「超出 3 张换行」与「横向滚动」两种读法：保留换行，MANUAL 已写明。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release            # 发布构建专属代码也要编译通过
cargo test
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
```

R1-3、R1-4 的新测试须能在修复临时改回时变红（编排者会抽查）。
