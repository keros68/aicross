## 总判：可交付但有未闭合项

以下按严重度排序。我核的是工作区未提交状态（含新文件），未能执行 `git diff`（本次只有只读文件工具），所以逐条以文件实际内容为准。

---

### 1【高】控制台派发的提示词漏掉角色模板与写护栏——prompt 组装是另造的一条路

- **文件:行**：`src/console_api.rs:511-545`（`build_node_request` 只拼 `[用户]\n{original}` + 上游回复）；对照 `src-ui/src/graph/dispatch.ts:1136`（`promptWithWriteGuard(promptForRole(role, prompt), write)`）与 `src-ui/src/graph/dispatch.ts:402-423`（模板来自 `fixtures/prompts/*.txt`）；引擎原样使用该正文：`src/dispatch.rs:3784-3790`（`prepare_outbound` 只做脱敏）。
- **问题**：`fixtures/prompts/execution.txt` 里那两句（「需要精确数值的部分必须写代码并执行…不得心算」「不得 git push、发布、安装依赖或调用付费接口；这些留给用户在验收之后做。」）在 Rust 侧没有任何运行时引用（`grep 不得 push / 任务契约 / 是否完整实现` 在 `src/` 只命中一处断言模板文件的测试 `src/dispatch.rs:5096-5100`）。验收/验证/出方案的模板同样只在界面路径生效。界面的 `serializeContext`（`src-ui/src/graph/context.ts:184`）在 Rust 侧也不存在，所以「继续该图」只有图连续、没有上下文连续（新请求节点的上游回复为空，祖先的用户消息不会进 prompt）。
- **复现**：`aicross dispatch --project <dir> --preset economy --text-file p.txt`，看 `.dispatch/<run>/` 留痕或节点 prompt → 执行节点正文里没有写护栏与角色模板；`--preset blind` 的验收/验证节点也没有验收清单。同一份预设从界面 G1 派发则带全。
- **建议**：把角色模板 + 写护栏做成两侧唯一来源（`include_str!` 同一批 fixtures，或把「拼 prompt + 选材料」抽成引擎命令），`build_node_request` 调它；上下文按 `serializeContext` 的口径拼（至少父链用户消息）。

### 2【高】同一张图被界面与通道双写：整图覆盖会把采纳/编辑抹掉

- **文件:行**：`src/console_api.rs:464-505`（`run_prepared` 用 `let mut graph = prepared.graph` 把整张图驻留内存到整轮跑完，每阶段 `orchestration::save_graph(graph.clone())` 整图回写；`src/console_api.rs:559` 的 `old_adopted` 取自这份陈旧副本）；`src-ui/src/components/center/CenterPanel.tsx:495-511`（收到任何 console 事件都 `loadGraph` + `SET_GRAPH`，含 `progress`）；`src-ui/src/components/center/graph/DispatchOverlay.tsx:781-785`（采纳改走 Rust 读改写）；`src-ui/src/components/center/graph/DispatchOverlay.tsx:776-777`（擂台「选定」仍走 `pickOne` + 整图 commit）。
- **问题**：通道 run 存活几分钟到几十分钟，期间任何界面侧写入（采纳、选定、节点面板编辑、蜂群合并）都会被通道的下一次整图 `save_graph` 用陈旧副本覆盖；界面又在每次 console 事件后回读磁盘，于是「被覆盖」会显式反映到界面上。`save_graph` 本身是原子的（`write_json_atomic`），丢的是语义不是文件。
- **复现**：控制台派一个多阶段预设 → 等第一阶段完成、run 还在跑时，在同一张图的界面上编辑某节点的 prompt（面板改动即写盘，`GraphTab.tsx:235` → `commitGraph`），或对上一轮的节点点「采纳」→ 阶段 2 的 `save_graph`（`console_api.rs:473/496`）把这次改动/采纳从磁盘抹掉，随后的 `progress` 事件把旧值拉回界面。
- **建议**：通道每次写盘前重新 `load_graph`（或改成按节点合并 Reply，而不是整图覆盖）；`progress` 类事件只更新进度 store，不 `SET_GRAPH`；「选定」也走同一个写入入口，并考虑给图加版本号/`updated_at` 乐观锁。

### 3【高】界面建的图永远不会被控制台继续——`project` 归属链在界面侧断了

