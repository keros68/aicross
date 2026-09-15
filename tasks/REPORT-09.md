# REPORT-09 资源页美化与冒烟口径

任务书 `tasks/TASK-09-resources-ui-and-smoke.md`，口径依据 `docs/PRODUCT.md`
「资源冒烟口径（09-15 定案）」与「视觉纪律」。分支 `ui/phase9`，未 commit、未 push。

## 契约

**已做：`ResourceModel.verification` 四态（native / unverified / verified / failed）。**
`src/resources.rs:37-40` 四个常量，`:45` `NATIVE_AGENTS = ["claude","codex","kimi"]`，
`:100` 结构体字段（`#[serde(default)]`）。旧库推断在 `:244 inferred_verification`
（原生 agent → native；其余按旧 `status`：已接入 → verified、冒烟失败 → failed、
其余 → unverified），`:258 ensure_verification` 只补空字段、不覆盖已有值，由
`load()` 与 `ensure_agents()` 两条入口调用。旧字段
`status` / `smoked_at` / `smoke_model` / `smoke_failure` 一个没删。

**已做：`verify_on_first_use`。** `src/resources.rs:1403 FirstUseVerification`
（Skipped / Verified / Failed(String)）、`:1422` 公开入口、`:1429`
`verify_on_first_use_at`（冒烟函数注入，测试用）。只对 `verification ==
"unverified"` 动手，判据与写回完全复用 `run_smoke` + `apply_smoke_outcome`；
原生、已验证、验证失败、库里没这一条都返回 Skipped（失败不自动重试）。
读状态时短暂持锁、放锁后再冒烟（一次冒烟最长 60s，整段持锁会堵住界面读资源）。
`apply_smoke_outcome`（`:1356`）顺带写 `verification`，但原生 agent 恒为 native
——原生模型不按冒烟结果改档，冒烟结果只落在 `status` 与详情里。

**已做：`run_one` 接线。** `src/dispatch.rs:1897-1912`，插在取消检查之后、
`needs_oneshot` 之前：失败 → `error = "首次验证失败：…"`（`status` 因此保持
`error`，通道不再进）+ `runner_note` 记「首次派发自动验证」；通过 → 继续派发并记
「首次派发已验证真身」。测试替身在 `src/dispatch.rs:1758` 的 `FIRST_USE_STUB`
thread_local + `:1765 first_use_verification`：`cfg(test)` 下没装替身就按 Skipped
放行，测试不碰用户真实的 `~/.aicross/resources.json`。

**已做：Tauri 命令不变。** `smoke_resource_model` / `smoke_agent_models` 原样保留
（手动重验）；`list_dispatchable_models` 返回的 `ResourceModel` 自动带上新字段。
**没做：`src/server.rs` 注册。** 没有新增命令，注册表无需改动，文件一个字节没动。

## 实现 1–6

**1. Rust 字段 / 推断 / verify_on_first_use / run_one 接线** — 见上。
`detected()` 的排序（`src/resources.rs:371-385`）从旧 `status_rank` 换成
`verification_rank`：原生 / 已验证 → 未验证 → 验证失败，同档 agent → provider → id。

已补测试：
- `resources::tests::task09_legacy_store_infers_verification_three_ways`（`:2473`）
  旧库三态映射 + 原生三家不看旧 status + 旧字段保留。
- `resources::tests::task09_verify_on_first_use_never_smokes_native_models`（`:2512`）
  原生条目冒烟函数调用次数 = 0。
- `resources::tests::task09_first_use_failure_persists_and_does_not_retry`（`:2535`）
  失败落 failed 并持久化，第二次调用 Skipped、冒烟只发生一次。
- `resources::tests::task09_first_use_success_is_verified_once_only`（`:2564`）
  通过落 verified + 真身，第二次不再验证。
- `dispatch::tests::task09_first_use_verification_failure_fails_node_before_channel`
  （`src/dispatch.rs:3208`）验证失败 → `status == "error"`、`error` 以「首次验证失败：」
  开头、`runner_note` 含「首次派发自动验证」，且 stub 端点脚本没被执行（marker 文件不存在）；
  同一路径改成 Verified 后照常派发并记「首次派发已验证真身」。

