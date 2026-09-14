# TASK-04 修正轮 R3（窗口真派实测 + 两家复审的未闭合项）

先读 `AGENTS.md`、`tasks/TASK-04-pipeline.md`、`tasks/REPORT-04.md`（含修正轮 R1、R2），再读 `tasks/review-04-glm-r2.md` 与 `tasks/review-04-ds-r2.md`（GLM-5.3-Flash 与 DeepSeek 对 R1/R2 的复审，两家都判「可交付但有未闭合项」）。逐条处理，每条在 `tasks/REPORT-04.md` 末尾新增「修正轮 R3」一节里写「已修：怎么改的（文件:行）」或「不采纳：原因」。验证跑 `cargo check`、`cargo test`、`cd src-ui && npx vitest run`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不起窗口。不要 commit，不要 push。真机请求本轮不发（编排者修完后在窗口里再派一次）。

## 编排者窗口真派（2026-09-14 08:38–09:05，合并后 main，dev 窗口）

流程：接入页全部探测 → 冒烟 codex/gpt-5.6-luna、pi zai-coding-cn/glm-5.3-flash、kimi/k3-256k 均「已接入」 → 设置·角色 三个角色选好并落 `~/.aicross/roles.json` → 左栏工作区选临时 git 项目 → 新建图、发根句 → 右栏「应用」经济模式 → 派发。结果：

- 出方案（pi/glm-5.3-flash）ok：in 118 / out 1932 / 48.6s，留痕 `.dispatch/20260914-085235-pi-glm-5-3-flash-出方案.md`。
- 自动推进第 2 层时执行节点是「未指定模型」，仍被派出，后端报 `不支持的派发 agent：`，status=error，留痕 `20260914-085235---执行.md`（agent 为空串）。
- 结果屏渲染崩溃：`Cannot read properties of undefined (reading 'length')`，整个对话图 tab 进 ErrorBoundary。
- 编排者绕行（关 tab、直接改图文件给执行/验收填模型、从「已有图」重开）后：执行（codex/gpt-5.6-luna）ok：in 101825 / out 1145 / 72s，项目里 `roman.py` 改了、`test_roman.py` 新建了；随后前端在组装第 3 层（盲验收）时抛 `TypeError: Cannot read properties of undefined (reading 'map')`，catch 分支把**已经成功的执行节点**改写成 failed，可派 0 路，流水线卡死。第 3 层与 G4 未跑到。

## 必修 A：窗口实测缺陷（按严重度）