- **文件:行**：`src-ui/src/graph/types.ts:174-194`（`newGraph(project)` 把 `project` 塞进了**根节点** `root` 的展开里，返回的 Graph 本身没有 `project`）；`src/console_api.rs:450-455`（`latest_project_graph` 按 `graph.project` 过滤）；`src/orchestration.rs:49-72`（`GraphNode` 无 `project` 字段、且无 `deny_unknown_fields` → 存盘时被静默丢弃）。前端唯一读 `graph.project` 的地方是 `CenterPanel.tsx:509`。
- **问题**：界面上任何图的 `graph.project` 都是空串，所以「该项目的当前对话图」只能命中通道自己建的那张；用户为同一工作区手建的图不会被追加，语义上离任务书「节点长在当前工作区的对话图上」差一半。TypeScript 不报错是因为对象展开属性不做过量属性检查，所以 `npm run build` 通过掩盖了这条。
- **复现**：工作区 tab → 新建对话图（`GraphTab.tsx:331` 传了 `session.folderPath`）→ 在同一目录执行 `aicross dispatch` → 磁盘上出现**第二张**图（`~/.aicross/graphs/*.json`），界面 tab 里多出一个「来自总指挥」的新图，而不是续上用户那张。
- **建议**：`newGraph` 把 `project` 写到 Graph 上（顺手从根节点去掉），或让 `latest_project_graph` 兼容「图无 project 时看根节点 project / 看该图当前打开的 tab 目录」。

### 4【高】控制台派发固定用 `DispatchSettings::default()`：验证命令不跑、上限用死值 8

- **文件:行**：`src/console_api.rs:485`（`settings: DispatchSettings::default()`）、`src/console_api.rs:406`（409 预检也用 `DispatchSettings::default().max_requests`）；界面设置来自 localStorage（`src-ui/src/graph/dispatch-settings.ts:34-66`、`DispatchOverlay.tsx:188-198`）；引擎取用点 `src/dispatch.rs:3458-3463`（`run_validation(&settings.validation_command, …)`）与 `src/dispatch.rs:2648-2651`（空命令直接 `Ok("")`）。
- **问题**：默认 `validation_command` 为空 → 可写执行节点的验证门、红绿核对（TASK-13）、「验证命令未通过」的重派/修正轮在控制台路径上全部静默失效；用户把「总请求上限」调到 20 也没用（控制台按 8 判 409，且 8 是整轮累计判据）；`timeout_secs/concurrency/fix_round_limit/swarm_limit` 同理取默认。
- **复现**：设置页填「验证命令 = python -m pytest」→ 从界面派发会看到「验证结果」段与 `validation_failed` 分支；从 `aicross dispatch` 派同一预设，结果卡没有验证结果、`status` 直接 ok。另：某个用户自定义预设累计超过 8 路时，界面可派、控制台 409。
- **建议**：把界面设置落一份 Rust 可读的事实源（`~/.aicross/dispatch-settings.json`）并在两条路都读它，或让请求体带 `settings`（文档里已允许扩展字段），至少在 409 原因里点名用的是默认值。

### 5【中】清单权限收紧失败会连带停掉既有 hook 通道

- **文件:行**：`src/hook_server.rs:59-61`（`write_manifest(...).map_err(anyhow::Error::msg)?`）、`src/server.rs:4002-4010`（失败只打日志，`hook_port` 留 0 → `terminal.rs:755-767` 不再注入 `AICROSS_HOOK_PORT`）、`src/console_api.rs:665-679`（Windows 硬编码 `C:\Windows\System32\whoami.exe` / `icacls.exe`，失败即 Err）。
- **问题**：这是把「清单加固」的失败升级成「hook server 启动失败」，动态岛的状态灯（既有功能）跟着一起死。系统盘不是 C:、策略拦 icacls/whoami、家目录在只读/不支持的卷（Unix 上 `chmod` 失败）都会命中。
- **复现**：Windows 上把 `C:\Windows\System32\icacls.exe` 的执行挡掉（或用非 C: 的系统盘）→ 日志 `[hook-server] start failed: 设置控制台清单权限失败`，所有 tab 的状态灯不再更新；CLI 侧则一律「控制台未运行」。
- **建议**：hook server 先起、清单写入失败只告警并标记「通道不可用」（CLI 侧仍然给退出码 3）；路径用 `%SystemRoot%`；再加一个不依赖外部 exe 的兜底（`GetNamedSecurityInfo`/`SetNamedSecurityInfo` 或 `%USERPROFILE%` 继承 ACL 说明）。