**2. 左栏资源 tab** — `src-ui/src/components/resources/ResourcesTab.tsx` 重写。
- 列全部探测模型：`modelsByAgent`（`src-ui/src/resources/types.ts:120`）替掉
  `connectedByAgent`，组内排序 原生 → 已验证 → 未验证 → 验证失败 → provider → id。
- agent 行 = 徽标 + 名称 + 弱化版本号：`ResourcesTab.tsx:126` 的
  `<AgentBadge>`（新文件 `components/resources/AgentBadge.tsx`，圆角方形 + 1–2 个字母
  + 色相，无商标图）；版本号 `resources.css:191-201` 等宽字 + `opacity: .75`。
- 模型行 = 模型名 + 右侧状态标记：`ResourcesTab.tsx:143-144`，行从两行改单行
  （`resources.css:207-244`），状态标记是新组件
  `components/resources/VerificationMark.tsx`——native 无点无字、unverified 灰点 +
  「未验证」、verified 绿点（状态词只给读屏）、failed 红胶囊「验证失败」，
  符合「胶囊只给异常状态」。
- 来源与时间挪到悬停：`title={modelHint(model)}`（`ResourcesTab.tsx:140`，
  `types.ts:184`），把来源路径、最近验证时间与真身、失败原因收进同一条 title。
- 底部链接 `ResourcesTab.tsx:83` 改「管理接入 →」。
- 顺手加了焦点环（`resources.css:171-176`）：这两种行都是键盘可达的按钮，原来没有
  可见 `:focus-visible`。

徽标色相在 `resources.css:7-25`：`:root` 一套（深色主题；本仓库只有
`[data-theme="light"]` 是浅色），`[data-theme="light"]` 一套压暗版。Claude 暖橙
`#c4834a`/`#a2602a`、Codex 石墨青 `#7d9aa6`/`#46707f`、Kimi 靛蓝
`#7a86d6`/`#4a55ab`、pi 苔绿 `#7aa585`/`#3f7552`、未知 agent 中性灰。底色是同色
14% 薄底，只做小面积标识。

**3. 接入页** — `src-ui/src/components/resources/ResourcesOverlay.tsx`。
- 同一套徽标：`:326` `<AgentBadge size="md">` 换掉 `.agent-card-mark`。
- 三态展示：`:351` `<VerificationMark showLabel>` 换掉 `StatusTag`（旧组件与
  `.resource-status` 四条样式一并删掉）。
- 原生不给「冒烟」按钮：单条 `:408` `verification !== 'native'` 才渲染，
  文案「验证」/「重新验证」；整卡 `:446` `hasThirdParty`（`:322`）才渲染，
  文案「全部验证」。相关状态词与提示语全部从「冒烟」改「验证」。
- 统计：`:220-222` 用 `unverifiedSummaryText`（`types.ts:145`）显示
  「N 个未验证（首次派发时自动验证）」，全部验过时这一行不出现；lead 段落改成
  「原生 CLI 的模型读清单即可用；第三方端点的模型首次派发时自动验证真身。」

**4. 派发下拉与 G1** — `src-ui/src/resources/model-picker.ts:31-43`
`modelStatusSuffix` 改走 `verificationOf`：原生与已验证不加后缀，未验证
「（未验证）」、失败「（验证失败）」。下拉仍列全部模型、unverified 可选。
G1 在 `src-ui/src/components/center/graph/DispatchOverlay.tsx:428/449/450`：
unverified → 「该模型未验证，首次派发会先验证真身。」；failed → 「该模型上次验证
失败，可在接入页手动重验。」`DispatchableModel` 加了可选 `verification`
（`src-ui/src/graph/types.ts:110`）。

**5. 前端测试** — 新文件 `src-ui/src/resources/__tests__/resources-tab-09.test.tsx`：
- `ResourceAgentList > 来源路径只在 title 悬停里，不渲染进行内正文` — 快照式断言：
  `renderToStaticMarkup` 的 markup 里还有 `models_cache.json`（title 属性），
  剥掉标签后的可见文字里没有它、也没有 `读:C:`。
