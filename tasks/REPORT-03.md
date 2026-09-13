# REPORT-03 — 派发第一刀

实现日期：2026-09-13。任务书：`tasks/TASK-03-dispatch.md`（含 09-13 第 0 节）。本轮未启动 Tauri 窗口，未运行 `tauri dev`，未提交或推送。

## 改了什么

### Rust 派发链路

- `src/process.rs`：从第 1 期进程实现继续收敛统一子进程入口；Windows 的 agent、命令脚本与 `taskkill.exe` 都使用 `CREATE_NO_WINDOW`；输出上限提高到 4 MiB并返回 `truncated`；保留超时杀进程树，增加运行中 PID 注册和单节点取消。Windows npm/cmd shim 会经隐藏的 `cmd.exe /d /s /c` 启动，修复了把无扩展名 shim 当 Win32 程序执行的错误。
- `src/resources.rs`：把派发所需的 `resources.json` v1 契约读入 fork，`orchestration.rs` 与派发共用同一个 `ResourceModel` 和“已接入”过滤，不再各自维护镜像结构。第 1 期完整探测/接入界面仍留在 `port/`，本期没有重做。
- `src/channels/mod.rs`：实现 Claude、Codex、Kimi、pi、cc-switch 的 argv、stdin/任务文件、输出体成功判定、用量/真身/session 解析。Claude 盲审带 `--restricted`；Codex 父级选项位于 `resume` 前；Kimi 按 UTF-8 字节拒绝超过 30000 的 prompt；pi 只认 assistant `text` 且核对 provider/model，并使用固定 UUID 与 aicross 自有 session 目录。
- 本机 Kimi Code CLI 0.42.0 的 `--help` 显示续接参数为 `-S/--session`，因此实现采用 `-S`，没有照抄任务书中已漂移的 `-r`。
- `src/dispatch.rs`：实现预览、单阶段派发和取消三个 Tauri 命令；同阶段最多 3 路且同一 agent 最多 1 路；共享节点用项目目录，盲节点用空运行目录并只复制所选附件；prompt 与 UTF-8 附件在外发前覆盖六类脱敏；结果、原始输出、用量、身份、session、截断和错误写入 `.dispatch/`，`STATE.md` 每次更新；未写项目时改写到 aicross 运行目录。
- `.dispatch` frontmatter 使用 `aicross-dispatch/1`，正文保存实际送出的 prompt（续接时只保存本条，而非未发送的完整上下文）。

### 对话图与界面

- `Reply` 新增可选 `session_id`；R1 又增加持久化的 `adopted` 标记。`context.ts` 在既有 `contextOf` 语义上增加稳定的角色化序列化、盲审冻结前缀、逐条消息/附件选择和续接本条序列化；逻辑不散在组件中。
- draft 节点只有在父节点为 material 或 reply 已 `done` 时进入本阶段；派发前先持久化 `pending`，完成后把正文、状态、真身、in/out、耗时和 session 写回原节点并再次持久化。
- G1 显示每路决策行、厂商授权、脱敏计数、盲审材料清单，可改目标、档位和 thinking；G2 显示排队/运行/完成/失败/超时并可取消；G3 并排显示结论、采纳标记、重派、升级、用量表、留痕和“交人裁决”。“升级”提高档位/thinking并回到 G1，也可在 G1 换厂商。
- 设置新增默认超时、并发上限、归类模型、外发脱敏和项目留痕。归类执行属于任务书可选项，本期只保留设置和容错解析测试，没有接结果屏按钮。
- `fixtures/blind-task/plan.md` 为 321 字，`data.csv` 为 20 行。

## 自动验证（命令与输出原文）

### `cargo check`（根目录，exit=0）

```text
   Compiling aicross v0.1.0 (D:\work\aicross)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.99s
```

### `cargo test`（根目录，exit=0，110 项；相关用例与收尾原文）