1. **产物 IPC 形状不一致（崩溃根因）**：`src/dispatch.rs:110` `DispatchArtifacts` 带 `#[serde(rename_all = "camelCase")]`，序列化成 `changedFiles`/`stdoutSummary`；前端 `src-ui/src/graph/types.ts:33` `ReplyArtifacts` 是 `changed_files`/`stdout_summary`，`DispatchOverlay.tsx:410` 与 `context.ts:70` 直接 `.changed_files.length/.map` 就炸。`ChangedFile` 同查。修法：去掉这两个 struct 的 camelCase（与 `orchestration.rs` 的 `ReplyArtifacts`/`ReplyChangedFile` 完全同形，最好直接复用同一个类型），前端所有访问改成 `artifacts?.changed_files ?? []` 的防御式写法。补测试：Rust 侧断言 `serde_json::to_value(DispatchArtifacts)` 的键名是 `diff/changed_files/worktree/stdout_summary`；vitest 侧结果屏/`artifactSummary` 传入无 `changed_files` 的对象不抛。
2. **循环异常不得改写已完成节点**：`DispatchOverlay.tsx` `start()` 的 catch 把 `activeNodes` 全标 failed，但此时它们的结果可能已经 ok 并 `applyDispatchResults` 写回。改为：只把「本轮没有拿到结果」的节点标 failed；有 ok 结果的节点保持 done；异常文本进 `runError` 显示。补测试（把这段抽成纯函数）。
3. **逐层推进前拦「未指定模型」**：`start()` 循环里每层 `ids` 算出后，若有节点 `agent`/`model` 为空，停下来：phase 回 `confirm`、`nodeIds` 设为该层、G1 列出这些节点让用户选目标（首层现在就是这么做的，`hasMissingTargets` 只看了首层）。后端 `dispatch_graph_stage` 也在拍快照、拿写锁之前先校验 agent 非空，空的直接 status=error 且不产生空产物、不写 `---执行.md` 这种 agent 为空串的留痕文件名（文件名里 agent 段为空时用 `unspecified`）。
4. **角色默认不生效**：设置·角色保存 `roles.json` 后，右栏「应用」盖出来的节点仍是「未指定模型」。原因：`list_presets` 在加载时 `fill_role_defaults`，`OrchestrationPanel.tsx:45/64` 与 `GraphTab.tsx:103` 只在挂载时 `listPresets` 一次。修法：`SettingsModal` 保存角色成功后 `window.dispatchEvent(new CustomEvent('aicross:presets-changed'))`，两处监听它与 `aicross:resources-changed` 重拉预设；另外盖预设时若目标 `model` 为空，前端再按 `loadRoles()` 填一次兜底（抽纯函数 `applyRoleDefaults(preset, roles)`，补测试）。
5. **失败节点在图上没有重派入口**：结果屏关掉后（本次是崩溃关掉），失败节点只剩「失败」两个字，`dispatchableNodeIds` 不含它，可派 0 路。给节点卡与右栏节点面板加「重派」（走现有 `redraftNode`，允许先在节点面板改目标再重派）。补测试：failed → redraft → 出现在 `dispatchableNodeIds`。
6. **DeepSeek 官方路由别名被判真身不符**：pi `deepseek/deepseek-v4-flash` 冒烟失败「真身不符：请求 deepseek/deepseek-v4-flash，响应 deepseek/deepseek-flash」。用户已说明 DeepSeek 官方现在统一路由到 `deepseek-flash`，`deepseek-v4-flash` 是别名。在 `src/channels/mod.rs` 真身核对处加一张明示的路由别名表 `KNOWN_MODEL_ROUTES`（provider、请求 id、响应 id 三元组，首条 `("deepseek","deepseek-v4-flash","deepseek-flash")`），命中即通过、`identity` 记响应 id；资源冒烟同样走通，`smoke_model` 记实际 id，meta 追加「路由别名：deepseek-v4-flash→deepseek-flash」。**不要**放宽 zai：`glm-4.7→glm-5.3-flash` 静默升级仍必须报真身不符（已有测试保住）。补测试两条。
7. **留痕文件把整条 pi 流写进去**：出方案留痕 564KB，`## 原始输出` 是 pi 的完整 jsonl（含 thinking 增量）。`dispatch.rs:1545` 改为：原始输出 ≤ 32KB 原样写；超过则写到 `~/.aicross/runs/<run_id>/<node_id>.raw.txt`，留痕里只留一行指针 + 末尾 4KB。补测试。
8. **未选工作目录时不得回退到进程 cwd**：`dispatch.rs:1496` `project_path("")` 回退 `current_dir()`（dev 下就是 aicross 仓库本身，单写者会往里写）。改为 `Err("请先在左栏「工作区」选择项目目录")`；`preview_dispatch` 同样报错；G1 在 `projectDir` 为空时显示该提示并禁用「确认派发」。补测试。
9. **G1 决策行补写权限与验证命令**：现在只显示档位/thinking/可见性/隔离，看不出这一路会改项目文件。加「写=只读/单写者/worktree」，设置了验证命令时显示「验证=<命令>」。
10. **关闭图 tab 后右栏仍显示旧图的节点链**：关 tab 时清 `graphFocus`（右栏回到「在图上选一个节点」）。
11. **接入页「详情」鼠标点不开**：模型行的 `<details><summary>详情</summary>` 用鼠标点击不展开（codex、pi 两张卡都试过），键盘 Tab+Enter 能开，同页的按钮鼠标都正常。先在代码里找原因（`res-dialog-backdrop onMouseDown={onClose}` / 内层 `stopPropagation` / 卡片上是否有拦截 mousedown 的东西 / WebView2 对 summary 的 click 处理），改成受控 `open` 状态 + `onClick` 切换，不再依赖原生 summary 行为。报告里写清你找到的原因；若复现不了要说明用了什么方法。