- `ResourceAgentList > 模型行显示模型名；原生无状态点，未验证带「未验证」，失败用胶囊`。
- `ResourceAgentList > agent 行带字母徽标与弱化版本号` — 徽标映射到 class。
- `modelHint`（两条）、`agentTone`（徽标映射，四家 + 大小写 + 未知落中性色）。

`src-ui/src/resources/__tests__/resources.test.ts` 改写：`verificationOf`（三个
describe：显式字段优先 / 原生三家 / 第三方按旧 status 映射）、`modelsByAgent`
（分组排序 + 空组不出现）、`unverifiedSummaryText`。旧的 `connectedByAgent` /
`resourceSummary` / `resourceSummaryText` 连同它们的测试一起删掉（本次改动造成的
无用代码）。`model-picker.test.ts` 的状态后缀断言改成三态。

**6. `docs/MANUAL.md`** — 「接入资源」一节改口径：探测完就能用；原生 CLI 不验证、
第三方首次派发自动验一次（防静默降级），手动重验在接入页；来源与时间在悬停里。

## 范围之外但必须改的一处

`src/orchestration.rs:1241 dispatchable_lists_all_detected_with_connected_first`
是纯测试改动，由 `detected()` 换排序键引起：原来三条 claude 模型按旧 status 分先后，
新口径下它们全是 native、按 id 排，断言必然变。顺便给这条测试加了一组 pi 模型，
把「已验证 → 未验证 → 验证失败」的新排序也钉住。`src/orchestration.rs` 的生产代码
没动。

## 验证（全部实跑，输出原文）

```
$ cargo check
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.72s
```

零 warning。`cargo check --all-targets`（含 test target）也是零 warning：
`verify_on_first_use` 在测试构建里没有调用者，加了
`#[cfg_attr(test, allow(dead_code))]` 并在文档注释里写明原因。

```
$ cargo test
test result: ok. 285 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 7.31s
```

新增五条全绿：

```
test dispatch::tests::task09_first_use_verification_failure_fails_node_before_channel ... ok
test resources::tests::task09_first_use_failure_persists_and_does_not_retry ... ok
test resources::tests::task09_legacy_store_infers_verification_three_ways ... ok
test resources::tests::task09_first_use_success_is_verified_once_only ... ok
test resources::tests::task09_verify_on_first_use_never_smokes_native_models ... ok
```

```
$ cd src-ui && npx vitest run --configLoader native --pool threads
 RUN  v5.0.0 D:/work/aicross-ui/src-ui
 Test Files  24 passed (24)
      Tests  304 passed (304)
   Start at  09:12:43
   Duration  565ms (transform 65%, import 17%, tests 16%, worker 3%)
```

```
$ cd src-ui && npm run build
dist/assets/main-CuViL6Us.js                                   428.01 kB │ gzip: 211.63 kB
dist/assets/xterm-C7jn2Fqg.js                                  465.79 kB │ gzip: 123.12 kB

✓ built in 578ms
```

额外跑了 `npx eslint src/components/resources src/resources`：改前 2 error、改后
1 error（剩下的是 `ResourcesTab.tsx` 里既有的 `react-hooks/set-state-in-effect`，
与本次改动无关）。为了不新增 `react-refresh/only-export-components`，`agentTone`
与 `modelHint` 放在 `resources/types.ts` 而不是组件文件里。

## 约束核对

- 换行：改动的 13 个文件与 3 个新文件全是 LF，与原文件一致（`src/server.rs` CRLF
  与 `src-ui/src/tauri.ts` 混排都没动，本次不需要新增命令或新 invoke 包装）。
- `src/channels/` 没动；`src/server.rs` 没动。
- 没起窗口、没发真机请求（Rust 测试的冒烟函数是注入的闭包 / stub Python 脚本，
  dispatch 侧走 thread_local 替身；没有任何测试打到真实端点）。
- 没 commit、没 push，没写仓库以外的位置。
- 图标是统一风格的字母徽标 + 色相，没有任何商标图。
- 没跑 `cargo fmt`。

## 不确定处

