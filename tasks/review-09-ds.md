## 1. 需求完整性

**1.1 原生判定只按 agent，claude 换端点这一洞没有兜底（需求级）** — `src/resources.rs:45`、`:244`；`src-ui/src/resources/types.ts:80`、`:89`；`src-ui/src/components/resources/ResourcesOverlay.tsx:322`、`:408`
问题：`is_native_agent` 只看 agent ∈ {claude,codex,kimi}，与端点和 provider 都无关。于是 claude 挂 `ANTHROPIC_BASE_URL`（或 `~/.claude/settings.json` 的 env）指向中继端点时：① 不自动验证——正是本期要防的静默降级场景（PRODUCT 自己举的 glm-4.7→glm-5.3-flash 就是中继端点行为）；② **UI 里也没有任何手动验证入口**：单条按钮的条件是 `verification !== 'native'`，整卡按钮的条件是 `hasThirdParty`（= 该 agent 下存在非 native 条目），claude 全部条目都是 native，两个条件都为假。`resources.rs:1178-1181` 注释和 REPORT-09「不确定处」都写「由接入页的手动『验证』兜底」——这条兜底在实现里不存在。
为什么：这是本期唯一的功能性防线，按 agent 判等于在中继端点上自动豁免。
建议改法：`is_native` 同时要求「没有第三方端点」，直接复用已有的 `claude_third_party_endpoint()`（`resources.rs:1183`），即 `agent=="claude" && claude_third_party_endpoint() → 非 native（unverified，首次派发自动验，走 run_smoke 里已有的真身比对分支）`。同时给原生模型保留一个手动「验证」入口（或至少当 `claude_third_party_endpoint()` 为真时显示）。若这期不做，必须把 `resources.rs` 的注释、REPORT-09 的「不确定处」和 MANUAL 改掉，不能留「有兜底」的假话。

**1.2 回答「原生是否应要求 provider 为空」：不该按 provider 判**
claude/codex/kimi 的条目在库里 provider 恒为空串（官方端点和中继端点都是空），所以「provider 为空」这个条件对两种端点都成立，判不出问题里关心的区别。能判的只有端点（`ANTHROPIC_BASE_URL` / `claude settings.json`），也就是 1.1 的改法。codex 的第三方端点同理要看 `OPENAI_BASE_URL` / `~/.codex/config.toml` 的 `model_provider`，kimi 看 `config.toml` 的 base_url；本期至少把 claude 补上并在手册里写明 codex/kimi 暂不覆盖。

**1.3 契约文字与实现不一致（低）** — 任务书「原生直接返回 native」，实现是 `FirstUseVerification::Skipped`（`resources.rs:1403-1412`）。语义等价（`dispatch.rs:1905` 对 Skipped 与「不需要验证」同样继续派发），但 `Skipped` 把四种情况混成一档：原生、已验证、验证失败、**资源库里没有这一条**。留痕里因此只有 `runner_note` 的一句「首次派发已验证真身」或什么都没有，事后无法区分「原生跳过」与「库里没这一条所以静默跳过」。建议枚举带上原因（`Native` / `AlreadyVerified` / `NotInStore`），至少把 NotInStore 记进 `runner_note`。

**1.4 旧库三态推断：做了，两侧都做了** — `resources.rs:244-266`（`ensure_verification` 只补空字段，`load()` 与 `ensure_agents()` 两个入口都调）、`types.ts:89-99`（前端兜一次）。旧字段 `status`/`smoked_at`/`smoke_model`/`smoke_failure` 全在。✔ 唯一不一致：Rust `NATIVE_AGENTS.contains(agent.trim())` 大小写敏感，TS `toLowerCase()`；手工 agent id 写成 `Codex` 时前端显示 native（无点、无按钮），引擎却按第三方自动冒烟。建议 Rust 侧改成 `eq_ignore_ascii_case`。

**1.5 手册多了一句不存在的行为** — `docs/MANUAL.md:31-32`「条目超过一个月没用过，派发前重验一次」。代码里没有任何基于 `smoked_at` 的过期重验（`verify_on_first_use_at` 只认 `unverified`）。要么删、要么明确写成「建议用户手动」。