```text
running 110 tests
test dispatch::tests::live_blind_review ... ignored, real model requests; run once for TASK-03 acceptance
test dispatch::tests::live_shared_continuation ... ignored, two real model requests; run once for TASK-03 acceptance
test channels::tests::kimi_rejects_more_than_30000_utf8_bytes ... ok
test channels::tests::pi_identity_mismatch_fails ... ok
test dispatch::tests::classification_parser_tolerates_fences_and_missing_arrays ... ok
test dispatch::tests::frontmatter_roundtrips ... ok
test dispatch::tests::blind_directory_contains_only_selected_materials ... ok
test dispatch::tests::redacts_all_six_secret_shapes_and_stays_clean ... ok
test channels::tests::blind_claude_contains_restricted ... ok
test channels::tests::kimi_resume_uses_current_cli_session_flag ... ok
test channels::tests::pi_uses_explicit_session_and_app_owned_session_directory ... ok
test channels::tests::codex_resume_follows_parent_options ... ok
test process::tests::run_process_truncates_and_marks_output ... ok
test process::tests::run_process_times_out_and_terminates_the_child_tree ... ok

test result: ok. 106 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 4.61s
```

### `cd src-ui && npx vitest run`（exit=1，未验证；输出原文）

```text
failed to load config from D:\work\aicross\src-ui\vite.config.ts

⎯⎯⎯⎯⎯⎯⎯ Startup Error ⎯⎯⎯⎯⎯⎯⎯⎯
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:458:11)
    at spawn (node:child_process:813:9)
    at Object.execFile (node:child_process:349:17)
    at exec (node:child_process:236:25)
    at optimizeSafeRealPathSync (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:1954:2)
    at windowsSafeRealPathSync (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:1940:3)
    at getRealPath (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32413:36)
    at tryResolveRealFileOrType (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32407:9)
    at tryCleanFsResolve (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32151:21)
    at tryFsResolve (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32144:14)
```

失败发生在测试文件加载前。当前受限环境禁止 Node 24 为 Vite 的 Windows 网络盘探测启动 `cmd.exe`。为核对实际前端用例，追加运行不需要该子进程的原生配置加载和 worker thread 模式：

### `cd src-ui && npx vitest run --configLoader native --pool threads`（exit=0）

```text
 RUN  v5.0.0 D:/work/aicross/src-ui

 Test Files  6 passed (6)
      Tests  58 passed (58)
   Start at  19:27:35
   Duration  252ms (transform 62%, import 20%, tests 12%, worker 6%)
```

因此 58 个前端测试本身通过，但任务书指定的原命令按要求记为“未验证”，不把替代命令冒充为原命令通过。

### `cd src-ui && npm run build`（exit=0，收尾原文）

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 575 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                      2.51 kB │ gzip:   1.12 kB
dist/assets/main-DhfNuIrn.css                      148.75 kB │ gzip:  25.92 kB
dist/assets/main-C6R4AXaN.js                     1,250.78 kB │ gzip: 445.41 kB

✓ built in 2.33s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

中间为 Vite 对 575 个动态语法资源的逐项文件大小清单；上面保留命令头、关键产物与原始收尾。chunk 体积提示为既有提示。

## 真实盲审与续接尝试

当前沙箱拒绝写用户目录。验收 harness 仅在 `cfg(test)` 下把 aicross 运行根临时改到项目内被忽略的 `.dispatch/runtime/`；生产构建仍使用 `~/.aicross/runs/<run>/<node>`。未读取、复制或改写 `~/.pi/auth.json`、`~/.pi/agent/auth.json`、`~/.cc-switch` 或任何凭据文件，也没有输出或保存环境变量值。

### 三路盲审 + 唯一替补（真实进程，模型请求上限 4）

命令：

```text
cargo test dispatch::tests::live_blind_review -- --ignored --nocapture
```

本次最终尝试输出原文：

```text
running 1 test
BLIND_RUN=task03-blind-20260913-191823 STATE=D:\work\aicross\.dispatch\STATE.md
BLIND_RESULT node=blind-pi-glm status=error record=D:\work\aicross\.dispatch\20260913-191824-pi-glm-4-7-数据审查.md isolation=部分 identity_present=false
BLIND_RESULT node=blind-codex-luna status=error record=D:\work\aicross\.dispatch\20260913-191824-codex-gpt-5-6-luna-风险审查.md isolation=部分 identity_present=false
BLIND_RESULT node=blind-kimi-fast status=error record=D:\work\aicross\.dispatch\20260913-191824-kimi-kimi-for-coding-highspeed-方案审查.md isolation=部分 identity_present=false
BLIND_FALLBACK for=blind-pi-glm status=error record=D:\work\aicross\.dispatch\20260913-191825-pi-deepseek-v4-flash-替补审查.md identity_present=false
test dispatch::tests::live_blind_review ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 107 filtered out; finished in 2.30s
```