- 「原生」按 agent id 判（claude / codex / kimi），不按端点判。claude 挂了
  `ANTHROPIC_BASE_URL` 时它仍算原生、不自动验证；这种情形由接入页的手动「验证」
  兜底（`run_smoke` 那边照样比对真身）。任务书契约就是这么写的，但真机上如果
  用户主要靠 cc-switch 给 claude 换端点，可能要把这条细化成「按端点判」。
- 界面只做了代码与样式核对，没起窗口看实际渲染（约束要求不起窗口）。徽标色相在
  cappuccino / obsidian 这类单色主题下会是全屏唯一的彩色元素，观感需要真机看一眼。

## 修正轮 R1

任务书 `tasks/fix-09-r1.md`（来自 `tasks/review-09-ds.md` 12 条与
`tasks/review-09-glm.md` 7 条）。分支 `ui/phase9`，未 commit、未 push。
Rust 只动了 `src/resources.rs` 与 `src/dispatch.rs` 的冒烟接线段，
`src/channels/`、`src/server.rs` 一个字节没动。

### 第 1 组 原生判定 = 原生 agent 且非第三方端点（DS 1、GLM 3）

- **已修**：`is_native_model`（`src/resources.rs:259`）= `is_native_agent` 且
  `!third_party_endpoint`；`third_party_endpoint`（`:252`）复用已有的
  `claude_third_party_endpoint()`（`:1295`），claude 挂 `ANTHROPIC_BASE_URL`
  （进程环境或 `~/.claude/settings.json` 的 `env`）即算第三方。codex / kimi
  本期没有端点事实源读法，仍按原生算，手册与 PRODUCT 都写明了。
- **已修**：`is_native_agent`（`:242`）改 `eq_ignore_ascii_case`——原来
  `NATIVE_AGENTS.contains(&agent.trim())` 大小写敏感，手工 agent id 写成
  `Codex` 时前端（`toLowerCase`）判 native、引擎判第三方（DS 1.4）。
- **已修**：`ensure_verification`（`:285`）从「只补空字段」改成「补空字段 +
  维持 `verification == native` ⟺ `is_native_model` 这条不变式」：库里存着
  `native` 的 claude 条目在换到第三方端点后降回按 `status` 推断的档（否则永远
  不会自动验证），换回官方端点再回到 native。用户手动验出来的
  verified / failed 仍不被推断覆盖。
- **已修**：`apply_smoke_outcome`（`:1429`）的 `native` 也改走
  `is_native_model`——第三方端点的 claude 冒烟成败会真的落进 verified / failed
  （GLM 指出原来恒 native，失败记不进）。
- **已修**：接入页因此自动拿到手动入口——单条「验证」按钮的条件是
  `verification !== 'native'`、整卡「全部验证」的条件是 `hasThirdParty`，
  第三方端点的 claude 两个条件都成立，不用再改 UI 代码。
- **已修**：口径写进 `docs/PRODUCT.md`「资源冒烟口径（09-15 定案）」原生那一条，
  `docs/MANUAL.md:24-35` 同步。
- 测试构建里 `claude_third_party_endpoint()`（`:1296-1317`）不读开发机的环境变量与
  `~/.claude/settings.json`，改由 `set_claude_third_party()`（`:1326`）显式给，
  默认官方端点——否则测试结果跟着开发机的 settings 漂。
- **已补**：`resources::tests::r1_claude_on_third_party_endpoint_is_not_native`、
  `resources::tests::r1_native_agent_match_is_case_insensitive`。

### 第 2 组 provider 为空不得静默跳过（DS 2）

- **已修**：新增 `resolve_first_use_target`（`src/resources.rs:1538`）。精确匹配
  `(agent, provider, id)` 落空且 provider 为空时退到 `agent + id`：唯一命中就用
  它（连 provider 一起拿来做写回键）；命中多条且其中还有未验证的，返回
  `Failed`「在多个 provider 下都有（…）且未全部验证，无法确定要验的端点」——验
  哪一个都可能不是真正要派的那一条，只能报错，这就是任务书说的「验不了的返回
  失败并写明原因」；命中多条但都验过了，随便挑一条走到 `Skipped`。