**1.6 实现 1–6 逐条核对**：1 ✔（除 1.1/1.3）；2 ✔（列全部探测模型、徽标+版本、单行、状态标记、来源进 title、底部「管理接入 →」）；3 ✔（同一套徽标、原生无按钮、三态、统计文案）；4 ✔（后缀三态、unverified 可选、G1 提示）；5 ✔（前端测试真的存在）；6 ✔（除 1.5）。`server.rs` 未动是对的：`tauri.ts:361-380` 的 9 个资源命令全在 `server.rs:3940-3948` 注册表里，本次没有新 invoke。

## 2. 逻辑正确性

**2.1 provider 为空时静默跳过验证（高）** — `src/resources.rs:1447-1450`
`verify_on_first_use_at` 用 `find_model(agent, provider, id)` 精确匹配，provider 为空则必然 `Err → Skipped`。而来源是真实存在的：`DispatchOverlay.tsx:424-425` 注释自己写了「节点面板选模型时不写 provider」；旧图的 Target、旧 roles.json、`NodePanel` 的 provider 输入留空都会产生 `provider == ""` 而 model 在 pi 下的某个 provider 里。此时 G1 会显示「该模型未验证，首次派发会先验证真身」（`picked` 有 agent+id 的兜底匹配），引擎却一次请求都不发——承诺了不做，比不承诺更糟。
建议：provider 为空时退到 `agent+id` 匹配；唯一命中就用它，多命中要么按通道默认 provider 解析（与 `channels::build_invocation` 同一规则）要么全部验证；并补一条 `provider=""` 的用例。

**2.2 冒烟跑在项目写锁内、且在 worktree/快照/材料之后（高）** — `src/dispatch.rs:1817-1835`（锁与 worktree）→ `:1855`（before 快照）→ `:1873`（prepare_outbound）→ `:1903`（验证）
单写者节点会持 `acquire_project_write` 最长 60s 才真正派发：同一项目的其他写节点全部排队，而界面上只是「已运行 00:xx」。worktree 节点更亏：`create_worktree` 已经建好分支+目录、`prepare_outbound` 已经把 REQUEST.md/diff.patch/changed 铺进隔离目录、单写者还做了一次全项目快照，验证失败后这些全部作废（残留一个未使用的 worktree 与分支）。
建议：把验证块提到 `acquire_project_write` / `create_worktree` / `snapshot_directory` / `prepare_outbound` 之前（把 `error` 里早期的取消判定单独提前一行即可），验证只在「当前没有更早的错误」时跑。

**2.3 验证窗口内的取消不生效（高）** — `src/dispatch.rs:1888-1904`（取消检查）vs `:373-378`（`cancel_dispatch_node`）
`is_cancelled` 的检查在验证之前；取消的实现是「标脏 + kill 已注册 pid」（`process.rs:cancel_registered`），而 `run_smoke` 走的是无 registry_key 的 `run_process`（`resources.rs:1230`）——所以冒烟进程不会被杀，标记也没人再看：冒烟跑完后 `needs_oneshot = true`，真实请求照样发出去。用户点了「叫停」却收到一次完整派发。
建议：验证返回后立刻再判一次 `is_cancelled(&cancel_key)`（→ `error = "派发已取消"`）；更好的做法是把 cancel_key 透进 `run_smoke`（`run_process_registered`）让叫停真能杀掉那次冒烟。

**2.4 同一模型两路并发首派：双份冒烟 + 后写覆盖（中）**
`verify_on_first_use_at:1440-1466` 的设计是「锁内读→放锁冒烟→锁内重读→写」。重读再写这点是对的：不会覆盖期间别的命令（用户改 vendor、探测刷新）的写。但残留两个问题：① 两路 fan-out 到同一个 unverified 模型时，两次读都拿到 unverified → **真发两次短请求**（成本 + 端点调用，且在用户不知情下）；② 两次结果都会写回，最终档位由 write 顺序决定，一成一败时可能被后到的 failed 覆盖（或反过来）。冒烟失败不自动重试的语义也被这个顺序打乱。
建议：加一张进程级 in-flight 表（`Mutex<HashSet<(agent,provider,id)>>`）——第二路要么等待第一路的结果再读库（推荐），要么直接按第一路的结果判定；退一步至少做到「已 verified 不被后到的 failed 覆盖」。