### 6【中】令牌比较不是常量时间

- **文件:行**：`src/console_api.rs:170-171`（`if authorization != Some(expected.as_str())`，`String`/`&str` 短路比较）。
- **问题**：任务书自问的「常量时间比较」没做到。可利用性很低（同用户进程本来就能读 `console.json`，`0600` 之外没有额外的密钥边界），但不满足要求。
- **复现**：代码审查即可；`cargo test` 那条 401 测试只断言状态码。
- **建议**：改成字节切片异或累加比较（长度不等直接返回 401），或引 `subtle`。

### 7【中】`Origin` 未拒绝，且 401 发生在读完正文之后

- **文件:行**：`src/hook_server.rs:116-160`（先读头、读 body，再调 `console_api::route`）、`src/console_api.rs:170-176`（令牌校验在路由前、但已在 body 之后）。
- **问题**：结论是**没有可写路径**——浏览器跨站没法带 `Authorization`（自定义头触发预检，`OPTIONS` 落到 JSON 分支被拒），DNS rebinding 也拿不到令牌，所以读到的最好结果就是 401；但未鉴权的本机进程可以让服务端先分配最多 4 MiB 缓冲，且没有任何 `Origin`/`Sec-Fetch-Site` 拒绝逻辑，属于零成本纵深防御缺口。另外 `OPTIONS/HEAD` 会被当 JSON 行解析并打 `bad JSON` 日志（刷日志）。
- **复现**：浏览器页面 `fetch('http://127.0.0.1:<port>/v1/dispatch', {method:'POST', headers:{'Content-Type':'text/plain'}, body:'{}'})` → 请求确实到达，返回 401，无副作用。
- **建议**：带 `Origin`（或 `Sec-Fetch-Site: cross-site`）直接 403；令牌校验挪到读 body 之前（先回 401 再读/丢 body）。

### 8【中】`status --wait` 没有超时，且清单只读一次

- **文件:行**：`src/console_cli.rs:39-60`（500 ms 轮询、无上限；`manifest` 在循环外只读一次）。
- **问题**：run 卡在 `running`（某个节点挂到引擎 600 s 超时、或 `save_graph` 失败后 `run_prepared` 提前 break 但节点仍留在 running）时，CLI 永久阻塞；控制台中途重启也不会重新发现新端口/新令牌，只会一直打旧端点直到传输错误 → 退出码 3（措辞「控制台未运行」此时是误导）。
- **复现**：派一个必然超时的节点，然后 `aicross status <run_id> --wait`（无 `--timeout`），观察永不返回；再把控制台重启，观察仍打旧端口。
- **建议**：加 `--timeout <secs>`（默认如 600）到点退出非零并打印一行；每轮迭代重读 `console.json`；把「端点不通但清单存在」与「清单不存在/无效」在 stderr 上分开说。

### 9【中】安装包 PATH 是整段展开回写

- **文件:行**：`installer.nsi:679-690`（安装追加）、`installer.nsi:777-789`（卸载删除）、`windows/fragments/path.wxs`、`tauri.conf.json`（`fragmentPaths` + `componentRefs: ["AicrossConsolePath"]`）；测试只有字符串断言（`src-ui/src/__tests__/console-channel.test.ts:48-54`）。
- **问题**（逐条事实）：① 用 `ReadRegExpandStr` 读、`WriteRegExpandStr` 写**整段**用户 PATH，`%VAR%` 形式的项会被展开成字面量后写回，值的类型也会被改成 `REG_EXPAND_SZ`；② `Path` 原本不存在时也会写一个空值；③ 幂等只覆盖「同一 $INSTDIR 字面量」这一种：带尾反斜杠、大小写不同、或用户手工又加了一次的项不会被识别/清理；④ 卸载只按 `;$INSTDIR;` 精确匹配删除，别项不动（不会误删别项），但正因为整段回写，别项的写法会被改；⑤ 无论安装模式都只改 HKCU（`tauri.conf.json` 未设 installMode，当前默认 currentUser，所以现在自洽；将来改 perMachine/both，其它用户拿不到 `aicross`）；MSI 是 per-machine 而 WiX 项 `System="no"`，只有安装者能用该命令。另外 NSIS 变量长度上限与 Windows PATH 上限不同量级，超长用户 PATH 是否被截断需要真机验证（本次未生成安装包）。
- **复现**：装完在注册表把 `HKCU\Environment\Path` 改成含 `%USERPROFILE%\bin` 的值，卸一次 → 变成长路径字面量。
- **建议**：用 EnVar 插件（或直接注册表 API）只增删自己那一项、保留 `REG_EXPAND_SZ` 与变量形式；安装时匹配大小写与尾反斜杠；真机跑一遍装/升/卸，并把断言从「源码含某字符串」换成对模板变量的实际渲染检查。

