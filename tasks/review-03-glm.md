# TASK-03 代码审查

## 1. 需求完整性

派发单位=draft 节点（`src-ui/src/graph/dispatch.ts:16`）、prompt=contextOf 序列化（`context.ts:141`）、结果写回 `Reply` 并经 `commitGraph` 持久化（`DispatchOverlay.tsx:133-141`、`persist.ts:10`）、claude 盲节点 `--restricted`（`channels/mod.rs:67-69`）、codex 父级选项在 `resume` 前（`channels/mod.rs:80-98`）、kimi 按 UTF-8 字节 >30000 拒绝（`channels/mod.rs:106`，`str::len` 即字节数，正确）、pi 固定 uuid（`dispatch.rs:391-395`）、frontmatter 28 个字段与 C 节逐字同序（`dispatch.rs:158-189`）、`.gitignore` 写 `*`（`dispatch.rs:662-670`）、STATE.md 每次变更重写（`dispatch.rs:265,293,331`）——这些均落实。发现：

- **E节「新开会话」无入口** — `dispatch.ts:29` 的 `continuationSession` 只读不清，全 UI 无任何清除 `session_id` 的操作。为什么：任务 E 明确要求用户可重开会话；现在一旦某目标拿到 session_id 只能永远续聊。建议：G1 或结果卡加「新开会话」，置空父链 session_id。
- **E节用量角标未展示** — `GraphTab.tsx:417-425` 回复气泡只渲染 `nodeLabel`，`tokens_in/out` 存进 Reply 后从未显示。建议：meta 行加 `in/out` 角标。
- **kimi 用量从不记录** — `channels/mod.rs:481-509` `parse_kimi` 返回 `ChannelOutput::default()`，不解析 usage。任务表要求「流里若有 usage 就记」。建议：`collect_text_blocks` 同路遍历取 usage 字段。
- **cc-switch 用量丢失** — usage 打在 stderr（`cc_switch.py:13,207`），`parse_cc_switch`（`channels/mod.rs:533-542`）只看 stdout。建议：从 stderr 解析 usage 行。
- **共享节点附件只列文件名不给路径** — `context.ts:163-167` 把附件 basename 拼进 prompt。B节要求「附件路径列表」；共享模式 CLI 在项目目录里无法定位非 cwd 下的附件。建议：共享节点序列化用全路径，盲节点保留 basename。
- **G2 形态偏差** — 任务说「中栏系统行」，实现为 overlay 内行（`DispatchOverlay.tsx:253-261`）。功能等价，记录在案即可。
- **live 验收测试不断言成功** — `dispatch.rs:1089,1123` 只断言 `dispatch_file` 非空，四路全错也通过（实际就是这么通过的）。验收第 3 条的自动断言形同虚设。建议：非沙箱环境（能写用户目录时）断言 `status=="ok"` 且 tokens>0。

## 2. 逻辑正确性

并发上限 `min(3, n)` 且同 agent 串行（`dispatch.rs:270-338`）、超时杀树（`process.rs:116-131`）、截断标记（`process.rs:198-218`，读满 4MB 后探针再判截断并 drain 防止子进程卡死，正确）、claude `is_error`/`api_error_status` 检查（`channels/mod.rs:317-325`）、判成功看输出体——均正确。发现：

