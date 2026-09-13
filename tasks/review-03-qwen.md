Rust 侧读完，接着看前端图逻辑与派发组件。



补几个行号定位。



# TASK-03 代码审查

## 1. 需求完整性

- **src/channels/mod.rs:497** — kimi session_id 取不到。发送侧已改用 `-S`（mod.rs:124，REPORT 也声明以 0.42.0 `--help` 为准），但解析侧正则仍是 `(?:--resume|-r)\s+...`，且任务书要求从输出末尾 `To resume this session` 行取。CLI 提示行若打的是 `-S <id>`/`--session <id>`，永远匹配不上 → kimi 续聊链路断。建议：正则改为同时匹配 `-S|--session|--resume|-r` 并锚定提示行。
- **src/channels/mod.rs:481-511** — kimi「流里若有 usage 就记」未实现，`parse_kimi` 返回的 `Usage` 恒为 0，任务书 A 表明确要求。建议：扫 stream-json 里的 usage 字段。
- **src/channels/mod.rs:92** — codex 盲审额外加 `--ephemeral`。任务书没有这个 flag，REPORT 也没写实测依据（B 节要求"测出来写进 REPORT"，REPORT 不确定处 2 承认没测）。若是无效 flag，codex 全通道直接挂。建议：删掉或补 `codex exec --help` 证据。
- **src/channels/mod.rs:229-235** — codex thinking 映射 `高→xhigh`，导致 CLI 的 `high` 档永不可达；任务书表列的是 low/medium/high/xhigh 四档对 UI 四档（关/低/中/高）。口径需和编排者确认，至少 `高→high`、留一档给 xhigh 或注明取舍。
- **src/channels/mod.rs:529-540** — cc-switch「它打印的 usage」未解析（恒 0），「不支持续聊，标"单轮"」的标注也没有落点。
- **B 节宿主级隔离实测** — 任务书要求 codex/kimi/pi 的等价开关"本期一并测出来写进 REPORT"；REPORT 不确定处 2 承认只静态标 `部分`，未测。属未完成项（虽按契约标了部分，不算造假）。
- **E 节** — ① 用量角标（in/out）没渲染：GraphTab.tsx:400-430 的 `renderTurn` 只显示正文；② 「新开会话」入口不存在。按第 0 节口径续聊本体已实现（continuationSession + 只发本条），这两小项仍缺。
- **G2 形态** — 任务书写"中栏系统行"，实现是模态 overlay（DispatchOverlay.tsx:248-262）。可辩护的偏差，应记录。
- 以下核对为符合：派发单位=draft 节点（dispatch.ts:16-27）；prompt 来自 contextOf/serializeContext（context.ts:139-176，含盲审冻结前缀）；结果写回 `node.reply` 并经 commitGraph 持久化（dispatch.ts:56-73、persist.ts:10-15，pending 先落盘）；claude 盲审 `--restricted`（mod.rs:66-68）；codex 父级选项在 `resume` 前（mod.rs:95-100）；kimi >30000 UTF-8 字节拒绝（mod.rs:110-116，`prompt.len()` 即字节数，正确）；pi 真身不符判失败且映射到 `identity_mismatch`（mod.rs:427-433、dispatch.rs:437-439）；frontmatter 29 个字段与 C 节逐字一致（dispatch.rs:159-190）；STATE.md 写（dispatch.rs:711-751）；`.dispatch/.gitignore` 内容 `*`（dispatch.rs:679-687）。

## 2. 逻辑正确性

- **src/channels/mod.rs:464-467** — pi 的 cache_read 找 `cache_read`/`cache_read_tokens`，但 pi 真实事件的键是 `cacheRead`（铁证：`.dispatch/20260913-193721-pi-glm-4-7-数据审查.md` 原始输出里 `"usage":{"input":0,"output":0,"cacheRead":0,...}`）→ pi 的 cache_read 恒 0。加 `usage.get("cacheRead")` 分支。
- **src/channels/mod.rs:316-319** — `value.get("api_error_status").is_some()` 对显式 `null` 也成立；claude 正常应答若带 `"api_error_status": null` 会被误判失败。改为 `and_then(Value::as_u64).is_some()` 或非 null 判断。
- **src/dispatch.rs:285-289** — 同 agent 串行用原始 `node.agent` 字符串做键，`normalize_agent`（kimicode/kimi-code→kimi）只在 channels 内部生效；混用别名时"同一 agent 最多 1 路"会被绕过。调度前先归一化。
- **src/dispatch.rs:397-410** — `prepare_outbound` 出错（如附件缺失）时 `outbound` 回落为**未脱敏**的 `node.prompt`，虽然不外发，但仍写进 `.dispatch` 的「任务全文」，脱敏内容落盘。错误路径下应写占位文本或脱敏后再写。
- **src/channels/mod.rs:497-505** — kimi session 正则在 stdout+stderr 全文取最后一个 `-r <8+位字母数字>`，模型正文里出现类似片段就会误捕获为 session_id。
- 核对为符合：并发 `clamp(1,3).min(n)`（dispatch.rs:269-274）；超时后 `taskkill /PID /T /F` 杀树（process.rs:222-238）；4MiB 截断+探测多读 1 字节置 `truncated`（process.rs:186-199）；stdin 送 prompt、kimi 走 argv、cc-switch 走 task-file；成功判定看输出体不看退出码；claude `session_id`、codex `thread.started.thread_id`、pi `session` 事件 id + 固定 uuid 回填（dispatch.rs:441-443）均正确。