### 10【低-中】「N 路在飞」把尚未派发的阶段也算进去

- **文件:行**：`src/console_api.rs:281-291`（run 建立时把**所有**阶段的节点一次性置 `running`）、`src/console.rs`… 前端 `src-ui/src/console-channel.ts:21-24` 与 `TaskBoard.tsx:632`。
- **问题**：`dispatch` 一返回，界面就显示全部阶段的路数在飞（经济模式 4 路），实际只有第 1 阶段在跑；任务书要求的是「在飞几路」。
- **复现**：`aicross dispatch --preset economy …`，立刻看右栏任务板与总指挥 tab 标题。
- **建议**：仅在该阶段真正发起后置 running（或前端只数收到过 `dispatch-progress` 心跳的节点）。

### 11【低】run 状态词与 `leftover` 契约

- **文件:行**：`src/console_api.rs:498-505`（`done|failed` 二值）、`src/console_api.rs:585-598`（`update_run_results` 把 `quota_exhausted` 等一律记成 `failed`）、`src/console_api.rs:88-98`（`leftover` 字段存在但从未填值，`skip_serializing_if` 使它永不出现）、`runs()` 表从不清理。
- **问题**：文档承诺的 `leftover` 永远不出现；「额度用尽」这类非真失败被报成 `failed`；长时间运行的会话里 run 表与摘要文本只增不减。
- **复现**：`GET /v1/runs/<id>` 观察响应里从无 `leftover`；让某路撞额度，看 `run.status`。
- **建议**：`leftover` 要么实现（解析节点 Reply 的遗留清单）要么从文档/契约删掉；节点/run 状态保留原状态词；给 run 表加容量或 TTL。

### 12【低】文档与实现的零碎不一致

- **文件:行**：`docs/CONSOLE-CHANNEL.md` 全文 vs `src/hook_server.rs:140-146`（413）、`src/console_api.rs:176-182`（400）、`src/console_cli.rs:33-36`（参数错误退出码 2）、`src/console_cli.rs:88-93`（`--preset` 别名 shared/economy/blind）。
- **问题**：正文上限 4 MiB、JSON 无效 400、退出码 2（参数错误）与 1（业务错误的具体值）、预设别名都没写；`leftover` 写了但永不产生；带查询串的路径不被支持（`GET /v1/runs/x?y=1` → 404）也没说明；文档把 `aicross` 当通用入口，但只有 Windows 安装包改了 PATH，`dmg/deb/rpm` 包没有命令行入口（macOS 只有 `aicross.app/Contents/MacOS/aicross`）。
- **复现**：按文档写 skill，传 5 MiB 文本或 `--preset 共享`（中文别名）会失败/报错，而文档没提前说。
- **建议**：补状态码表、退出码表、别名表、平台可用性；未实现的字段从文档删除。

### 13【低】`--text-file` 的编码/BOM 与 `--text=` 静默忽略

- **文件:行**：`src/console_cli.rs:82-86`（只拒精确 `--text`，`--text=…` 落到「缺少 --text-file」静默丢弃）、`src/console_cli.rs:86`（`fs::read_to_string`，无 BOM/UTF-16 处理）。
- **问题**：Windows PowerShell 5 的 `Set-Content -Encoding utf8` 写的是带 BOM 的 UTF-8 → BOM（U+FEFF）会原样进 prompt；UTF-16 文件直接报错退出 2。
- **复现**：`Set-Content -Encoding utf8 p.txt`…（或 `-Encoding Unicode`）后 `aicross dispatch --text-file p.txt`。
- **建议**：剥 BOM、对非 UTF-8 给一句明确提示；把 `--text=` 也一并拒掉或明确报错。

### 14【低】通道没有任何在飞上限，也没有 G1 的脏目录提示等价物