- **已修**：库里根本没有这一条时不再和「原生跳过」混成一档，新增
  `FirstUseVerification::NotInStore`（`:1471`），引擎照常派发但 `runner_note`
  记「资源库无此条目，未做首次验证」（`src/dispatch.rs:1827`）。DS 1.3 要的
  「事后分得清」就是这条。
- **已修**：G1 承诺与行为对齐，文案补上「会先发一个短请求核对真身」
  （`src-ui/src/resources/types.ts:116 verificationHint`）。
- **已补**：`resources::tests::r1_first_use_verifies_when_provider_is_empty`、
  `resources::tests::r1_first_use_reports_ambiguous_provider_and_missing_entry`、
  `dispatch::tests::r1_first_use_not_in_store_dispatches_with_note`。

### 第 3 组 验证窗口可取消且不占写锁（DS 3、4、GLM 4）

- **已修**：验证块整段前移到 `src/dispatch.rs:1812-1845`——在
  `create_worktree` / `acquire_project_write` / `rollback_node_changes_core` /
  `snapshot_directory` / `prepare_outbound` 之前。后面那五处全部加了
  `verified` 前置条件（`:1852`、`:1868`、`:1876`、`:1884`、`:1903`），验证没过
  就不建 worktree、不取写锁、不回滚、不做快照、不铺材料。`error` 的组装改成
  验证失败时直接用 `verify_error`（`:1947`）。
- **已修**：冒烟进程进注册表——`run_smoke_registered`（`src/resources.rs:1351`）
  走 `run_process_registered`（`:1374`），`verify_on_first_use` 把节点的
  `cancel_key` 传下去（`src/dispatch.rs:1787`），`cancel_registered` 现在真能杀
  掉这次冒烟。手动冒烟仍走不注册的 `run_smoke`（接入页没有叫停入口）。
- **已修**：取消不再被吞。`verify_on_first_use_at` 冒烟返回后先查一次取消
  （`src/resources.rs:1646`），取消了就**不写库**（免得把「进程被杀」记成验证失败）
  并返回新的 `FirstUseVerification::Cancelled`；引擎把它翻成 `error = "派发已取消"`
  + `runner_note` 记「首次派发自动验证，验证中已取消」（`src/dispatch.rs:1837`）。
  验证通过后还会再查一次取消（`:1842`）。
- **已补**：`dispatch::tests::r1_first_use_verification_runs_before_write_lock_and_worktree`
  （测试自己握着同项目写锁，验证失败的节点必须直接返回「首次验证失败：」而不是
  在写锁上等到超时；同时断言 `runs/<node>/wt` 没被建出来）、
  `dispatch::tests::r1_cancel_during_first_use_verification_stops_the_node`、
  `resources::tests::r1_first_use_cancelled_does_not_write_store`。

### 第 4 组 并发首派与写回（DS 5、6、GLM 7）

- **已修**：进程级「验证中」表 `first_use_gate`（`src/resources.rs:1591`），按
  `(agent, provider, id)` 发一把 `Arc<Mutex<()>>`。拿到闸门后**重读一次**库状态
  （`:1630`），所以两路并发首派只冒烟一次，第二路读到第一路写完的结果返回
  `Skipped`，不会出现一成一败按写回顺序覆盖。闸门只圈住同一个模型，不挡别的派发。
- **已修**：写回失败不再 `let _ =` 吞掉（`:1650-1668`）。`apply_smoke_outcome` 或
  `save_store` 失败时，库里还是 `unverified`、下次派发会再冒一次烟，「失败不自动
  重试」就不成立了；现在把原因并进返回值（「验证通过，但结果写回资源库失败：…」
  或「…；验证结果写回资源库也失败：…」），这次派发按失败结束。
- **已补**：`resources::tests::r1_concurrent_first_use_smokes_only_once`（两个线程
  过 Barrier 同时进，断言冒烟计数 = 1、两个返回值分别是 Verified 与 Skipped）、
  `resources::tests::r1_first_use_write_back_failure_fails_the_dispatch`。
