# TASK-16 控制台通道 审查

**总判：可交付但有未闭合项。** 安全基线（loopback + Bearer + 0600 清单）成立，复用面（dispatch_stage / save_graph / adopt_graph_node）是真的；主要缺口在「Rust 侧第二个写者」带来的丢更新竞态、用户派发设置全部不生效、以及若干 CLI/取消语义边角。

---

## 按严重度排序

**1. 双写者丢更新：`run_prepared` 用自持快照反复全图保存，会抹掉运行中的采纳与前端改动**
- 位置：`src/console_api.rs:464-473`（`let mut graph = prepared.graph;` 之后每阶段 `save_graph(graph.clone())`）、`src/console_api.rs:556-567`（`apply_results` 只保留快照内的 `old_adopted`）、`src/orchestration.rs:885-900`（adopt 是「读盘-改-存盘」）。
- 复现：经济模式通道派发（阶段1执行、阶段2验收）→ 阶段1结果落盘 → 用户在结果卡点「采纳」（或 `aicross adopt`，写盘成功）→ 阶段2 开始时 `run_prepared` 把派发前的快照整图写回 → 采纳标记从磁盘和图上消失。同理，图 tab 里用户的改名/编辑（`GraphTab.tsx:332-333` 的 `SET_GRAPH`+`saveGraph` 全图保存）与阶段保存互相覆盖； CenterPanel 每个通道事件都 `loadGraph`+`SET_GRAPH`（`CenterPanel.tsx:499-501`）会刷新显示，但挡不住写侧互相践踏。
- 建议：每个阶段派发前从磁盘 `load_graph_at` 重读合并（节点按 id 合并、reply 保留磁盘侧 adopted），或把「改 adopted」和「写阶段结果」收敛到同一把锁/同一条读-改-写路径。

**2. 派发设置用 `DispatchSettings::default()`，用户的验证命令、请求上限、超时、并发、cc-switch、修正轮全部不生效**
- 位置：`src/console_api.rs:406`（409 上限检查用默认 8）、`src/console_api.rs:485`（阶段请求用默认 settings，`validation_command`/`cc_switch_path` 为空）。
- 对照：界面派发走 `loadDispatchSettings()`（`DispatchOverlay.tsx:188-198`，存 localStorage）。任务书要求「按 G1 规则自动派发……仍按现有规则处理」，现实现按的是出厂默认而非用户的现有规则；通道派发的验收/验证节点不会执行用户配置的验证命令。
- 根因：设置在 webview localStorage，Rust 够不到。需要把设置落到 `~/.aicross`（或请求体携带），否则 409「请求上限」与验证门都和界面行为分叉。报告未提这一限制。

**3. cancel 对已结束的 run 无条件置 failed**
- 位置：`src/console_api.rs:148`（`run.status = "failed".into()` 无前置判断）。
- 复现：run 已 `done` 后调 `POST /v1/runs/<id>/cancel`（或 `aicross cancel`）→ 状态变 `failed` 而 nodes 全 `done`，GET 返回自相矛盾。
- 建议：仅 `status == "running"` 时生效，否则返回 409「run 已结束」。

**4. `status --wait` 没有超时，HTTP 客户端也没有总超时**
- 位置：`src/console_cli.rs:39-60`（`loop` + 500ms 轮询，无上限）；`src/console_cli.rs:149-159`（ureq 默认无总超时）。
- 复现：run 永远停在 running（引擎卡住、控制台进程僵死但端口还在）→ CLI 无限轮询不退出。审查点明确问「轮询间隔与超时」，报告也只写了「轮询至非 running 状态」。
- 建议：加 `--timeout`（含默认值），到时按非 0 退出；给 ureq 设总超时。

**5. `send()` 把「HTTP 200 但响应非 JSON」也归为 exit 3「控制台未运行」**
- 位置：`src/console_cli.rs:160-163`（`into_string().ok().and_then(from_str).ok_or(SendError::Unavailable)`）。
- 复现：端口被别的进程占用且返回非 JSON 200 → 已连通却报「控制台未运行」退出 3，与「3 只用于控制台未运行」的文档契约有偏差。
- 建议：区分「传输不通」（3）与「响应不可解析」（非 0，另给一句话）。

**6. `review_material_validation` 判定与前端不一致**
- 位置：`src/console_api.rs:552`（`target.role == "验证"`）对照 `src-ui/src/graph/dispatch.ts:1155-1157`（要求 `role === '验证' && visibility === '盲'`）。
- 影响：共享可见性的验证节点也会附带验证输出。当前因第 2 条（validation_command 为空）暂不显形，修好第 2 条后即成实际行为分叉。
- 建议：改为与 `reviewMaterialValidationOf` 同一条件。