**2.5 写回失败被吞，`cargo test` 与用户都看不到（中）** — `src/resources.rs:1466`（`let _ = save_store(...)`）、`:1462-1464`（读不出来直接 `finish(failure)`）
写盘失败或库被删/损坏时，函数仍按冒烟结果返回 Verified/Failed，节点继续/失败，但库里还是 `unverified` → 下一次派发**再冒一次烟**。也就是说「失败不自动重试」在这条路径上不成立。
建议：写回失败时把原因并进返回（新的第三态或 `Failed(原因)` 的附加说明），至少进 `runner_note`；不要静默。

**2.6 失败节点的 status/error/runner_note 口径：与任务书一致** — `dispatch.rs:1909-1912`（`error` 前缀「首次验证失败：」、`runner_note` 记「首次派发自动验证」）、`resources.rs:1356-1388`（`status=冒烟失败` + `verification=failed` + `safe_summary` 过的 `smoke_failure`）。✔ 一点结构性建议：失败原因只存在于自由文本里，`aicross-dispatch/1` 没有可用字段，G3/接入页无法机读区分「通道错误」与「验证失败」；建议只追加 `verify: skipped|ok|failed`（第 3 期蜂群已经在用同样的追加方式）。

**2.7 detected() 排序变化：影响面比报告写的小，但两处实现已经不一致** — `resources.rs:379-386` vs `types.ts:108-113`
Rust 把 native 与 verified 当作同一档（并列后按 agent/provider/id），TS 是 native 在前、verified 在后。同一 agent 下同时有 native 与 verified 条目（kimi 09-15 前冒烟过的条目）时，左栏与下拉的顺序会不同。既有测试：`orchestration.rs:1241` 改了断言 ✔，但 `resources.rs:2105` 的测试**名字**还叫 `..._sorts_by_status`，`orchestration.rs:1240` 还叫 `..._with_connected_first`（名字已失真）。`detected()` 的消费方只有 `dispatchable_from`（`orchestration.rs:567`）与展示，角色默认值走 roles.json，没有隐藏的「第一个模型」依赖，所以没有行为回归。建议统一一张排序表（TS 从同一规则生成或直接接受后端的顺序）并改掉两个测试名。

**2.8 左栏点行只传 `(agent, model)`，pi 同名多 provider 不可区分（中）** — `ResourcesTab.tsx:143-147`、`resources/mention.ts:insertMention`、`graph/GraphTab.tsx:711`、`graph/mutate.ts:parseSend`
`modelKey`/`modelSelectValue` 的注释自己说了「两个 provider 下的同名模型不会撞 key」「光靠 agent/id 区分不开」。左栏现在把这些条目都列出来（还可能一个绿点一个红胶囊），但两行文字完全一样、title 里也没有 provider（`modelHint` 第二行是 `source`，pi 的 source 是 `读:…` 路径），点哪一行都插入同一段 `@pi/<id>`；最终 provider 由 `detected()` 顺序 + `parseSend` 的 `find` 决定，**可能不是你点的那一行**。
建议：多 provider 的 agent 行内显示 provider 后缀（`glm-4.7 · zai`）；或让 `INSERT_MENTION_EVENT` 带上 provider（`@` 语法本身表达不了 provider，只能走事件/查表）。

**2.9 后端不跳过原生条目，只有 UI 藏按钮（低）** — `resources.rs:1737-1788`
`smoke_agent_models_at` 对 agent 的所有模型都冒烟；`apply_smoke_outcome` 对原生条目保持 `verification=native` 却把 `status` 写成「冒烟失败」。若从别处（或将来某个混合卡）触发，会出现「左栏无点、接入页显示原生、详情显示验证失败」的自相矛盾。UI 的 `hasThirdParty` 目前恰好等价于「agent 不是原生」，所以现实里很难触发——但口径把「原生不冒烟」写成了规则，就不该只靠 UI 守。建议后端也跳过原生（或返回一条说明）。

## 3. 安全

**3.1 自动验证不带项目材料：成立** — `resources.rs:1214-1236`：固定 prompt「只回复 OK」、cwd = `std::env::temp_dir()`、`blind=true`、`cc_switch_path=None`、`writable=false`、不传节点 prompt/材料/附件；结果只取 `identity`，进程 stdout/stderr 不进留痕也不回前端。启动时 cwd 是临时目录、构造参数里没有项目路径，材料无从进入。✔
**残留一条**：这次外发的隔离强度完全继承 `channels::build_invocation(blind=true)`。按 PRODUCT「盲的边界（09-01 实测）」，claude 只有加 `--restricted` 才不注入用户级 `~/.claude/CLAUDE.md`，pi 要看 `--no-context-files`。自动路径意味着用户不会盯着这次请求。建议在验收时确认 `blind=true` 对 claude/pi 确实置了这两个开关（我按约定没读 `src/channels/`），如果是「隔离：部分」，就在留痕/接入页标注自动验证的隔离等级。