## 必修 B：两家复审一致的未闭合项

12. **产物未过脱敏**（qwen 3.1，两家复审都列首位）：`diff`、`changed_files[].content`、`stdout_summary` 与盲目录里的 `diff.patch`/`changed/` 全部先过 `redact_text(settings.redact)`（六类），命中数累加进 `redacted`。补测试：diff 里的 `sk-…` 被打码。
13. **git 不可执行时回退内置 diff**（DeepSeek 新缺陷 1）：`project_is_git_work_tree` 探测失败按 `false` 处理走 `fallback_diff_for_paths`，不得让跑完的模型请求变 error；`materialize_snapshot` 失败要清理 `~/.aicross/runs/<run>/diff-*/` 临时目录（用 guard/`defer` 式清理）。补测试（PATH 里没有 git 的场景可用假 git 路径注入）。
14. **验收/盲节点不得拿写权限**（qwen 3.2）：`validate.ts` 对 `role=验收` 或 `visibility=盲` 且 `write != 只读` 报错；后端 `run_one` 对盲节点强制只读（不拿写锁、不拍快照）。补测试。
15. **R1-5 残留提示**：G1 在 `write ∈ {单写者, worktree}` 且 `agent = cc-switch` 时提示「cc-switch 单轮无工具，产物将为空」；kimi 盲验收显示「kimi 无只读档，隔离=部分」。`fill_role_defaults` 的 `_models` 死参删掉。
16. **worktree 节点不跑验证命令**（qwen 2.11）：worktree 是干净检出没有依赖，验证命令几乎必失败。worktree 节点跳过验证并在 `validation_output` 写「worktree 节点未跑验证命令，采纳合并后再验」。
17. **恢复与放弃**（qwen 2.12/2.13）：`continueUnfinished` 沿用 STATE 头里的 `run_id`；`abandon_dispatch_state` 把 `STATE.md` 改名为 `STATE.abandoned-<时间戳>.md` 而不是删；`cancelled_dispatches` 在 run 结束时清掉该 run 的键。
18. **live 测试假绿**（qwen 6.2）：`live_cc_switch_route` 缺 `AICROSS_CC_SWITCH_PATH` 时 `panic!` 明示缺什么，不再 `println + return`。
19. **REPORT 措辞与代码对齐**：R1-5、R1-14 两段按现状重写（R1-5 改法已被 R2#2 取代、R1-14 由 R2#10 的过滤实现取代）；REPORT:272「任务书允许的 fallback」改「修正单允许的」。

## 次要（一并做）

20. 小任务启发式偏严：194 字、含「实现 + 新建测试 + 验证命令」的任务被判「单步小任务」。`isSmallTask` 增加：文本含「测试/验证/新建/并」等多步词或含换行时不判小任务；阈值降到 120 字。改测试。
21. `git diff` 路径改写 `replace("a/before/","a/")` 只作用于 `---`/`+++`/`diff --git` 三种行头，不碰内容行（GLM nitpick 1）。
22. `maxRequests` 小于首层节点数时 G1 直接提示并禁用确认，而不是进结果屏才报错（GLM nitpick 2）。

## 不采纳

- 跨进程孤儿进程（R1-9 局限）：pid 登记表进程内，重启后杀不掉上一实例子进程。留第 5 期做 pid 文件；本轮只在「继续」提示里加一句「若上次是崩溃退出，请先手动检查残留进程」。
- `fixtures/prompts` 在 Vite root 之外（DeepSeek 新缺陷 2）：dev 窗口实测已正常渲染，`server.fs.allow` 未拦；不动。