**7. hook-server 启动与 console.json 写入耦合，清单失败会拖垮既有功能**
- 位置：`src/hook_server.rs:59-61`（`write_manifest(...).map_err(...)?`）；`src/console_api.rs:665-679`（Windows 启动即跑 whoami+icacls，任一失败即返回 Err）。
- 复现：`~/.aicross` 不可写（或 ACL 收紧失败）→ `hook_server::start` 整个失败（`server.rs:4008-4010` 仅打日志）→ 既有 agent-status 心跳/动态岛全部失效，而那是 TASK-16 之前就有的功能。
- 建议：监听器照常起，清单写失败只降级「控制台通道不可用」。

**8. 令牌比较非常量时间**
- 位置：`src/console_api.rs:170-172`（`authorization != Some(expected.as_str())`，普通字节比较短路）。
- 评估：仅 127.0.0.1 可达 + 令牌 244 位熵，实际可利用性很低；但审查点点名，标准做法是 constant-time 比较（如 `subtle` 或手写逐字节异或累计）。

**9. 无 Origin/Host 校验（纵深防御缺一项，非漏洞）**
- 现状：浏览器跨站 POST 能到达端口，但无令牌必 401（令牌只在 0600 清单里，网页读不到）；`OPTIONS` 首行不匹配 `GET /` `POST ` 分流（`src/hook_server.rs:103`），按坏 JSON 走、回非 HTTP 的 `{}\n`，CORS 预检必败，实际请求发不出去。DNS rebinding 同样被令牌挡住。
- 建议（低成本加固）：`route` 前若请求带 `Origin` 头直接 403——本机 CLI 客户端（ureq、Invoke-RestMethod）不发 Origin，浏览器一定发。

**10. Windows 清单 ACL 时序：先写正文后收紧**
- 位置：`src/console_api.rs:627-640`（Unix 在写入前 `harden`，Windows 在 `drop(file)` 后才 `harden`）。
- 影响：令牌短暂存在于继承 ACL 下。用户 profile 默认 ACL 只含 user/SYSTEM/Administrators，跨用户泄露风险很小，但与 Unix 分支的严格顺序不对称。
- 建议：Windows 同样在写入前对空临时文件先收紧。

**11. 额度用尽在通道里并进 `failed`，丢失重置时间**
- 位置：`src/console_api.rs:590`（`quota_exhausted` → 节点 `failed`）、`src/console_api.rs:504`（任一节点非 done → run `failed`）。
- 对照：TASK-14 定案「额度用尽不算失败、把重置时间写进节点」；图侧 `Reply` 保留了 `quota_exhausted` 与 `quota_resets_at`（`apply_results` 正确写入），但 run 节点摘要只有三态、无重置时间字段，skill 拿不到「何时恢复」。
- 建议：`ConsoleRunNode` 加可选 `quota_resets_at`/细化 status，或至少把重置时间并进 `text_excerpt`。

**12. NSIS 卸载无条件重写 HKCU Path（两处小瑕疵，无误删）**
- 位置：`installer.nsi:777-788`。
- 事实：删除逻辑按 `;完整项;` 精确匹配、只删 `$INSTDIR` 本项，不会动用户其他 PATH 项 ✓；安装侧 `${StrLoc}` 守卫幂等 ✓（`installer.nsi:679-690`）。但：① `ReadRegExpandStr` 失败（值不存在）时 `$R8` 为空，卸载仍会写入一个空 REG_EXPAND_SZ `Path` 值；② 用户 Path 若原本是 REG_SZ，`WriteRegExpandStr` 会把它改成 REG_EXPAND_SZ（内容不变，类型变更）。③ 卸载时即使用户自己把该目录加进了 PATH 也会被移除（可argue是正确清理）。
- 建议：`ReadRegExpandStr` 后先判 `${IfErrors}` 跳过重写；或记录安装时的值类型。WiX 侧 `windows/fragments/path.wxs` 是标准 user-env 写法，但 `Guid="*"` 的 user 环境组件无 HKCU keypath 会触发 ICE38 警告，且报告承认未实际出包验证。