**3.2 密钥不漏** — 失败链 `run_smoke Err → safe_summary`（sk-/URL 内嵌凭据/query secret/Bearer/环境变量值，`resources.rs:1593-1633`）→ 写库进 `smoke_failure`、返回 `FirstUseVerification::Failed(脱敏原因)` → `dispatch.rs:1911` 拼进 `error` → `:2144 safe_trace` 二次清洗 → 才进留痕/Reply/前端。全程不打印、不落库外位置。✔ 唯一提示：`响应模型不符：请求 X，响应 Y` 里的 Y 来自端点响应体，靠 `safe_summary` 的 200 字截断兜底，可以接受。

**3.3 写回不越界** — `apply_smoke_outcome` 只改 `(agent, provider, id)` 命中的那一条，且仅在 `apply_smoke_outcome` 成功时才 `save_store`；条目被并发删除时整条不写。✔（配合 2.5 的静默失败一起看。）

**3.4 用户不知情多一次请求（低）** — 一次 G1 授权在 unverified 目标上实际发两个请求。PRODUCT 定案允许，但 G1 文案只说「会先验证真身」，没说是额外一次真实请求。建议文案补半句（「会先发一个短请求核对真身」）。

## 4. 代码质量与契约

**4.1 只追加 ✔** `verification` 为新增 `#[serde(default)]`（`resources.rs:98-100`），旧字段一个没删；前端类型标 optional（`graph/types.ts:110`）以兼容旧图缓存。

**4.2 「详情」按钮没有样式，`summary` 规则成死代码（中，视觉）** — `ResourcesOverlay.tsx:355` 用 `className="agent-card-detail-toggle"`，`resources.css` 里**没有这个类**；同时 `resources.css:443-447` 的 `.agent-card-model-detail summary` 已随旧的 `<details><summary>` 一起作废。global.css 没有全局 button reset（只有 `*{margin:0;padding:0}`），所以这个按钮现在渲染成浏览器原生控件（灰底、凹陷边框、系统字体），夹在卡片里非常突兀。
建议：换成 `.row-action`，或补 `border:none;background:none;color:var(--text-3);font-size:11px;cursor:pointer`；删掉 `summary` 那条死规则。

**4.3 注释/文档漂移（低，但会误导下一期）** — `resources.rs:1-3`（文件头仍写「冒烟通过（status=已接入）的模型才进派发清单」）、`resources.rs:272-277`（`connected()` 注释称左栏资源 tab 用它）、`resources.rs:340-345`（`detected()` 同）、`orchestration.rs:562-564`、`NodePanel.tsx:58`（「顺序沿用后端排好的『已接入 → 待冒烟 → 冒烟失败』」）。现在左栏走 `getResources` + `modelsByAgent`，排序按 verification。

**4.4 死导出（低）** `types.ts:73 STATUS_PENDING` 已无引用（`STATUS_CONNECTED`/`STATUS_FAILED` 仍被 `verificationOf` 用）。

**4.5 两套厂商色并存（中，视觉纪律）** — 新徽标 `resources.css:8-22`：claude `#c4834a` / codex `#7d9aa6` / kimi `#7a86d6` / pi `#7aa585`；既有 `graph/vendor.ts:20-27`：claude `#D97757` / codex `#10A37F`（OpenAI 绿）/ kimi `#6E56CF`。结果是同一个 codex 在对话图上是绿点、在资源页是灰蓝徽标。PRODUCT「厂商色」一条的前提就是「一眼看出这段是谁跑的」，两套色相直接削弱这个功能。
建议：以 `graph/vendor.ts` 为唯一色源派生 badge tone（补一个 `pi` 条目；codex 用 OpenAI 绿），或把「资源页徽标色与图上色点不是同一套」写进 PRODUCT。