- **已补**（DS 6.2）：dispatch 的测试替身不再丢参数，`FIRST_USE_CALLS`
  （`src/dispatch.rs:1765`）按调用顺序记下 `(agent, provider, model)`，
  `r1_first_use_verification_runs_before_write_lock_and_worktree` 断言它等于节点目标。

### 第 5 组 UI 与可访问性（DS 7–11、GLM 2、5、6）

- **已修**（DS 8 / GLM 2）：G1 的 unverified 提示不再是红字。抽出纯函数
  `verificationHint`（`src-ui/src/resources/types.ts:116`）给文案与 tone，
  组件 `VerificationHint`（`src-ui/src/components/center/graph/DispatchBits.tsx:25`）
  按 tone 选 `dispatch-reason` / `dispatch-warning`，`DispatchOverlay.tsx:450`
  换成一行调用。unverified → 常规提示，failed 保持红字。
- **已修**（DS 7）：「详情」按钮补样式 `.agent-card-detail-toggle`
  （`src-ui/src/components/resources/resources.css:436-452`），删掉随旧
  `<details><summary>` 作废的 `.agent-card-model-detail summary` 死规则。
- **已修**（DS 9）：左栏多 provider 同名模型。新增 `ambiguousModelIds` /
  `modelRowLabel`（`types.ts:158/169`），同一 agent 下同 id 挂多个 provider 时行内
  显示 `glm-4.7 · zai`；点击目标改成带 provider——`mentionRef`
  （`src-ui/src/resources/mention.ts:14`）在 provider 非空时补 `#provider` 后缀，
  `parseSend`（`src-ui/src/graph/mutate.ts:106-124`）按最后一个 `#` 切回来并按
  provider 精确匹配（模型 id 里不会出现 `#`；`@` 弹层的 `pickModel` 与过滤也走同
  一套，`GraphTab.tsx:699/712`）。没有 provider 的 agent 文本不变，老草稿照旧解析。
- **已修**（DS 11）：厂商色统一到 `src-ui/src/graph/vendor.ts`——`AgentBadge`
  直接 `vendorColor(agentId)` 内联成 `--agent-tone`，资源页自己那套
  `--agent-claude/--agent-codex/…` 连同 `.tone-*` 规则全删；`vendor.ts` 补了
  `kimi` 与 `pi` 两个条目。同一个 codex 在对话图上是 OpenAI 绿，在资源页也是。
- **已修**（DS 10）：对比度与焦点环。徽标字母改 `--text-1`、色相只留在 18% 薄底
  与 40% 内描边上（`resources.css:9-21`），浅色主题不再有 3.9–4.3:1 的字；
  verified 绿点新增 `--verify-green`，浅色主题 `#1f7a3d`（对 `#fafaf9` 约 5.1:1，
  原来 `--green` 是 2.2:1），接入页的「已检测」同源；焦点环改 `outline: 2px solid
  var(--accent); outline-offset: -2px`（`:163-167`），对齐 `GraphTab.css` 的既有约定。
- **已修**（DS 10 / GLM 5）：悬停提示键盘可达。模型行抽成 `ResourceModelRow`
  （`ResourcesTab.tsx:160`），`title` 的同一段文本挂进 `aria-describedby` 指向的
  `.resources-sr-only` 节点（`:185/191`）；`modelHint`（`types.ts:213`）顺手把
  provider 单列一行（原来它只是 source 的兜底，多 provider 时分不清端点）。
- **已修**（DS 10 / GLM 6）：字号与间距回到纪律值。徽标 16px/9px → 20px/11px，
  md 26px/11px → 24px/12px；左栏与接入页的 `5px 6px` / `4px 6px` / `8px 10px` /
  `gap: 6px` / `padding: 2px 4px` 全部归到 4 的倍数（行内小标签的 `2px 8px` 是
  PRODUCT 明写的例外）。