- **pi 真身比对失效（有实证）** — `channels/mod.rs:426-435` 只比对 `message.provider/model`，而供应商会在消息级字段回显请求别名，真实应答模型在 `responseModel`。磁盘证据 `.dispatch/20260913-194041-pi-glm-4-7-续接验证.md:87`：请求 `glm-4.7`，`responseModel: glm-5.3-flash`，被判 `ok`，identity 写成 `zai-coding-cn/glm-4.7`。这正是契约里 `identity` 要抓的「真身」。建议：优先取响应级 model 字段（`responseModel`/`turn_end` 上的），不符判 `identity_mismatch`。
- **kimi session_id 提取过松** — `channels/mod.rs:496-503` 在 stdout+stderr 全文里取最后一个 `-r/--resume <id>`。任务要求取末尾 `To resume this session` 行；模型正文若讨论 CLI 参数（本项目场景极常见）会被当成 session id，下轮续聊打到假会话。建议：锚定该提示行。
- **错误路径把未脱敏 prompt 写进留痕** — `dispatch.rs:399-400` prepare 失败时 `outbound = node.prompt` 原文，随后仍写入 task 文件（:411-412）和 `.dispatch/` 正文（:516-524）。脱敏在 prepare 内先执行，但盲目录/附件错误会把它整个作废。建议：失败时正文写「(未发送)」而非原文。
- **STATE.md 决策记录撒谎** — `dispatch.rs:737` 硬编码「并发上限 3」，与 `settings.concurrency` 无关；且失败/超时节点列在「进行中」（:733-736）。建议：写入实际 limit；失败/超时单列。
- **非 Windows 取消是假动作** — `process.rs:226-241` `terminate_process_id` 非 Windows 分支为空，`cancel_registered` 仍返回 true。建议：非 Windows 用 `libc::kill` 或如实返回 false。

## 3. 边界与安全

- **脱敏漏 JSON 引号形态** — `dispatch.rs:610-611` 要求键后紧跟 `[:=]`，`"password": "x"`、`"api_key":"sk-…"` 不命中，而这是材料里最常见的形态。测试只覆盖 `k=v` 裸形态（`dispatch.rs:918-933`）。建议：键允许可选引号，值允许可选引号前缀。附带：`[^\s,;]+` 对含空格/引号的值只吞第一段（`password: "a b"` 残留 `b"`）。
- **kimi 经 .cmd shim 会炸** — `process.rs:53-56` 对 `.cmd/.bat` 包 `cmd.exe /d /s /c`，此时 kimi 的 30KB argv 受 cmd 8191 字符上限约束，且 `%VAR%` 展开、引号/换行破坏。本机 `~/.kimi-code/bin/kimi.exe` 直跑没问题（19:37 记录证明多行 prompt 完整到达），npm 安装的机器会静默出错。建议：kimi 命中 cmd 包装时直接报「请用原生 exe」。
- **codex 盲跑不盲（实证）** — `.dispatch/20260913-194022` codex 盲审 `tokens_in=389489`（cache_read 354304）审一份 321 字方案。宿主级上下文大量进入。`isolation: 部分` 是诚实的，但 G3 结果卡对盲节点应显著提示（按 input tokens 阈值告警）。
- **六类正则本体、脱敏后不再命中、计数**：✓（`dispatch.rs:606-631`，测试覆盖往返）。
- **环境变量值不落盘/不进日志** ✓；**不读 auth.json、cc-switch.db** ✓（全仓 grep，新增代码无接触）；**盲目录只拷勾选附件** ✓ 且有断言（`dispatch.rs:936-958`）。
- **CREATE_NO_WINDOW 每个 spawn 点都有** ✓：`process.rs:78-82`、`:229-237`（taskkill）、`channels/mod.rs:280-284`（where.exe）。
- `codex --ephemeral`（`channels/mod.rs:91-93`）是任务书与 REPORT 都没提的自加 flag；19:37 真跑证明 flag 存在且可用，但 REPORT 应补记这条隔离尝试及其效果（毕竟 isolation 仍标部分）。

## 4. 代码质量

- `DispatchOverlay.tsx:26-27` `isolationOf` 前端复制 `channels::isolation_for`；preview 缺失时两处可能漂移。建议：preview 必达或在 tauri.ts 暴露同一函数。
- `dispatch.rs:606-618` 每次调用重编译 6 个正则，每节点×每附件都跑一遍。建议 `OnceLock<Vec<Regex>>`。
- `DispatchOverlay.tsx:291` JSX 文本里的 `\n\n` 是字面反斜杠 n，不是换行。
- `DispatchOverlay.tsx:52` 每次 render 同步读 localStorage 并 JSON.parse。
- `DispatchOverlay.tsx:182` running 阶段点背景即 `onClose`，G2/G3 视图丢失（后台 run 与结果 commit 不受影响，但用户回不去）。建议 running 时禁用背景关闭。
- `DispatchOverlay.tsx:51,280` 「采纳」标记只活在组件 state，关 overlay 即丢。建议写进 reply 或图。
- `dispatch.ts:106-112` 「交人裁决」把系统消息写成 user 文本 `[系统]\n…`，后续序列化成 `[用户]\n[系统]\n…` 双重角色标记。
- `context.ts:60-66` failed 回复的 error 文本会作为「模型回复」进入后续共享上下文。建议 `replyMessage` 排除 `failed`。
- 第 2 期图/面板行为（分叉、盖预设、@ 弹层、三列、持久化）未见破坏；文案符合 AGENTS.md（2-6 字动词、无 emoji、无内部词）。