## 3. 边界与安全

- **src/dispatch.rs:609** — `(password|passwd|pwd)\s*[:=]\s*[^\s,;]+`：值含逗号时只脱前半，`password=abc,def` 会留下 `,def`。token/api_key 同理（dispatch.rs:610）。建议值字符集收掉尾部标点或在 G1 提示残留风险。
- 六类正则各自可命中、`[REDACTED]` 替换整段（含键名）后不再命中，有测试断言 ✓。误杀面（正文里的 `token: ...`、示例代码）属设计内，未见异常。
- **环境变量值** — frontmatter 无 env 字段 ✓；但 `## 原始输出`/`## stderr` 是 CLI 输出全量落盘，无兜底过滤，CLI 自己回显的凭据/路径会进 `.dispatch`（19:37 kimi 记录已含 `C:\Users\keros68\...`）。任务书说"绝不写入任何环境变量值"，严格讲应对 error/stderr 再过一遍脱敏。
- 不读 `auth.json`/`cc-switch` db：grep 确认新代码未触碰 ✓。
- 盲目录只含勾选附件 ✓（dispatch.rs:566-604，含重名拒绝、空目录校验）；测试断言了 CLAUDE.md、.dispatch 未拷入，但没断言 AGENTS.md（任务书点了三个，测试少一个）。
- CREATE_NO_WINDOW：process.rs:81（子进程）、process.rs:233（taskkill）、channels/mod.rs:283（where.exe）全覆盖，无遗漏 spawn 点 ✓。
- **src/channels/mod.rs:300-307** — `D:\work\agentteam\dist\ai-cross\references` 硬编码进源码作 cc_switch.py 兜底，机器特定路径，换机即失效且泄露开发机布局。改为可配置或随应用资源打包。

## 4. 代码质量

- **DispatchOverlay.tsx:26-27** — `isolationOf` 与 Rust `channels::isolation_for` 逻辑重复（preview 已返回 isolation，前端兜底可留但应注明镜像关系）。
- **src/dispatch.rs:861-910** — `Classification`/`parse_classification` 是 `#[cfg(test)]` 死代码放生产文件；功能没接就别留半成品，移到测试模块或删。
- **OrchestrationPanel.tsx:2** — 注释「本期不做派发执行」已过时（本期就做了），误导后来者。
- **DispatchOverlay.tsx:288** — 用量表「结果」列直接渲染内部英文状态 `ok/error/timeout/identity_mismatch`，同屏卡片头却用「完成/失败/超时」；违反 AGENTS.md 文案规范（不用内部词、状态词成对）。「阶段」列硬编码 `1`，而请求里 stage 可变。
- **DispatchOverlay.tsx:255-259** — G2 行只显示 `agent/model`，同 agent 多节点（重派+新节点）无法区分；建议带 role 或节点摘要。
- 第 2 期行为未破坏：serializeContext 复用 contextPartsOf、旧测试全过（58 passed）、图提交仍走 commitGraph ✓。G1/G3 按钮文案（确认派发/取消/采纳/重派/升级/交人裁决）符合 2-6 字动词短语规范 ✓。

## 5. 测试

验收第 2 条九组测试**都存在**（frontmatter 往返、六类脱敏+不再命中、truncated、--restricted、盲目录、kimi 30000、codex resume 顺序、pi 真身、归类容错）。问题：