- **文件:行**：`src/console_api.rs:404-447`（预检只覆盖「单次派发」的清单）、`src/console_api.rs:275-306`（无并发 run 上限）。
- **问题**：令牌即授权（任务书要求不弹确认屏），但同一用户可以无限次派发、每次都可能拉起真模型进程；界面 G1 的「项目目录有未提交改动；worktree 看不到这些改动」提示（`src-ui/src/graph/dispatch.ts:1086-1089`）在控制台路径上没有对应检查/说明。
- **复现**：写个循环连发 `aicross dispatch`，观察没有任何拒绝。
- **建议**：给「同时在飞 run 数」加上限并在超过时 409；worktree 目标命中脏目录时把提示写进 run/节点或返回 409。

### 15【低】测试里有同义反复，覆盖不到关键分支

- **文件:行**：`src/console_api.rs:800-805`（Windows 分支只断言 `windows_acl_args(...)` 等于它自己拼出来的字面量数组，`whoami` 的 SID 解析、icacls 调用与否全无覆盖）；`src-ui/src/__tests__/console-channel.test.ts:41-46`（断言 `GraphTab.tsx`/`TaskBoard.tsx`/`CenterPanel.tsx` 源码里含某个字符串）与 `:48-54`（断言 `installer.nsi`/`path.wxs`/`tauri.conf.json` 含某字符串）——改文案就红、行为回归不红；`fake_channel_dispatch_status_adopt_round_trip`（`src/console_api.rs:771-784`）只验路由形状，adopt 的真实写图由 `orchestration.rs:1735-1742` 单独覆盖。
- **问题**：任务书要求的 401/404/409/round-trip/清单/退出码都有人写，且能红；但「Windows 私密权限」「NSIS/WiX 幂等与不误删」「HTTP 首行分流/413/Content-Length」这几条只靠读代码。
- **复现**：把 `windows_acl_args` 改成 `["/grant:r", format!("*{sid}:M"), "/inheritance:r"]`（M 而非 F）→ 测试全绿。
- **建议**：ACL 用可注入的 runner 断言调用参数序列，并在 Windows CI 上真跑一次 `icacls` 后读 DACL；安装脚本改成渲染 `{{...}}` 后的产物断言，或写一个解析 PATH 的纯函数来测。

### 16【低】崩溃残留与 dev/release 共用清单

- **文件:行**：`src/console_api.rs:646-656`（删除）、`src/server.rs:4141-4142`（`ExitRequested` 时删）、`src/console_api.rs:614-644`（写）、`src/console_cli.rs:128-143`（读，不检查 `pid`）。
- **问题**：`pid` 字段没有任何消费者；崩溃残留靠「端点不通 → 退出码 3」兜底（这条兜底成立），新启动覆盖旧清单等效清理（成立）；但 debug 构建跳过单实例锁（`src/server.rs:3866` 注释与 `#[cfg(not(debug_assertions))]`），dev 与已装的 release 会互相覆盖 `console.json`，先退出的一方会删掉另一方的清单，直到对方重启。
- **复现**：同时开 `tauri dev` 与已安装版，`aicross status` 一会儿打 dev、一会儿打 release；关掉 dev 后 release 的命令变「控制台未运行」。
- **建议**：读清单时用 `pid` 做一次活性判定（进程不存在就清掉并报 3，措辞改成「控制台未运行（清单已过期）」）；dev 用独立清单文件名。

---

## 我核过、没问题的要点

