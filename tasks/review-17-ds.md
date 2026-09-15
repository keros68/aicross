## 总判

**不可交付**：核心的登录判定在真机上恒为 `unknown`——实现只认顶层 `ready` 布尔，而本机 pi 的 `pi auth check --json` 只输出 `{"status":"ready"|"not_ready","provider":…,"authType":…}`，导致「只显示已登录 provider」「G1 未登录默认不勾」在真机上全部不生效。

## 按严重度排序

1. **【阻塞】`pi auth check` 输出字段认错：真机永远得到 unknown，整个功能空转。**
   `src/resources.rs:1090-1112`（`pi_login_from_check` / `PiAuthReady` / `pi_auth_ready`）。
   - 问题：只反序列化顶层 `ready: Option<bool>`；真机输出没有这个字段 → `pi_auth_ready` 永远返回 `None` → 恒 `Unknown`。后果：真机上所有 pi provider 都进「未检查 N 个 provider」行，**已登录 provider 的模型完全不显示**（`ResourcesTab.tsx:265` 只渲染 `groups`，`ResourcesOverlay.tsx:478` 同），左栏/接入页看不到任何 pi 模型；G1「未登录」分区与默认不勾永不触发（`dispatch.ts:1048-1065` 需要 `logged_out`）；「重新检查」永远得到未检查。
   - 证据（本机）：`D:\pi-0.84.2\pi.exe` 内嵌源码 `545533-545549` 返回 `{status:"ready"|"not_ready"|"invalid", provider, authType}`，`599032-599045` `JSON.stringify({...result2, ...credentials})` 且 exit code `ready→0 / not_ready→1 / 其他→2`；仓库现成真机 fixture `fixtures/resources/pi_auth_check_ready.json:1` 就是这一行；`tasks/fix-01-r2-prompt.txt`、`tasks/review-01-glm-r3.md:14` 记录了同一结论。实现自己的测试把真机形状当作「legacy」：`resources.rs:2854-2856`（假 pi 输出 `{"status":"ready"}`）配 `resources.rs:2897` 断言 `Unknown`——方向正好反了。
   - 复现：让假 pi 打印 `fixtures/resources/pi_auth_check_ready.json` 的内容（`{"status":"ready","provider":"x","authType":"api_key"}`）再跑 `task17_pi_login_check_uses_safe_args_once_and_ignores_credentials`，期望值会是 `Unknown` 而不是 `LoggedIn`；真机 `pi auth check --provider zai-coding-cn --json` 输出同上。
   - 建议：判定改回顶层 `status`（`status=="ready" && code==0` → `LoggedIn`；`"not_ready"` → `LoggedOut`；`"invalid"`/解析失败/超时/命令缺失 → `Unknown`），并保留/显式说明 `ready` 分支的去留；测试直接用两个真机 fixture（现已是死文件）+ 一条 `invalid` 用例。这是单函数改动，其余部分不用动。

2. **【中】「重新检查」写库路径与「凭据不落盘」断言都缺真正的端到端覆盖，凭据断言接近同义反复。**
   `src/resources.rs:2220-2243`（`check_pi_provider_login_at`）、`src/resources.rs:2912-2923`。
   - 问题：`check_pi_provider_login_at`（UI「重新检查」唯一入口：trim、pi 缺失→unknown、写库+时间）没有任何测试；`task17_…ignores_credentials` 里的 `assert!(!stored.contains("do-not-read"))` 用的是手工构造的 model（只过 `apply_pi_provider_login`），即使实现把 stdout 误塞进别的字段（如 `meta`/`errors`）也不会红；探测→`merge_detected_models`→落盘这条链路同样只有单元级断言。
   - 复现：把 `check_pi_provider_login_at` 的 `apply_pi_provider_login` 换成把 `result.stdout` 写进 `model.meta`，现有测试全绿。
   - 建议：给 `check_pi_provider_login_at` 加注入点（假 pi 路径/runner，像 `check_pi_provider_logins_with_runner` 那样），跑一条走假 pi（带 `credentials` 字段）→ 读回落盘 JSON，断言只有 `login`/`login_checked_at` 且不含凭据串、`errors` 为空；再加一条假 pi 的 `--list-models` + auth 的探测合并测试。