**4.6 第 2–8 期路径**：`run_one` 只在既有 `error` 判定之后插了一段，锁/快照/回滚/对账代码未动（`rollback_node_changes_core:1217`、`reconcile_dispatch_state:442` 原样）；蜂群不在本分支（全库无 `swarm_item`/`merge_result`）✔。前端只动 picker/types/两个展示组件与 G1 提示 ✔。**唯一真实的耦合风险就是 2.2/2.3**（写锁与取消都在插入点之前）。

## 5. 视觉纪律与可访问性

**5.1 G1 把常态画成红字（中）** — `DispatchOverlay.tsx:449` 用 `.dispatch-warning`，而 `GraphTab.css:49` 把它定义为 `color: var(--red)`。同一个文件上一行（`:396`）的注释还写着「红字只留给异常（TASK-07-5）」。unverified 是第三方模型的初始状态（本机 pi 几十条都是），满屏红字会让真正的异常（failed）贬值——正是「胶囊只给异常」要避免的。failed 那条（`:450`）保持红字是对的。
建议：unverified 改 `dispatch-reason`（text-3）。

**5.2 徽标字母对比度不足（浅色主题）** — `resources.css:16-22` + `:40`（9px）：
按 14% 同色底合成后估算对比度：Claude `#a2602a`≈3.9:1、Codex `#46707f`≈4.3:1、pi `#3f7552`≈4.3:1（Kimi `#4a55ab`≈5.2:1 通过）。都低于 AA 4.5:1。深色主题那套约 4.6:1，勉强过。徽标是 `aria-hidden`（`AgentBadge.tsx`）且旁边就有 Sans agent 名，所以信息不丢，但视觉上确实偏糊。
建议：浅色主题再压暗（≈`#8a4a1e`/`#33596b`/`#2f5c41`），或把字母色改成 `--text-1/--text-2`、色相只留在薄底上。

**5.3 「已验证」的绿点在浅色主题几乎看不见（中）** — `resources.css:71` 用 `var(--green)`，浅色主题是 `#34c759`（`global.css:341`），对 `#fafaf9` 约 2.2:1，低于非文本 3:1；而左栏 verified 的**唯一**可见标识就是这个 6px 点（状态词是 sr-only 的）。深色主题 `#7ec77e` 没问题。
建议：浅色主题给点一个专用深绿，或让 verified 在左栏也带字（与 unverified 对称）。

**5.4 焦点环弱且与既有约定不一致（中低）** — `resources.css:172-176`：`outline: 1px solid var(--border-focus)`，深色主题 `--border-focus` 是 `rgba(255,255,255,.2)`（`global.css:286`），1px 半透明白在 panel 底色上几乎看不见；仓库既有约定是 `GraphTab.css:652-655` 的 `2px solid var(--accent); outline-offset:-2px`。加了焦点环值得肯定，但强度要跟上。

**5.5 悬停提示键盘不可达（中低）** — `ResourcesTab.tsx:140` 的 `title`。键盘 Tab 到该行时浏览器不显示 title（`aria-hidden` 徽标 + 内容文本仍构成可访问名，读屏端 title 会作为 description，勉强可用），纯键盘用户丢掉来源路径与最近验证时间。建议：`:focus-visible` 时用 CSS 弹出同一段文本，或把来源放进行内可展开区/右栏详情。

**5.6 间距与字号阶梯（低）** — `resources.css:126`（`4px 6px`）、`:157`（`5px 6px`）、`:212`（`4px 6px`）、`:247`（`8px 10px`）都不是 4 的倍数（PRODUCT「间距」条）；`:40` 的 9px 不在六级阶梯内且过小。建议统一到 4 的倍数、徽标字提到 10–11px 并把徽标放大到 18–20px。

**5.7 其余合规 ✔** 胶囊只给 failed（`:82-90`），native 无点无字；「原生/未验证/已验证/验证失败」都是中性词，无 emoji，按钮 2–6 字；「管理接入 →」「N 个未验证（首次派发时自动验证）」文案合格。单色主题（obsidian/cappuccino/cobalt/moss）下四个全饱和色相会是全屏唯一彩色物——报告自己记了，建议这类主题下降饱和（`color-mix` 与 `--text-1` 混）或只留底色不留彩色字。

## 6. 测试