四条路线都在到达模型响应前失败：

- pi 原路线与 `pi/qwen-token-plan-cn/deepseek-v4-flash` 唯一替补：CLI 为读取凭据创建 `~/.pi/agent/auth.json.lock` 时被沙箱以 `EPERM` 拒绝。
- Codex：无法在 `~/.codex` 初始化 in-process app-server client，`os error 5`。
- Kimi：输出 `system.version=0.42.0` 后在用户存储写入处返回 `storage write failed: permission denied`。

记录里的 `tokens_in/out=0`、`identity=''`，结合错误发生点，说明本次没有得到模型响应；按证据边界不计为盲审成功，也不声称“用量表有数”。三路进程和一条指定替补均已尝试，未换用其它路线。

### 共享续接（上限 2；首轮即被拦截）

命令：

```text
cargo test dispatch::tests::live_shared_continuation -- --ignored --nocapture
```

收尾原文：

```text
thread 'dispatch::tests::live_shared_continuation' panicked at src\dispatch.rs:1150:9:
assertion `left == right` failed: 首轮失败：
  left: "error"
 right: "ok"
test dispatch::tests::live_shared_continuation ... FAILED

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 107 filtered out; finished in 1.20s
```

首轮同样在 pi 凭据锁处被沙箱拒绝，没有得到 session_id，因此没有发第二轮。共享续接的真机端到端结果为“未验证”。静态用例已覆盖同目标判定、固定 session id、只发送本条、回复持久化和续聊 argv。

最终验收轮包含 5 个进程尝试（三路 + 一条指定替补 + 续接首轮）；在修正 Windows shim 和测试运行根之前还做过两轮到达模型前即失败的诊断。所有记录均为零 tokens、空 identity/session，未观察到成功的模型响应。即使保守地把三次启动后才报存储失败的 Kimi 都计作模型请求，总数也为 3，未超过用户给定的 6 次上限；其余路径有明确的本地启动/凭据锁失败证据。没有继续重试。由于用户明确要求不启动界面，本轮也未做肉眼黑窗检查；代码和进程测试确认所有 Windows spawn 点带 `CREATE_NO_WINDOW`。

## `.dispatch/` 实际路径与 frontmatter 摘录

盲审三路：

```yaml
# D:\work\aicross\.dispatch\20260913-191824-pi-glm-4-7-数据审查.md
schema: aicross-dispatch/1
run_id: task03-blind-20260913-191823
stage: 1
node: blind-pi-glm
agent: pi
provider: zai-coding-cn
model: glm-4.7
role: 数据审查
visibility: 盲
isolation: 部分
workdir: D:\work\aicross\.dispatch/runtime\runs\task03-blind-20260913-191823\blind-pi-glm
materials:
- plan.md
- data.csv
redacted: 0
truncated: false
status: error
tokens_in: 0
tokens_out: 0
identity: ''
session_id: ''
exit_code: 1

# D:\work\aicross\.dispatch\20260913-191824-codex-gpt-5-6-luna-风险审查.md
schema: aicross-dispatch/1
run_id: task03-blind-20260913-191823
node: blind-codex-luna
agent: codex
provider: ''
model: gpt-5.6-luna
role: 风险审查
visibility: 盲
isolation: 部分
materials:
- plan.md
- data.csv
status: error
tokens_in: 0
tokens_out: 0
exit_code: 1

# D:\work\aicross\.dispatch\20260913-191824-kimi-kimi-for-coding-highspeed-方案审查.md
schema: aicross-dispatch/1
run_id: task03-blind-20260913-191823
node: blind-kimi-fast
agent: kimi
provider: kimi
model: kimi-for-coding-highspeed
role: 方案审查
visibility: 盲
isolation: 部分
materials:
- plan.md
- data.csv
status: error
tokens_in: 0
tokens_out: 0
exit_code: 1
```