3. **【中】`ResourceLogin` 严格 enum 反序列化：脏值让整个 resources.json 不可读，与前端口径冲突。**
   `src/resources.rs:48-56`（enum）、`276-280`（`load` 严格解析）；对照 `src-ui/src/resources/types.ts:237-240`（异常值按未检查）。
   - 问题：字段缺失有 `#[serde(default)]` 兜底，但值只要不在 `unknown/logged_in/logged_out` 里（旧版写过的值、手工改的、将来新增的状态），整个 `ResourceStore` 反序列化失败 → 资源页「读取资源失败」，探测/增删改全部被拒；而前端注释明确宣称「异常值按未检查」。升级前所有字段都是 `String`，不存在这类硬失败。
   - 复现：`resources.json` 里写 `"login":"expired"` → `get_resources` 返回「resources.json 解析失败：unknown variant `expired`…」。
   - 建议：用 `deserialize_with` 或手写 `Deserialize` 把未知字符串落到 `Unknown`（`#[serde(other)]` 对外部标签 enum 不适用），并补一条脏值测试。

4. **【中】TASK-14 额度用尽分区与 logged_out 分区并存：同一节点出现两行；引擎备选不看 login。**
   `src-ui/src/components/center/graph/DispatchOverlay.tsx:594-627`（两区）、`src/dispatch.rs:3515-3529`（`resource_fallback_models` 不带 login）、`src-ui/src/components/resources/ResourcesOverlay.tsx:355-372`（备选候选不过滤）。
   - 问题：`blockedIds` 是并集（`DispatchOverlay.tsx:144`），但两个分区各自 map 一遍原 id：一个既额度用尽又已登出的目标会在「额度用尽」和「未登录」各出现一行（两个复选框绑同一 state，不冲突但重复、说法互相矛盾）；额度用尽自动切备选仍会切到 logged_out provider，切过去必然失败且会中断后续备选链（`dispatch.rs:3299-3352` 只在 `quota_exhausted` 时继续）；备选顺序编辑器把未登录模型当普通候选，无提示。
   - 复现：给某 pi 模型同时写 `quota_exhausted_until`（未来）和 `login:"logged_out"`，G1 打开即见两行；或把备选顺序设为某已登出 pi 模型，主目标额度用尽后引擎直接切过去。
   - 建议：两区按优先级去重（未登录优先或额度优先，取一个展示），或在未登录区加「额度同时受限」标注；备选候选过滤/警示 `logged_out`，或引擎切备选前跳过 `logged_out` 并写 `fallback_skipped`。

5. **【低】混合登录状态的 provider 整组被折进「未检查」，已登录的模型跟着消失。**
   `src-ui/src/resources/types.ts:274-289`（`splitPiProviderGroups` 先 logged_out 后 unknown）。
   - 问题：只要组内有一条 `unknown`（典型：用户在已登录 provider 下手动接入一条模型，`resources.rs:1545-1546` 写入 `Unknown`），整组进 unknown 桶，该 provider 下所有已验证/已登录模型从正常列表消失，只能逐个点「重新检查」找回。
   - 复现：对 `provider:'zai'` 同时给 `login:'logged_in'` 与一条缺 login 的模型，`splitPiProviderGroups` 把 `zai` 放进 `unknown`。
   - 建议：按组内多数/最新探测结果归组，或在组内单独渲染未检查的那几行，不要把整组降级。

6. **【低】「全部探测」写锁内时长被放大；启动不受影响。**
   `src/resources.rs:2190-2213`（`detect_all_resources_at` 全程持 `store_lock`）、`1019-1014`/`1070-1086`（每 provider 一次子进程）。
   - 问题：TASK-17 让锁内新增 N 个 provider × 最多 10s ÷ 并发 4 的子进程（3 provider 最坏 +30s，18 provider 最坏 +45s）；期间 `get_resources`（`2158-2160`）与「重新检查」的写库阶段都要排队。真机 `auth check` 是本地读数（pi 侧 `allowModelNetwork:false` + `--no-refresh`）通常很快，风险只在 pi 卡死/超时场景。探测不在启动时跑（前端只在按钮里调 `detectAllResources`，`ResourcesOverlay.tsx:95-105`），启动无回归。
   - 复现：假 pi 的 auth 分支 sleep，点「全部探测」→ 探测期间左栏刷新卡在「正在读取资源…」。
   - 建议：登录检查放到锁外（照 `check_pi_provider_login_at` 的「先查后锁」），或分批查完即落盘。