- **监听与端点**：只 `bind("127.0.0.1:0")`（`hook_server.rs:56`）；客户端只接受 `http://127.0.0.1:<port>`（`console_cli.rs:137-141`），`localhost`、https、远端都被拒。
- **鉴权顺序**：所有路由进入前先查 `Authorization`（`console_api.rs:170-176`），未知路径也先过令牌，不会先 404 再鉴权；`401/404/409/413` 的 HTTP 状态与原因字段一致。
- **正文上限**：`content_length > 4 MiB` 直接 413、不读体（`hook_server.rs:140-152`）；`Content-Length` 解析失败按 `usize::MAX` 处理 → 413。
- **令牌不进日志**：`hook_server.rs:62` 只打印端口；清单内容从不打印；CLI 只把令牌放 `Authorization` 头（`console_cli.rs:148-156`）。
- **清单权限与生命周期**：Unix 先 `chmod 0600` 再写内容（`console_api.rs:628-633`，无「先落盘后收权限」窗口）；写临时文件→sync→替换→rename，失败清临时文件；Windows 在 `temp` 上 `icacls /grant:r *SID:F /inheritance:r` 后 rename（ACL 随 inode 保留）；退出在 `RunEvent::ExitRequested` 删除且 `NotFound` 视为成功（`server.rs:4141`、`console_api.rs:650-655`）。
- **单实例不会误删清单**：`tauri-plugin-single-instance` 在插件 setup（`App::build` 阶段，早于 app 的 `setup`）里对第二进程 `process::exit(0)`，既不写自己的清单也不触发 `ExitRequested`，所以双击/安装后自动启动不会破坏运行中实例的 `console.json`。
- **HTTP 与 hook JSON 行互不干扰**：hook 端口经 `AICROSS_HOOK_PORT` 环境变量注入到工具会话（`terminal.rs:733-767`），HTTP 客户端从清单读同一端口；首行前缀分流（`hook_server.rs:103`）不会把 `{...}` 误判成 HTTP。hook 行无鉴权是既有行为，本次未改变。
- **真走既有入口**：派发 `dispatch::dispatch_stage`（`console_api.rs:487`）、落盘 `orchestration::save_graph`（`278/473/496` → `save_graph_in` → `write_json_atomic` 原子写）、采纳 `orchestration::adopt_graph_node`（`880-899`，HTTP/CLI 与结果卡同一函数，并有落盘断言 `1735-1742`）。
- **409 清单与 G1 口径**：未接入目标、`verification=failed`、未到期的 `quota_exhausted_until`、只读角色（验收/验证/仲裁/拆分）写权限不符、超过请求上限、空文本、预设不存在、项目目录不存在→404；**未验证目标放行**交给引擎「首次派发先冒烟」的既有逻辑，与 `docs/PRODUCT.md` 09-15 冒烟口径一致；且预检在写图之前（`prepare_dispatch_at` → `start_dispatch` 才 `save_graph`），409 不会留下半张图。
- **界面接线**：节点/图 `source=console` 全量打标（`prepare_dispatch_at`），节点卡显示「来自总指挥」、tab 标题 `· 来自总指挥`；总指挥在前台用 `ADD_TERMINAL_BACKGROUND`（`app-state.tsx:429-437` 不动 `activeTerminalId`）不抢焦点，普通 tab 自动打开或切换（`CenterPanel.tsx:502-510`）；心跳复用同一 `run_id/node_id`（`dispatch_stage` 的 `DispatchStageRequest.run_id` 与 run 一致），任务板 `subscribeProgress` + `runningDetailOf`/`idleHint`、5 s 时钟均走既有实现。
- **文案合规**：新用户可见字符串（「来自总指挥」「N 路在飞」「来自总指挥 · N 项任务」「控制台未运行」）是中性说明体、无内部词、无 emoji、标点全角，符合 `AGENTS.md`「界面文案与命名规范」；`docs/CONSOLE-CHANNEL.md` 是维护者协议文档，不受界面文案约束。
- **CLI 语义**：退出码 3 只出现在「拿不到/够不到控制台」这一类（`aicross_root` 失败、清单缺失/校验不过、传输错误），业务 401/404/409 → 1 并回体，参数错误 → 2；`--project/--text-file` 必填、`--text` 被拒、多行正文只走文件（`console_cli.rs:78-100`）；`status --wait` 间隔 500 ms。
- **子命令不干扰 GUI**：只匹配 `argv[1]` 的四个整词，`launch --tool`、深链、文件关联参数都不撞（`main.rs` CLI 分支早于 `server::start_ui`）；`aicross` 子命令在 macOS/Linux 上会白跑一次登录 shell（PATH 修复在该分支之前），只影响启动延迟。
- **NSIS 幂等与不误删**：安装前用 `${StrLoc} ";$R8;" ";$INSTDIR;"` 判断后 `WriteRegExpandStr … "$R8;$INSTDIR"`，重复安装不会重复追加；卸载 `StrRep` 只摘掉 `;$INSTDIR;` 这一段并正确去掉包装分号/尾分号，不会删掉用户 PATH 里的其它项；WiX 组件 `Permanent="no"` + `componentRefs` 已接线并在 `tauri.conf.json` 中登记。
- **能红的测试**：Rust 侧 401/404/409(带原因)/`graph.source` 与节点标记/假通道 round-trip/Unix `0600`/清单写删/无控制台退出码 3 单行/多行只走文件/拒 `--text`/端点为 IPv4 loopback，改坏实现都会红；前端三个纯函数用例（总指挥前台不抢焦点、普通 tab 打开或切换、计数只算 running）是真行为断言。