替补与续接首轮：

```yaml
# D:\work\aicross\.dispatch\20260913-191825-pi-deepseek-v4-flash-替补审查.md
schema: aicross-dispatch/1
run_id: task03-fallback-20260913-191824
node: fallback-for-blind-pi-glm
agent: pi
provider: qwen-token-plan-cn
model: deepseek-v4-flash
role: 替补审查
visibility: 盲
status: error
tokens_in: 0
tokens_out: 0
exit_code: 1

# D:\work\aicross\.dispatch\20260913-191948-pi-glm-4-7-续接验证.md
schema: aicross-dispatch/1
run_id: task03-cont-a-20260913-191946
stage: 2
node: continue-pi-a
agent: pi
provider: zai-coding-cn
model: glm-4.7
role: 续接验证
visibility: 共享
isolation: 部分
workdir: D:\work\aicross
materials: []
status: error
tokens_in: 0
tokens_out: 0
identity: ''
session_id: ''
exit_code: 1
```

`.dispatch/STATE.md` 存在。当前内容对应最后一次续接首轮：

```text
## 当前状态
阶段: 派发第 2 阶段 ｜ run: task03-cont-a-20260913-191946 ｜ 更新: 2026-09-13 19:19:48
## 已完成
## 进行中
- [ ] continue-pi-a: 失败
## 决策记录
- 同阶段并发上限 3；同一 agent 同时最多 1 路
## 下一步
- 检查失败原因后重派或更换通道
```

## 没做什么

- 未做第 4 期的阶段间自动推进，当前一次只派一个可执行阶段。
- 未做 ACP/长驻协议、流式输出或自动采纳；R1 已实现用户手动切换采纳标记并随图持久化。
- 未实现可选的机器归类、独有发现核验/转发按钮。
- 未读取凭据文件，未改变账号、凭据或全局 CLI 配置。
- 未启动界面；因此 G1/G2/G3 的窗口视觉和点击流程未做端到端 smoke。
- 未提交、未 push。

## 不确定处与待补验证

1. 编排者已在普通桌面进程中补跑两条 ignored 验收；结果见下方“修正轮 R1”的“验收 3、4”。盲审三路并未全部成功，pi 的静默换模、Kimi 裸别名和替补 pi 的 403 均据实保留；共享续聊第二轮答出了 `青松-731`。
2. Codex/Kimi 的宿主级指令隔离没有得到成功模型响应来实证，仍记 `isolation: 部分`；Claude 的 `--restricted` 与 pi 的 `--no-context-files` 加整套禁用参数按当前通道契约标完整。
3. 当前本机 Kimi 续接参数与任务书快照不同；实现以 0.42.0 的实际帮助为准，CLI 再升级时仍需冒烟。
4. 指定的 `npx vitest run` 在本环境被 Vite 的 Windows `net use` 子进程限制挡住；普通 Windows shell/CI 仍需用原命令复核一次。

## 修正轮 R1

修正日期：2026-09-13。本节覆盖 `tasks/fix-03-r1.md`，并取代上文中与编排者 19:37–19:41 真跑结果冲突的“未验证”表述。本轮未启动窗口，未运行 `tauri dev`，未提交或推送。

### 逐条处理