- **已补**：`resources-tab-09.test.tsx` 里
  `ResourceAgentList > 徽标色相从 graph/vendor.ts 取，不另立一套`、
  `ResourceAgentList > 同名多 provider 的模型行带 provider 后缀`、
  `ResourceAgentList > 悬停提示同时挂进 aria-describedby，键盘可达`、
  `modelRowLabel / ambiguousModelIds > 只有同 id 挂在多个 provider 下才加后缀`、
  `verificationHint / VerificationHint > 未验证走常规提示，验证失败才是红字，其余不出提示`、
  `verificationHint / VerificationHint > 渲染出来的 class 与 tone 对得上`，
  以及五条样式断言 `resources.css 的视觉纪律（修正轮 R1）> …`（详情按钮规则在 /
  死规则已删、焦点环 2px、徽标字走 --text-1 且无 9px、verified 专用深绿、
  左栏间距都是 4 的倍数）；
  `layout.test.ts > parseSend > #provider 后缀把同名模型解析到用户点的那一行`。
  `ResourceAgentList > 来源路径只在 title 悬停里` 的 `visibleText` 相应改成先剔掉
  `.resources-sr-only` 再剥标签——sr-only 视觉上不出现，来源仍然没进正文。

### 第 6 组 文档与清理（DS 12、GLM 1）

- **已修**：`docs/MANUAL.md` 删掉不存在的「条目超过一个月没用过，派发前重验一次」，
  改写成两档的准确口径并写明 codex / kimi 的第三方端点暂不覆盖。
- **已修**：`src-ui/src/resources/types.ts` 删掉死导出 `STATUS_PENDING`
  （全仓无引用）。
- **已修**：注释对齐实现——`src/resources.rs:1-3` 文件头（不再写「冒烟通过才进派发
  清单」）、`connected()`（`:379`）与 `detected()`（`:395`）的分工说明、
  `NATIVE_AGENTS`（`:43-47`）的口径注释、`src-ui/.../NodePanel.tsx:58` 的
  「已接入 → 待冒烟 → 冒烟失败」改成「原生 / 已验证 → 未验证 → 验证失败」。
- **已修**：测试名对齐——`resources::tests::detected_keeps_unsmoked_drops_cc_switch_and_sorts_by_status`
  改名 `…_keeps_unverified_drops_cc_switch_and_sorts_by_verification`。
- **不采纳**：`src/orchestration.rs:562-564` 的注释漂移与
  `dispatchable_lists_all_detected_with_connected_first` 的测试名。本轮硬性约束是
  「Rust 只动 `src/resources.rs` 与 `src/dispatch.rs` 里冒烟接线段」，
  `orchestration.rs` 不在范围内。两处都只是命名 / 注释失真，无行为影响，留到下一轮。

### 未采纳的其余审查建议

- **DS 2.7（两套排序表收敛）**：Rust 把 native 与 verified 并列、TS 分两档，
  左栏与下拉在「同一 agent 下同时有 native 与 verified 条目」时顺序可能不同。
  本轮没动：`detected()` 的消费方只有展示与 `dispatchable_from`，没有行为回归，
  收敛要改两侧排序契约，超出这 6 组范围。
- **DS 2.9（后端也跳过原生条目的手动冒烟）**：`smoke_agent_models_at` 仍对原生
  条目冒烟。第 1 组把 `apply_smoke_outcome` 的 native 判据改准之后，「左栏无点、
  接入页显示原生、详情显示验证失败」这个自相矛盾只在官方端点的 claude/codex/kimi
  被手动触发时才可能出现，而 UI 的 `hasThirdParty` 不给这个入口。不在本轮 6 组内。
- **DS 2.6（`.dispatch` frontmatter 加 `verify:` 字段）**：契约变更，留给后续期次。
- **DS 3.1（确认 `blind=true` 对 claude 置了 `--restricted`、pi 置了
  `--no-context-files`）**：要读 `src/channels/`，本轮硬性约束禁止改它；只读核对
  也没做，仍是未核实项。
- **GLM 6.2（run_one 层「通过后第二次不再验证」）**：这条语义由资源库状态保证，
  已被 `resources::tests::task09_first_use_success_is_verified_once_only` 钉住
  （第二次 `Skipped`、冒烟计数 = 1）；dispatch 层的替身每次都会被调到，在那一层
  加计数断言钉不住这条语义，故未重复补。

### 验证（全部实跑，输出原文）