**13. 界面接线测试一半是同义反复**
- 位置：`src-ui/src/__tests__/console-channel.test.ts:41-56`。对 `GraphTab.tsx`/`TaskBoard.tsx`/`CenterPanel.tsx`/`installer.nsi`/`path.wxs`/`tauri.conf.json` 的源码做 `toContain` 字符串断言——只证明字符串还在：改坏逻辑不红，改个文案就红。`WM_SETTINGCHANGE` 计数断言尤其脆。
- 对照：同文件前三个用例（`consoleTabPlan` 不抢焦点、自动开/切图、在飞计数）是真能红的行为测试 ✓；Rust 侧 12 个测试全是行为级（401 顺序、404、409 原因、source 标记、假通道往返、清单权限、CLI 退出码/参数）✓。
- 建议：把源码字符串断言收敛为对纯函数/解析结果的断言；NSIS/WiX 想留绊线至少注明其性质。

**14. 文档与实现的少量缺口（已核对部分全部一致）**
- `docs/CONSOLE-CHANNEL.md` 的路径、字段、状态码、示例与结构体逐字段核对一致（含 `leftover` 仅在有值时出现、cancel 正文 `{}`、401/404/409 语义、loopback 校验要求）。
- 未写：413/正文上限、CLI 参数错误 exit 2、「续哪张图」的规则（`src/console_api.rs:450-455`：路径归一化后匹配、`updated_at` 最新）、quota→failed 映射、`--preset` 非三个短名时原值直传。均不算错误，属 README 级缺漏。

---

## 核过没问题的要点

- **监听与令牌**：只绑 `127.0.0.1:0`（`src/hook_server.rs:56`）；所有路由先验 Bearer（`src/console_api.rs:170-173`），`wrong_token_is_401_before_routing` 钉住校验先于路由。令牌不进日志（日志只有端口、hook 状态行、坏 JSON 行，不含 Authorization 头；CLI 不打印令牌）。清单：Unix 0600 在写入前设置（测试断言 mode）、Windows 失败即拒启、`create_new`+rename 原子发布、失败清理临时文件；`ExitRequested` 删清单（`src/server.rs:4141-4142`）、崩溃残留由下次启动覆盖、CLI 对死端点 exit 3；`tauri-plugin-single-instance` 保证不会被第二实例覆盖清单。正文上限 4 MiB→413、Content-Length 解析失败按过大处理。
- **协议分流**：hook JSON 行（`{` 开头）与 HTTP（`GET `/`POST ` 开头）首行分流互不干扰，JSON 行的 `{}\n` 应答路径未动。
- **复用**：派发走 `dispatch_stage`（`src/dispatch.rs:1484`，`dispatch_graph_stage` 同走它）；图写走 `save_graph`；HTTP adopt 与结果卡共用 `adopt_graph_node`（`orchestration.rs:880`，有持久化测试）；readonly 名单与引擎（`dispatch.rs:1720`）、前端（`validate.ts:6`）同一份（验收|验证|仲裁|拆分），盲→只读同样校验（前端只是提示、通道 409 更严，方向正确）。
- **409 与 G1**：额度用尽（判断有效期末到）、未接入、验证失败、只读、角色无目标、预设不存在、超请求上限、空文本均有原因返回；「未验证」不放 409、交引擎首次派发冒烟（`src/resources.rs:1718` 起），与 PRODUCT 定案一致。通道建的阶段里 `accept` 节点天然满足扇入。
- **图与标记**：续图取项目归一化路径匹配、`updated_at` 最新；根空节点复用、否则挂 `latest_leaf`；图/根/请求/派发节点全部 `source=console`（测试钉住）；`project`/`source` 均为 serde default + skip 序列化，旧图兼容。
- **前端采纳回归**：`pickOne`（擂台选定）与蜂群合并的 adopted 写入未动（`dispatch.ts:444-445`、`swarm.ts:603` 仍走纯前端 `setReplyAdopted`，既有测试未改）；只有普通结果卡采纳/取消采纳改调共享入口，失败仅 console.warn（轻微 UX，不算回归）。
- **焦点语义**：`consoleTabPlan` 总指挥前台→background（不新增可见 tab、不切焦点），普通前台→focus/open；「N 路在飞」只加在 `commanderTitle` 的 tab 上；任务板复用 `dispatch-progress` 心跳 + 既有 idle/stuck 判定函数。
- **CLI 与启动**：仅 argv[1] 精确等于四个子命令才短路 GUI；无参/`launch --tool`/未知词都照常起 GUI；single-instance 回调在 `start_ui` 内，CLI 短路不触碰；`--text` 显式拒绝、多行只从 `--text-file` 进 JSON 体（测试钉住）；exit 3 仅「清单缺失/无效/端点不通」，stdout 空、stderr 一行（测试钉住）；清单校验 version=1、64 hex、强制 `http://127.0.0.1:<port>`。
- **依赖**：未新增 crate（`ureq = "2"`、`windows` 的 `Win32_System_Console` feature 均已在 `Cargo.toml`）。