1. **已修：pi 真身。** 解析时优先读取响应级 `responseModel`，缺失才回退 `message.model`；与请求的 provider/model 不同即返回 `identity_mismatch`，同时把响应真身写入 `identity`。新增真实事件 fixture，按请求 `zai-coding-cn/glm-4.7` 断言真身为 `zai-coding-cn/glm-5.3-flash`。
2. **已修：pi 错误事件。** `message_end.stopReason == "error"` 时优先返回 `errorMessage`；真实 403 事件 fixture 断言 `AccessDenied.Unpurchased` 进入 error。
3. **已修：pi 用量。** 支持真实键 `input`、`output`、`cacheRead`，fixture 断言 115 / 41 / 1472。
4. **已修：Kimi 模型与续聊。** 模型必须包含 config.toml 的完整别名，否则直接报“`Kimi 模型请使用 config.toml 里的别名`”。发送侧使用本机 0.42.0 help 明示的 `-S`；解析侧只在含 `To resume this session:` 的提示行解析，并兼容该行里的 `-S/--session/-r/--resume`，不会从正文中的 `-r` 误取。R1 真机只发出 1 次起始请求，因用户存储写权限失败而没有 session id，故没有发送两次续聊请求；总请求数 1，未超过本条上限 3。
5. **已修：脱敏。** password/token/api-key 等键和值支持可选引号，带空格、逗号的引号值整体替换；prepare 失败的任务正文固定写“`(未发送)`”。stdout、stderr、error 无论外发开关为何都在留痕和返回前再次脱敏。新增 JSON、失败路径和三类 trace 测试。
6. **已修：Kimi / cc-switch 用量。** Kimi 递归读取 stream-json 的 usage；cc-switch 从 stderr 的 `[usage]` 行解析；界面将 cc-switch 标为“单轮”。
7. **已修：Codex `--ephemeral` 证据。** 本机 `codex exec --help` 列出 `--ephemeral`，说明为“不持久化 session files”；盲审 argv 保留该参数。它不等于宿主上下文完全隔离，故 Codex 仍标“部分”。
8. **已修：Codex thinking。** 映射为关/低→low、中→medium、高→high，不再把“高”映射到 xhigh。
9. **已修：同 agent 串行。** 调度 active key 先经 `normalize_agent`，`kimi-code`/`kimicode` 与 `kimi` 共用一个串行键，`cc_switch` 也归一到 `cc-switch`。
10. **已修：Claude null。** `api_error_status` 只有非 null 才判错误。
11. **已修：E 节。** 回复气泡已有 in/out 角标；G1 增加“新开会话”，清除该目标父链的 session id，并强制本次不续接。
12. **已修：附件路径。** 共享序列化保留绝对路径；盲节点仅在 prompt 中给 basename，复制材料仍使用原路径。
13. **已修：STATE。** 写入本次设置经钳制后的实际并发上限；失败/超时进入独立“失败与超时”，不再列入“进行中”。
14. **已修：live 断言。** 可写用户数据目录的非沙箱环境要求 `status == "ok"` 且 in/out 均大于 0；沙箱探测失败时打印跳过原因。R1 未再次运行 ignored 测试，避免额外模型请求。
15. **已修：argv 测试。** argv 规格构造与二进制查找拆开，单测直接核对规格，不因本机缺 CLI 而静默跳过。
16. **已修：cc-switch 路径。** 删除源码硬编码目录，新增设置项 `ccSwitchPath`，默认空；空值先报“`未配置 cc_switch.py 路径`”，再核对文件存在。
17. **已修：非 Windows 取消。** Unix 使用 `libc::kill(pid, SIGKILL)` 并返回实际结果；其他平台如实返回 false。
18. **已修：状态与阶段。** 用量表状态统一为完成/失败/超时，阶段取实际 `request.stage`。
19. **已修：G2 / 采纳。** G2 行展示角色或节点摘要及 agent/model；running 时不能点背景关闭；采纳状态写入 `Reply.adopted` 并随图持久化。
20. **已修：上下文与裁决。** 仅 `done` 回复进入上下文，failed 排除；“交人裁决”生成普通用户决策节点，不再把 `[系统]` 角色前缀塞进用户文本。
21. **已修：Overlay 整理。** 留痕换行使用真实 JSX 换行；设置只在组件初始化时读一次；界面不再复制 Rust 的隔离判定，预览回来前显示“核对中”。
22. **已修：测试与注释。** `parse_classification` 移入测试模块；更新 OrchestrationPanel 过时注释；盲目录测试明确断言 CLAUDE.md、AGENTS.md 和 `.dispatch` 均未拷入。
23. **已修：正则缓存。** 脱敏、Kimi session 与 cc-switch usage 正则均由 `OnceLock` 缓存。

### 不采纳与后续