- **src/channels/mod.rs:574-584、594-627、639-670、672-694** — 四个 argv 测试开头 `if resolve_binary(...).is_err() { return; }`：机器上没装对应 CLI 时**静默空转通过**，验收断言形同虚设（CI 上大概率全跳过还显示绿）。应把二进制解析从 `build_invocation` 里拆出去（返回 unresolved argv），测试直接断言 args，不依赖真机环境。
- **src/dispatch.rs:1015+** — live 测试的 prompt 是手写 concat 文本，没走生产的 `serializeContext`/`BLIND_PROMPT_PREFIX` 路径，真跑验证的不是产品真实外发形态。
- live 测试用 `std::env::set_var` 改进程级环境（dispatch.rs:1017、1131），与其他读 `AICROSS_TEST_RUN_ROOT` 的测试并行有竞态（当前靠 `#[ignore]` 掩盖）。
- 最该补的 3 个：① **kimi session_id 解析**——用 0.42.0 真实末尾提示行做 fixture（现在正则和 `-S` 发送侧都对不上，竟无测试暴露）；② **pi usage/cacheRead 解析**——用 `.dispatch` 里已有的真实事件 JSON 做 fixture，立刻能抓到 `cacheRead` 键名 bug；③ **无环境依赖的 argv 断言**——claude `--restricted`、codex resume 顺序改成注入式 resolver，消除静默跳过。

## 6. 真跑证据

**REPORT-03 的真跑叙述与仓库现状冲突，证据链不闭合：**

- 19:13 / 19:15 / 19:18 / 19:19 各轮记录与 REPORT 叙述（两轮诊断 + 最终轮 + 续接首轮）吻合，失败原因（EPERM 凭据锁、codex os error 5、kimi storage write）前后一致、含 ANSI 色码和本机路径，**不像伪造**；报告引用的 frontmatter 与磁盘文件一致。
- **矛盾 1**：磁盘上有 REPORT 只字未提的 **19:37:05 轮**（run `task03-blind-20260913-193705`），其中 pi 一路**真实成功**：`status: ok`、tokens 457/675、identity `zai-coding-cn/glm-4.7` 匹配、真实 session_id、676+ 行真实流式事件（`.dispatch/20260913-193721-pi-glm-4-7-数据审查.md`）。这直接推翻 REPORT「所有记录均为零 tokens、空 identity/session，未观察到成功的模型响应」和「没有继续重试」两句。
- **矛盾 2**：REPORT 引用 STATE.md 为 19:19:48 续接轮内容；实际 STATE.md 是 19:37:21 盲审轮，codex 卡在「运行中」且**没有对应 record 文件** → 该轮被中途掐死（可能遗留孤儿 codex 进程），REPORT 证据块与仓库现状脱节。
- **矛盾 3**：REPORT 引 panic 位置 `dispatch.rs:1150`，当前同一断言在 1158 行 → 真跑之后代码又改过，报告证据与当前代码不同版本。
- 19:37 kimi 失败原因是 `kimi-for-coding-highspeed is not configured in config.toml`：验收指定的模型别名本机根本没配，违反 AGENTS.md「模型 ID 读本地事实源」的流程——派发前没做冒烟核对。
- 结论：无故意造假迹象，但报告要么写在 19:37 重跑之前没更新，要么隐瞒了一轮部分成功的重跑；验收第 3 条（三路成功+用量有数）、第 4 条（续聊两轮）**至今未达成**，且已有一路 pi 成功证据未被报告承认。

---

## 最终结论

**一句话**：派发引擎主体（图对接、G1/G2/G3、`.dispatch/` 契约、脱敏、进程卫生、并发/超时/取消）按第 0 节口径基本落地且质量不低，但 kimi 续聊解析、pi cache_read、codex `--ephemeral` 三处通道级缺陷未经验证，验收 3/4 真跑未达成，且 REPORT-03 的真跑叙述与 `.dispatch/` 磁盘现状（19:37 有一路 pi 真实成功、STATE.md 卡死）直接矛盾，需实现方解释并补跑。

**缺项列表**：
1. kimi session_id 解析与 `-S` 发送侧不匹配（续聊断链）
2. kimi usage「流里有就记」未实现
3. pi `cacheRead` 键名不匹配，cache_read 恒 0
4. codex `--ephemeral` 无出处、未实测
5. codex thinking 映射使 `high` 不可达（口径待确认）
6. cc-switch usage 未解析、「单轮」标注缺失
7. codex/kimi/pi 宿主级隔离实测未做（B 节要求本期写进 REPORT）
8. E 节用量角标、「新开会话」入口缺失
9. 四个 argv 验收测试在无 CLI 环境静默跳过
10. 验收 3（三路盲审成功+用量有数）、验收 4（续聊两轮）未达成
11. REPORT 与 `.dispatch/` 现状矛盾（未报告的 19:37 成功轮、STATE.md 卡在「运行中」、panic 行号漂移）
12. `D:\work\agentteam` 硬编码路径；用量表英文状态词、「阶段」硬编码 1