## 5. 测试

验收第 2 条所列 9 组测试全部存在且基本真实断言：frontmatter 往返（`dispatch.rs:879`）、六类脱敏含不再命中（:918）、truncated 置位（`process.rs:281`）、`--restricted`（`channels/mod.rs:575`）、盲目录只含勾选附件（`dispatch.rs:936`）、kimi 30000（`channels/mod.rs:587`）、codex resume 顺序（:595）、pi 真身不符（:630）、归类容错（`dispatch.rs:961`）；前端补了序列化与派发 helpers（`context.test.ts`、`dispatch.test.ts`）。注意 `channels` 里 argv 类测试有 `resolve_binary` 失败即跳过的护栏，CI 无 CLI 时这些断言静默蒸发。

最该补的 3 个：
1. **pi responseModel 真身不符**：直接用 `.dispatch/20260913-194041` 的真实事件行做 fixture，断言判 `identity_mismatch`。
2. **kimi session 提取**：模型正文含 `-r abc12345678` 时不得误取；锚定 `To resume this session` 行。
3. **JSON 引号形态脱敏 + 失败路径留痕不含未脱敏 prompt**。

## 6. 真跑证据

记录本身可信，无伪造迹象：错误发生点与报错文案自洽、零用量与空 identity 一致、stderr 含原始 ANSI 码、pi 事件流是真实格式、替补记录 `exit_code: 0` + `status: error`（`.dispatch/20260913-194024:31-32`）恰好佐证「退出码不可信」按输出体判定。

但 **REPORT 已落后于磁盘事实**：`REPORT-03.md:172` 写「没有继续重试」「共享续接未验证」，而 `.dispatch/` 里有 19:37–19:41 的后续补跑：
- 盲审 `task03-blind-20260913-193705`：pi ok（457/675 tokens）、codex ok（197s，389k in / 5.9k out）、**kimi 失败，真因是 `Model "kimi-for-coding-highspeed" is not configured in config.toml`**（`20260913-193707:32`）——REPORT 只归因到沙箱 storage write（`REPORT-03.md:147`）；这台机器上第三路补跑也过不了，需换 alias 或补配置。替补 pi/deepseek 也失败（text 块为空）。
- 续聊 `194038/194041`：两轮均 ok、session_id 相同（d226e89c）、第二轮原文答出 `青松-731`（`:86`）——**验收 4 实际已有通过证据，REPORT 未更新**。

即：三路盲审 2/3 通过、续聊通过， REPORT 的「未验证」结论需要按磁盘证据改写，kimi 缺口需要单独处置。

---

## 结论

**主体框架按 TASK-03 第 0 节正确落地，但对照原始需求不能算完整实现**：链路、契约、闸门、留痕都在且有真跑留证，缺项如下——

1. 验收 3 未闭环：kimi 路因本机 alias 未配置无法通过，替补也失败；REPORT 未反映 19:37 后补跑结果。
2. 验收 4 实际已跑通但 REPORT 状态仍是「未验证」，需改写并附证据。
3. E节「新开会话」无入口。
4. E节用量角标未展示。
5. kimi/cc-switch 用量不记录。
6. pi 真身校验对 `responseModel` 失效（实证：请求 glm-4.7、实答 glm-5.3-flash 被判 ok）。
7. 共享节点附件只给文件名不给路径。
8. 脱敏正则漏 JSON 引号形态；失败路径留痕可含未脱敏 prompt。
9. D节机器归类未接（可选，已声明）。
10. live 验收测试不断言 `status==ok`，自动验收无效。