- **不采纳：把 G2 改成中栏系统行。** 当前 overlay 内的 G2 已覆盖角色、路由、状态与取消，功能等价；保留实现位置偏差。
- **不采纳：本期接机器归类。** TASK-03 将其列为可选项，保留容错解析测试，第 4 期再接界面动作。
- **不采纳：本轮处理 Codex 389k 输入。** 这是空目录工具读取材料产生的路由成本，不是本轮执行器正确性缺陷；记录为第 4 期路由策略观察。

此外，根据当前 ai-cross 通道契约，pi 盲审同时使用 `--no-context-files`、`--no-tools`、`--no-skills`、`--no-prompt-templates`、`--no-themes`、`--no-extensions`，因此改标“完整”；Codex 仍为“部分”。

### 验收 3、4：编排者真跑事实

验收 3（普通桌面进程，2026-09-13 19:37–19:41）：pi 与 Codex 盲审取得正文和用量，Kimi 因裸模型 id 未配置而失败，替补 pi 因 `403 AccessDenied.Unpurchased` 失败。pi 记录请求 `glm-4.7`，但真实 `responseModel` 为 `glm-5.3-flash`；R1 修正后同样的响应会判 `identity_mismatch`，不再判 ok。证据为：

- `.dispatch/20260913-193721-pi-glm-4-7-数据审查.md`：in 457 / out 675，16.7s，session 有值；
- `.dispatch/20260913-194022-codex-gpt-5-6-luna-风险审查.md`：in 389489（cache_read 354304）/ out 5928，197.8s；
- `.dispatch/20260913-194024-pi-deepseek-v4-flash-替补审查.md`：403 错误事件；
- `.dispatch/20260913-194041-pi-glm-4-7-续接验证.md`：真实 `responseModel` 事件证据。

验收 4：共享 pi 两轮续聊的第二轮答出 `青松-731`，`marker_recalled=true`，in 115 / out 41，说明 session 续接行为实测成立。但这次历史运行仍受当时真身核对缺陷影响；R1 新规则会将静默换到 glm-5.3-flash 的响应判为真身不符。由于本轮明确不启动窗口，Windows 无黑窗仅有所有 spawn 点使用 `CREATE_NO_WINDOW` 的代码/进程测试证据，没有人工视觉确认。

### Kimi R1 限次实测

静态事实：本机 Kimi Code CLI 为 0.42.0，help 列出 `-S, --session [id]`；config.toml 的完整别名包含 `kimi-code/kimi-for-coding`、`kimi-code/kimi-for-coding-highspeed`、`kimi-code/k3`、`kimi-code/k3-256k`。

实际命令（请求 1/3）：

```powershell
$kimi = Join-Path $env:USERPROFILE '.kimi-code\bin\kimi.exe'; $env:AI_CROSS_PEER='1'; & $kimi -p '只回复OK' -m 'kimi-code/k3-256k'; $code = $LASTEXITCODE; Remove-Item Env:AI_CROSS_PEER -ErrorAction SilentlyContinue; "EXIT_CODE=$code"
```

输出原文：

```text
(node:25944) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `kimi --trace-warnings ...` to show where the warning was created)
kimi version 0.42.0
error: failed to run prompt: storage write failed: permission denied
See log: C:\Users\keros68\.kimi-code\logs\kimi-code.log
EXIT_CODE=1
```

失败发生在取得 session id 之前，因此没有可供 `-S` 或 `-r` 续聊的 id；按修正单要求停止，不以新起会话冒充续聊。R1 Kimi 模型请求合计 1 次。

### R1 自动验证（命令与输出原文）

#### `cargo check`（根目录，exit=0）

```text
   Compiling aicross v0.1.0 (D:\work\aicross)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.82s
```

#### `cargo test`（根目录，exit=0）