7. **【低】命令缺失时「重新检查」静默变成「未检查」，界面无说明。**
   `src/resources.rs:2225-2227`、`src-ui/src/components/resources/ResourcesOverlay.tsx:172-181`。
   - 问题：pi 不在 PATH 时 `unwrap_or(Unknown)` 并返回 `errors: []`，用户看到状态从「未登录」跳到「未检查」而不知道原因（契约只要求 unknown，没要求静默）。
   - 复现：临时改 PATH 使 `where pi` 失败，点某 provider 的「重新检查」。
   - 建议：`errors` 里带一句「未找到 pi，登录状态未检查」，或前端在 `login_checked_at` 更新但状态为 unknown 时给一行说明。

8. **【低】新增折叠/重新检查按钮没有 `:focus-visible` 规则。**
   `src-ui/src/components/resources/resources.css:397-417`、`419-442`；对照同文件 `241-245`、`391-394`、`735-738`。
   - 问题：`.resource-logged-out-toggle` / `.agent-card-logged-out-toggle` 有 hover 无焦点环，键盘 Tab 到「未登录 N 个 provider · 展开」「重新检查」时看不到焦点（仓库既有约定是补 2px `--accent`）。
   - 建议：照既有两条规则补 focus-visible。

9. **【低】报告与实际改动/仓库状态有出入。**
   `tasks/REPORT-17.md`。
   - 问题：报告未提两个既有测试文件为适配新分组被改（`resources-tab-09.test.tsx:49-51`、`resources-overlay-09b.test.tsx:35-36` 各加 `login:'logged_in'`）；`fixtures/resources/pi_auth_check_ready.json`/`pi_auth_check_not_ready.json` 已无任何代码引用成为死文件；「4 个测试均能红」的四项都不覆盖真机输出形状（见 1），所以整轮红绿核对没有触到唯一的阻塞点。
   - 建议：报告补「改了哪些既有测试、为什么」；修完 1 后把两个 fixture 用进测试，删死文件或注明用途。

## 核过没问题的要点

- **命令与凭据（六铁律）**：argv 精确为 `auth check --provider <p> --json --no-refresh`（`resources.rs:1070-1086`，测试逐 provider 精确比对 `2907`），全仓无 `--credentials`；pi 只有传 `--credentials` 才会在 JSON 里带凭据（pi.exe:599044）。检查不写 stdin/env，`ProcessResult` 的输出不外传、`errors` 不带 stdout、本模块无任何 `log::/println!`（生产路径），`resources.json` 只新增 `login`+`login_checked_at` 两个字段。字段名认错（见 1）是功能问题，不是凭据泄漏问题。
- **契约**：`login`/`login_checked_at` 只追加；`ResourceLogin` serde default = unknown，旧 JSON 缺字段测试真读文件（`2952-2985`）；时间只写检查时刻；只有 pi 走登录检查（`detect_pi` 与 `check_pi_provider_login_at`），claude/codex/kimi 探测路径未动；`detected()`（`resources.rs:617-634`）与 `list_dispatchable_models`（`orchestration.rs:712-730`）仍返回全部已探测模型，不过滤登录状态，G1/@ 弹层/启动器仍能看到全部 pi 模型。
- **并发/超时/失败口径**：按 4 个一批 spawn+join，上限测试用 9 个 provider 验峰值=4（`2928-2948`）；10s 超时经 `run_process` 杀进程树（`process.rs` 有既有超时测试）；缺失路径与真超时各跑一次真子进程得 unknown（`2950-2971`）；同 provider 去重后只查一次（`2904-2909`）。
- **界面**：三态类型与「未检查」兜底；仅 pi 分组折叠，已登入组内未验证仍折叠（`splitUnverified`）；未登录/未检查两行文案与展开后「名字 + 重新检查」；左栏与接入页同一套分组函数；G1 logged_out 进「未登录」分区、区块头与提示句、默认不勾且可勾回（`DispatchOverlay.tsx:118-137`、`611-627`）；四列等高（`.resources-page` 100vh/overflow hidden、grid `repeat(4,minmax(0,1fr))`+`grid-auto-rows:minmax(0,1fr)`、卡片 `min-height:0`、`.agent-card-models` overflow auto）；「全部探测」走 `detect_all_resources`，会刷新登录状态。
- **文案**：未登录 / 已登录 / 未检查 / 重新检查 / 检查中…，全角标点、无 emoji、无半角破折号；`docs/MANUAL.md:226` 术语表已加行。
- **回归**：探测只在用户动作时跑，启动不新增子进程；TASK-15 的显示名/图标路径未被改动（新行只渲染 provider 名，不再拼图标）；既有 resources-tab-09 / overlay-09b 展示测试在补 `logged_in` 后仍覆盖原行为；额度用尽分区的既有收敛逻辑（convergeQuotaSelection→convergeBlockedSelection）未改语义。