**6.1 报告里列的测试全部真实存在，断言的就是目标行为**（逐条核对）：
`resources.rs:2473` 旧库三态 + 原生不看旧 status + 旧字段保留；`:2512` 原生冒烟调用次数=0；`:2535` 失败落 failed/status 且第二次 Skipped、只冒一次；`:2564` 成功落 verified+真身、第二次 Skipped；`dispatch.rs:3208` 失败 → `status=="error"`、`error` 前缀、`runner_note` 含「首次派发自动验证」、stub 脚本 marker 不存在（真的证明没进通道），Verified 分支照常派发并记 note。前端 `resources-tab-09.test.tsx` 的「来源不进正文」是有效断言（title 里保留作为反证、剥标签后可见文字里没有）；`resources.test.ts` / `model-picker.test.ts` 的三态与排序断言与 Rust 的映射一致。`cargo check` 零 warning 的解释（`#[cfg_attr(test, allow(dead_code))]`）合理。

**6.2 缺口：接线只被忽略参数的替身覆盖** — `dispatch.rs:1765-1779` 的替身在 `cfg(test)` 下**丢弃 (agent, provider, model)**，所以没有任何测试能发现「键传错」或 2.1 的空 provider 问题；真实的 `verify_on_first_use`→`run_one` 链路（含 `store_path()`）端到端未被覆盖。

**6.3 缺口：并发取消/写锁无覆盖**（2.2、2.3、2.4 三条都无测试）。

**最该补的 3 个测试**：
1. **provider 为空 / 库里没这一条**：`verify_on_first_use_at(path,"pi","","glm-4.7")` 的一行单测（钉住「会验证」还是「Skipped 并在 runner_note 里说明」）+ dispatch 侧一条 `provider=""` 的用例。
2. **替身带上参数**：stub 记录 `(agent, provider, model)` 并断言等于节点目标；再补一条不装替身、用临时目录的真实路径测试（需要把 store 路径注入或加测试用 env 覆写 `aicross_root`）——同一测试可覆盖 2.5 的写回失败分支。
3. **验证窗口内的取消与写锁**：用一个慢冒烟（注入闭包/慢 stub）+ 第二路同项目写节点，断言 ① 取消后不派发、② 验证期间不持项目写锁（或钉住当前行为防回归）。

---

## 是否可交付

**不能按现状交付**：四态字段与旧库推断、UI 三态、下拉/G1 提示、测试、手册改口径都做完了（协议层可交付），但「首次派发先验证真身」这条本期核心防线在常见路径上失真（provider 为空静默跳过）、在 claude 第三方端点上整体豁免、并且验证窗口会堵住项目写锁、吞掉取消——这四条属功能性缺陷，不是美化问题。

缺项（按严重度）：
1. claude（及 codex/kimi）第三方端点的原生判定与「原生无手动验证入口」——需求级口径裁决 + 复用 `claude_third_party_endpoint()`（`resources.rs:45/244`、`ResourcesOverlay.tsx:322/408`）
2. provider 为空时静默跳过验证，G1 承诺不兑现（`resources.rs:1447-1450`）
3. 验证窗口内取消被吞、冒烟进程不可叫停（`dispatch.rs:1888-1904`、`process.rs:cancel_registered`）
4. 冒烟在项目写锁内、worktree/快照/材料之后执行（`dispatch.rs:1817-1903`）
5. 同一模型两路并发首派双份冒烟 + 后写覆盖（`resources.rs:1440-1466`）
6. 写回失败被 `let _ =` 吞掉 → 「失败不重试」不成立（`resources.rs:1462-1466`）
7. 「详情」按钮无 CSS + `.agent-card-model-detail summary` 死规则（`ResourcesOverlay.tsx:355`、`resources.css:443`）
8. G1 把 unverified 常态画成红字（`DispatchOverlay.tsx:449`）
9. 左栏多 provider 同名模型不可区分、点击目标可能与所选行不一致（`ResourcesTab.tsx:143`、`GraphTab.tsx:711`）
10. 可访问性：浅色主题徽标 3.9–4.3:1、verified 绿点 2.2:1、1px 半透明焦点环、悬停提示键盘不可达、9px 与 6/10px 间距越纪律（`resources.css:40/71/157/172/247`）
11. 两套厂商色不一致（`resources.css:8-22` vs `graph/vendor.ts:20-27`）
12. 注释/测试名漂移、`STATUS_PENDING` 死导出、MANUAL 多出的「一个月」句（`resources.rs:1-3/272/340`、`orchestration.rs:562`、`NodePanel.tsx:58`、`MANUAL.md:31`）