```text
   Compiling aicross v0.1.0 (D:\work\aicross)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.01s
     Running unittests src\main.rs (target\debug\deps\aicross-a390c24a3e96ccb6.exe)

running 124 tests
test channels::tests::cc_switch_requires_configured_script_path ... ok
test channels::tests::blind_claude_contains_restricted ... ok
test channels::tests::blind_pi_with_no_context_files_is_fully_isolated ... ok
test channels::tests::codex_high_maps_to_high_and_never_to_xhigh ... ok
test channels::tests::codex_resume_follows_parent_options ... ok
test channels::tests::claude_null_api_error_status_is_not_an_error ... ok
test channels::tests::kimi_rejects_more_than_30000_utf8_bytes ... ok
test channels::tests::kimi_requires_configured_alias ... ok
test channels::tests::pi_error_event_surfaces_error_message ... ok
test channels::tests::pi_uses_explicit_session_and_app_owned_session_directory ... ok
test channels::tests::pi_response_model_mismatch_fails_and_reports_true_identity ... ok
test channels::tests::scheduling_aliases_normalize_to_one_agent_key ... ok
test channels::tests::pi_reads_camel_case_cache_usage ... ok
test dispatch::tests::live_blind_review ... ignored, real model requests; run once for TASK-03 acceptance
test dispatch::tests::live_shared_continuation ... ignored, two real model requests; run once for TASK-03 acceptance
test dispatch::tests::classification_parser_tolerates_fences_and_missing_arrays ... ok
test dispatch::tests::frontmatter_roundtrips ... ok
test channels::tests::kimi_session_parser_ignores_resume_text_in_answer ... ok
test channels::tests::kimi_collects_stream_usage_when_present ... ok
test channels::tests::kimi_resume_uses_current_cli_session_flag ... ok
test channels::tests::cc_switch_reads_usage_from_stderr ... ok
test dispatch::tests::trace_fields_are_always_sanitized ... ok
test dispatch::tests::redacts_all_six_secret_shapes_and_stays_clean ... ok
test dispatch::tests::blind_directory_contains_only_selected_materials ... ok
test dispatch::tests::state_uses_actual_limit_and_separates_failures ... ok
test dispatch::tests::preparation_failure_records_only_unsent_placeholder ... ok
test process::tests::run_process_truncates_and_marks_output ... ok
test process::tests::run_process_times_out_and_terminates_the_child_tree ... ok

test result: ok. 120 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 4.45s
```

上段保留全部 R1 相关用例及 Cargo 原始收尾；其余 94 项既有用例逐项输出均为 `ok`，未在报告中重复展开。

#### `cd src-ui && npx vitest run`（exit=1）

```text
(node:13316) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:45932) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
failed to load config from D:\work\aicross\src-ui\vite.config.ts

⎯⎯⎯⎯⎯⎯⎯ Startup Error ⎯⎯⎯⎯⎯⎯⎯⎯
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:458:11)
    at spawn (node:child_process:813:9)
    at Object.execFile (node:child_process:349:17)
    at exec (node:child_process:236:25)
    at optimizeSafeRealPathSync (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:1954:2)
    at windowsSafeRealPathSync (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:1940:3)
    at getRealPath (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32413:36)
    at tryResolveRealFileOrType (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32407:9)
    at tryCleanFsResolve (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32151:21)
    at tryFsResolve (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32144:14)
```

失败发生在测试文件加载前，符合修正单允许的沙箱分支；随后使用指定 fallback。

#### `cd src-ui && npx vitest run --configLoader native --pool threads`（exit=0）

```text
(node:35076) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:28864) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 RUN  v5.0.0 D:/work/aicross/src-ui

 Test Files  6 passed (6)
      Tests  61 passed (61)
   Start at  20:15:23
   Duration  202ms (transform 63%, import 18%, tests 12%, worker 7%)
```

上段省略的是同一 PID 重复 6 次的 `NO_COLOR` warning；测试统计为原文。

#### `cd src-ui && npm run build`（exit=0）

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:25280) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
✓ 575 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                      2.51 kB │ gzip:   1.13 kB
dist/assets/main-DGDpbfzQ.css                       148.87 kB │ gzip:  25.93 kB
dist/assets/main-C2Rhvm_A.js                      1,252.12 kB │ gzip: 445.78 kB