```
$ cargo check
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.99s

$ cargo check --all-targets
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.15s
```

零 warning（`--all-targets` 含 test target 也是零）。

```
$ cargo test

test result: ok. 295 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 7.73s
```

本轮新增的 9 条 Rust 测试全绿：

```
test resources::tests::r1_claude_on_third_party_endpoint_is_not_native ... ok
test resources::tests::r1_native_agent_match_is_case_insensitive ... ok
test resources::tests::r1_first_use_verifies_when_provider_is_empty ... ok
test resources::tests::r1_first_use_reports_ambiguous_provider_and_missing_entry ... ok
test resources::tests::r1_first_use_cancelled_does_not_write_store ... ok
test resources::tests::r1_concurrent_first_use_smokes_only_once ... ok
test resources::tests::r1_first_use_write_back_failure_fails_the_dispatch ... ok
test dispatch::tests::r1_first_use_verification_runs_before_write_lock_and_worktree ... ok
test dispatch::tests::r1_cancel_during_first_use_verification_stops_the_node ... ok
test dispatch::tests::r1_first_use_not_in_store_dispatches_with_note ... ok
```

```
$ cd src-ui && npx vitest run --configLoader native --pool threads
 RUN  v5.0.0 D:/work/aicross-ui/src-ui

 Test Files  24 passed (24)
      Tests  315 passed (315)
   Start at  09:58:26
   Duration  550ms (transform 60%, tests 19%, import 18%, worker 3%)
```

（TASK-09 首版是 304 条，本轮 +11。）

```
$ cd src-ui && npm run build
dist/assets/main-BARkfd8k.js                                   427.86 kB │ gzip: 211.48 kB
dist/assets/xterm-C7jn2Fqg.js                                  465.79 kB │ gzip: 123.12 kB

✓ built in 710ms
```

另跑了 `npx tsc --noEmit`（无输出）与
`npx eslint src/components/resources src/resources`：仍是 1 error，就是 TASK-09
就有的 `ResourcesTab.tsx` `react-hooks/set-state-in-effect`，本轮没新增。

### 约束核对

- 换行：改动与新增的文件全部保持原样——`src/resources.rs`、`src/dispatch.rs`、
  `resources.css`、`docs/MANUAL.md` 与前端各 `.ts/.tsx` 都是 LF，`docs/PRODUCT.md`
  仍是 CRLF（用字节级脚本改的）。`src/server.rs`（CRLF）与 `src-ui/src/tauri.ts`
  （混排）本轮没动。
- Rust 只动 `src/resources.rs` 与 `src/dispatch.rs`；`src/channels/`、
  `src/server.rs`、`src/orchestration.rs` 没动。
- 没起窗口、没发真机请求（冒烟函数在测试里是注入的闭包，dispatch 侧走
  thread_local 替身，`claude_third_party_endpoint()` 在测试构建里读的是
  thread_local 而不是开发机的 settings）。
- 没 commit、没 push，没写仓库以外的位置。没跑 `cargo fmt`。
- 文案中性简短，胶囊仍只给「验证失败」。

### 仍不确定处

- 界面只做了代码与样式核对，没起窗口看实际渲染（约束要求不起窗口）。
  徽标现在是薄底 + 内描边 + `--text-1` 字，在 obsidian / cappuccino 这类单色主题
  下彩色面积比首版更小，但仍需真机看一眼。
- `#provider` 后缀会出现在用户的输入框文本里（`@pi/glm-4.7#zai`）。这是让「点哪
  一行就派到哪个端点」成立的最小代价，但它确实让 mention 文本变长；如果不能接受，
  替代方案是 `INSERT_MENTION_EVENT` 带 provider 走查表，代价是草稿文本与最终目标
  不再一一对应。
- provider 为空且同名模型跨多个 provider 时，本轮选择「报错而不是挑一条验」。
  这会让这类节点在首次派发时失败一次，直到用户在节点面板选定带 provider 的条目。
  如果真机上这类目标很常见，可以改成「按通道默认 provider 解析」，但那要复用
  `channels::build_invocation` 的解析规则，本轮不能动 `src/channels/`。
