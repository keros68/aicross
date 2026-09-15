# fix-16-r1 控制台通道（TASK-16）复审修正轮：通道改成薄中继，派发走界面既有流水线

依据 GLM-5.3-Flash 与 DeepSeek 对 phase/16 的盲审（`tasks/review-16-glm.md`、`tasks/review-16-ds.md`）。首版把派发在 Rust 侧另造了一条路（自己拼 prompt、整图覆盖、默认设置），四条高危同一个根因。编排者定案：**通道只做校验与中继，派发本身走前端既有的流水线**（stamp → G1 规则 → runStageLoop → applyDispatchResults），因为角色模板、写护栏、上下文拼接、设置、图的单写者、进度心跳全在那条路上。契约只追加。

## 定案的机制

- Rust `POST /v1/dispatch`：校验令牌、项目目录存在、请求体；生成 `run_id`；把请求（project、text、preset、targets、attachments、run_id）通过 Tauri 事件 `aicross:console-dispatch` 发给前端；同步返回 `{ run_id, accepted: true }`（graph_id / node_ids 由后续状态接口给）。前端不在（窗口未起）或 5 秒内没回执 → 503「控制台界面未就绪」。
- 前端收到事件：找到该项目当前打开的图（tab 的 `folderPath` 匹配；没有则新建并标 `source=console`），在根/末端节点上 `stamp()` 指定预设（默认经济模式），按 G1 的同一套判定（额度用尽、未验证、只读、请求上限、脏目录）决定可派集合——不可派的原因通过 `console_run_update` 命令回报 Rust，Rust 记成该 run 的 `status: rejected` + 原因（`GET /v1/runs` 返回）；可派则调用与 DispatchOverlay 相同的启动函数（抽成 `startPipelineHeadless(graph, nodeIds, settings)`，DispatchOverlay 的 `start` 改为调用它），沿用 `runStageLoop`、`applyDispatchResults`、修正轮、遗留、红绿核对、进度 store。
- 前端在每个阶段结束、每个节点结果落图后调用 `console_run_update`（run_id、graph_id、节点状态摘要、leftover 摘要、in-flight 节点 id 列表）；Rust 只保存这份摘要给 `GET /v1/runs/<id>`，不再自己写图。
- `adopt` / `cancel`：Rust 转发事件给前端，前端走结果卡同一函数（`setReplyAdopted` / `cancelDispatchNode`），回执后返回。
- 结果：DeepSeek 1（prompt 另造）、2（双写覆盖）、3（图归属）、4（设置）、10（在飞计数）一并消失；界面「采纳/取消采纳」改回首版之前的前端路径（撤掉「共享 Rust 采纳入口」的改动，避免回归）。

## 必修（两家共同：1/2/3/4/6/7；DeepSeek 独有：5/8；GLM 独有：9）

1. 按上面的机制重做 `src/console_api.rs`（去掉 `run_prepared`、整图写盘、`build_node_request`、`DispatchSettings::default()`），Rust 只保留：清单、令牌、路由、事件转发、run 摘要表（容量上限 200，超过按最旧淘汰）。
2. 前端 `src-ui/src/console-channel.ts`：事件处理、图选择（`graph.project` 真正写到 Graph 上，`newGraph` 顺手改；旧图无 project 时按 tab 目录匹配）、`startPipelineHeadless`、`console_run_update` 回报、adopt/cancel 转发；总指挥 tab 前台时不抢焦点保留。
3. 安全（DeepSeek 5、6、7 / GLM）：令牌常量时间比较；带 `Origin` 或 `Sec-Fetch-Site: cross-site` 的请求直接 403；令牌校验在读 body 之前（先 401 再丢 body）；`OPTIONS`/`HEAD` 不进 JSON 行分支、不刷日志；清单写入失败只告警并标「通道不可用」，hook server 照常启动；Windows 用 `%SystemRoot%` 找 icacls/whoami。
4. CLI（DeepSeek 8、13、16）：`status --wait` 加 `--timeout <secs>`（默认 600）到点非零退出；每轮重读 `console.json`；读清单时用 `pid` 判活，进程不存在 → 删清单并报「控制台未运行（清单已过期）」；dev 构建用独立清单名 `console.dev.json`；`--text-file` 剥 BOM、非 UTF-8 给明确提示；`--text=` 一并拒绝。
5. 安装包 PATH（DeepSeek 9）：NSIS 只增删自己那一项、保留 `REG_EXPAND_SZ` 与 `%VAR%` 形式（用 EnVar 插件或逐项解析），匹配大小写与尾反斜杠；`Path` 不存在时不写空值；WiX 同口径；把源码字符串断言换成对 PATH 解析纯函数的测试。
6. 契约与文档（DeepSeek 11、12）：`GET /v1/runs` 的 `status` 增 `rejected`，节点状态保留原状态词（额度用尽不算 failed）；`leftover` 真填（从遗留节点摘要）或从契约删；文档补状态码表（400/401/403/404/409/413/503）、退出码表（0/1/2/3）、预设别名表、平台可用性（只有 Windows 安装包改 PATH）。
7. 在飞上限（DeepSeek 14）：同时在飞 run 数上限 3（设置项，默认 3），超过 409；worktree 目标命中脏目录时按 G1 同一提示写进 run 的 rejected 原因。
8. 测试（DeepSeek 15）：事件中继往返（假前端回执）、rejected 原因回传、run 摘要容量、常量时间比较、Origin 403、body 前 401、pid 判活、PATH 纯函数、`startPipelineHeadless` 与 DispatchOverlay 共用同一函数的接线断言；删掉同义反复。

9. **GLM 补充**：`cancel` 对已结束的 run 不改状态（返回当前状态）；CLI 对「HTTP 200 但响应非 JSON」不归为退出码 3（用 1 并打印原文首行）；HTTP 客户端加总超时；Windows 清单先在临时文件上收紧 ACL 再原子替换（不先写正文后收紧）；额度用尽在 run 摘要里保留原状态词与恢复时间；NSIS 卸载只在 Path 里确有本项时才回写。

## 保持

- `console.json` 的写/删/权限、`aicross` 子命令骨架、退出码 3 语义、「来自总指挥」标记、任务板行、hook server 首行分流——核过没问题，按上面调整后保留。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF；`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-16-R1.md`（逐条对应本清单）。