[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
✓ built in 2.10s
```

Vite 在三条主要产物之间逐项打印动态语法资源；报告保留命令头、主要产物和原始收尾，未改写状态或数值。chunk 体积提示为既有提示。

## 编排者补记（2026-09-13 20:10–20:37，沙箱外）

修正轮 R1 之后编排者独立复跑并补做了实现方在沙箱里做不了的部分：

1. **复跑**：`cargo test` 120 passed / 4 ignored，`npx vitest run` 61 passed（6 文件），`npm run build` 通过。
2. **kimi 续聊实测**（普通桌面进程，4 次请求）：`kimi -p "记住口令 青松-731。只回复OK" -m kimi-code/k3-256k` 得到 `To resume this session: kimi -r session_cd6a…`；`kimi -S <id> -p "口令是什么"` 与 `kimi -r <id> -p …` 均正常续接，`-S` 回答 `青松-731`。结论：0.42.0 的 `-S` 与提示行里的 `-r` 都能续，发送侧用 `-S` 正确。
3. **kimi 解析缺陷（编排者直接修）**：R1 后首次真跑 kimi 报「未返回正文」。留痕 `20260913-202357-kimi-*.md` 显示 0.42.0 的最终回答是 `{"role":"assistant","content":"<纯字符串>"}`，工具轮是 `tool_calls`，工具结果是 `role: tool`，末尾另有 `{"type":"session.resume_hint","session_id":…}` 元事件；原解析只认 `{"type":"text"}` 块。改 `parse_kimi`：收 assistant 的字符串 content、跳过 tool 事件、session 优先取 `session.resume_hint`；新增测试 `kimi_reads_plain_string_assistant_content_and_resume_hint`（用真实事件形状）。`cargo test channels::tests` 18 passed。
4. **live 验收 fixture 改模型**：zai 端点把 `glm-4.7` 静默换成 `glm-5.3-flash`，R1 的真身规则会正确判 `identity_mismatch`，所以 fixture 的 pi 路线改为 `glm-5.3-flash`；替补路线改走 DeepSeek 官方 provider `deepseek`（qwen plan 下的 deepseek-v4-flash 无权限）。live 断言对 kimi 不要求 tokens > 0（其 stream-json 不带 usage，任务书 A 表已写「否则 —」）。
5. **验收 3 最终真跑**（`cargo test dispatch::tests::live_blind_review -- --ignored --nocapture`，20:34–20:37，3 次模型请求，替补未触发）：

   | 路 | 状态 | isolation | 真身 | 留痕 |
   |---|---|---|---|---|
   | pi/zai-coding-cn/glm-5.3-flash | ok | 完整 | zai-coding-cn/glm-5.3-flash | `.dispatch/20260913-203420-pi-glm-5-3-flash-数据审查.md` |
   | codex/gpt-5.6-luna | ok | 部分 | —（官方，不核对） | `.dispatch/20260913-203631-codex-gpt-5-6-luna-风险审查.md` |
   | kimi/kimi-code/kimi-for-coding-highspeed | ok | 部分 | — | `.dispatch/20260913-203400-kimi-kimi-code-kimi-for-coding-highspeed-方案审查.md` |

   `test result: ok. 1 passed`。
6. **验收 4 真跑**（`live_shared_continuation`，20:27，模型 glm-5.3-flash）：两轮 ok，`marker_recalled=true`，`test result: ok. 1 passed`。留痕 `.dispatch/20260913-202737-pi-glm-5-3-flash-续接验证.md` 及其后一份。
7. 编排者改动的文件：`src/channels/mod.rs`（parse_kimi + 测试）、`src/dispatch.rs`（live fixture 模型与断言）。
8. **R2 复审后补修（GLM R2 唯一未闭合项）**：Rust `Reply` 镜像补 `adopted: Option<bool>`（`src/orchestration.rs`），`graph_roundtrip` 的样例回复置 `adopted: Some(true)`，往返断言覆盖。GLM R2 结论：23 条中 22 条核实修好，可提交进入第 4 期（`tasks/review-03-glm-r2.md`）。
9. **qwen R2 复审后补修**：回复正文写进 `Reply` 前也过 `safe_trace`（`src/dispatch.rs`），模型回显的凭据不再进 `~/.aicross/graphs/*.json`。qwen R2 结论：22/23 核实修好，可提交进入第 4 期（`tasks/review-03-qwen-r2.md`）；其余未闭合项（真身判定类型化、cc-switch 路无真跑、running 期模态锁中栏、stage 硬编码 1、大输入阈值告警、决策行缺「理由」段、live prompt 未走 serializeContext）转入 TASK-04 遗留清单。
