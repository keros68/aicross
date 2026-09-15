## 逐条核对（6 组）

**第 1 组 · 原生判定 = 原生 agent 且非第三方端点 — 已核实修好（附 1 处边界缺口）**
- `src/resources.rs:259-262` `is_native_model = is_native_agent && !third_party_endpoint`；`:252-254` 只对 claude 生效，`:1295-1315` 读进程环境 + `~/.claude/settings.json` 的 `env.ANTHROPIC_BASE_URL`，codex/kimi 暂按原生算并在 `docs/PRODUCT.md:232`、`docs/MANUAL.md:33-36` 写明。
- `:242-247` 改 `eq_ignore_ascii_case`，`Codex` 不再前后端判相反；`:1429` `apply_smoke_outcome` 走 `is_native_model`，第三方 claude 的成败能真的落进 verified/failed。
- UI 免改即可拿到手动入口：`ResourcesOverlay.tsx:408`（单条 `verification !== 'native'`）、`:446`（整卡 `hasThirdParty`）在降档后的 claude 上两条都成立。
- 测试 `resources.rs:2810`、`:2865` 到位；测试构建里端点判据走 thread_local（`:1319-1330`），不再跟开发机 settings 漂。
- **缺口**：降档目标取 `inferred_verification(status)`（`resources.rs:296-300` → `:270`）。旧库里 claude 的 `status` 若是「已接入」（TASK-07 时代在接入页手动冒烟过 claude 卡就会写成这个），换到第三方端点后降回的是 **verified 而不是 unverified**，首次派发不进验证、G1 不出提示、左栏是绿点。`:2810` 那条测试只给了 `status=待冒烟`，没钉住这个边界。

**第 2 组 · provider 为空不得静默跳过 — 已核实修好**
`resources.rs:1538-1585` `resolve_first_use_target`：精确命中 → 用它；provider 空且唯一命中 → 用库里那条的 provider（连 provider 一起做写回键，测试 `:2875` 断言 `target.provider == "zai"`）；多命中且未全验 → `Failed`「在多个 provider 下都有…」且一次请求都不发（`:2911`）；多命中但全验过 → 取首条走 Skipped；库里没有 → 新增 `NotInStore`（`:1471`），引擎照常派发但 `dispatch.rs:1827` 记「资源库无此条目，未做首次验证」（测试 `dispatch.rs:3403`）。G1 文案同步补了「先发一个短请求核对真身」（`src-ui/src/resources/types.ts:116`）。

**第 3 组 · 验证在写锁/worktree/快照之前且可取消、取消不写库 — 已核实修好**
验证块在 `dispatch.rs:1812-1845`，五处前置条件齐：worktree `:1852`、写锁 `:1868`、回滚 `:1876`、快照 `:1884`、备料 `:1903`，`error` 组装在 `:1947` 走 `verify_error` 分支。冒烟进程进注册表（`resources.rs:1351-1382` 走 `run_process_registered`，`dispatch.rs:1787` 传节点 `cancel_key`），`cancel_registered` 真能杀掉。取消不写库：`resources.rs:1646-1648` 在写回前判一次并返回 `Cancelled`，引擎 `:1837` 翻成「派发已取消」+ note；测试 `dispatch.rs:3304`（自己握写锁，验证失败必须直接返回而不是等锁超时，且 `wt` 未被建）、`:3361`、`resources.rs:2952`。

**第 4 组 · 并发只冒烟一次 / 写回失败让派发失败 — 已核实修好**
`resources.rs:1591-1599` `first_use_gate` 按 `(agent,provider,id)` 发闸门，`:1628-1642` 拿到闸门后重读库状态再决定，第二路读到第一路写回的结果返回 Skipped（测试 `:2981` 断言计数 = 1、返回 Verified + Skipped）。写回失败不再吞：`:1666-1673` 把原因并进 `Failed`，这次派发按失败结束（测试 `:3029`）。替身也带上参数了（`dispatch.rs:1765-1782` `FIRST_USE_CALLS`，测试 `:3337-3340` 断言三元组等于节点目标）。

**第 5 组 · UI 与可访问性 — 已核实修好（两处既有间距遗留）**
G1 unverified 走 `dispatch-reason`（`DispatchBits.tsx:25-29`，`GraphTab.css:69` 是 `--text-3`；`.dispatch-warning` 才是红字 `:49`），failed 保持红字；「详情」按钮有规则、`summary` 死规则已删（`resources.css:436-452`，测试正则钉住）；多 provider 同名行显示 `id · provider`（`types.ts:158/169`）、点击带 provider（`ResourcesTab.tsx:185`→`mention.ts:14` `#provider`→`mutate.ts:106-124` 按最后一个 `#` 切回并按 provider 精确匹配，配套 `GraphTab.tsx:699/712`），`layout.test.ts:213` 钉住「点哪行派哪行」；厂商色统一到 `graph/vendor.ts`（`AgentBadge.tsx:20` 内联 `--agent-tone`，CSS 里资源和页内那套 `--agent-*`/`.tone-*` 已全删，`vendor.ts` 补了 kimi/pi）；对比度（字母 `--text-1` + 18% 薄底，`--verify-green:#1f7a3d` 浅色主题）、2px 不透明焦点环（`:163-167`）、`aria-describedby` + sr-only（`ResourcesTab.tsx:185/191`）、字号 11/12px 都做到。左栏四个选择器的 padding/gap 全是 4 的倍数（`resources.css` 测试逐条断言）。

**第 6 组 · 文档与清理 — 已核实修好，不采纳项理由基本成立**
`MANUAL.md` 的「一个月重验」已删、改成两档准确口径（`:24-36`）；`STATUS_PENDING` 死导出已删（全仓无引用）；`resources.rs:1-3`、`connected()`/`detected()` 注释、`NATIVE_AGENTS` 口径注释、`NodePanel.tsx:58` 都对齐；测试改名 `:2306`。不采纳理由：DS 2.7（两套排序表）确有差异但无行为回归，超本轮范围——成立；DS 2.9（后端不跳原生冒烟）在当前 UI 下确实没有入口——成立；DS 2.6（`verify:` frontmatter）、GLM 6.2（dispatch 层计数钉不住语义）——成立；`orchestration.rs:562-564` 属约束外——成立。**唯一站不住的是 DS 3.1**：它被写在「不采纳」里，但实际是「本轮没核实」（报告自己也这么写），这条不能靠不采纳结案。

## 新缺陷 / 回归
- **无主路径回归**：`run_one` 的锁、worktree、回滚、快照、备料、对账（`reconcile_dispatch_state`）、心跳都只是加了 `verified` 前置，条件互斥正确；验证失败时不建 worktree、不取写锁、不写 task 正文、不进通道（`marker` 不存在）。全库无 `swarm_item`/`merge_result`，蜂群不在本分支。
- **新的用户可见失败模式（有意取舍）**：provider 为空且同 id 跨多 provider 时首次派发直接失败（`resources.rs:1574-1585`），而 G1 仍只显示「未验证…会先验证真身」，不预告这次会失败。
- **nit**：`is_native_model` 每次调用都可能读一次 `~/.claude/settings.json`（`:1305-1315` 无缓存），`ensure_verification`（`:294`）对每条 claude 模型各调一次；取消检查与写回之间（`:1646` vs `:1652`）有极窄竞态，后果只是多写一次结果；`first_use_gate` 表项不回收。均无功能影响。
- **nit（报告）**：写「新增的 9 条 Rust 测试」但列出 10 条，285→295 实际是 +10。

## 最后：R1 之后 TASK-09 是否可交付？

**可交付**——6 组必修全部落地并经代码核对，验证前置/可取消/并发/写回失败/UI 与文档口径都对上了；剩下的都是边界或后续期条目。

仍未闭合的项（按严重度）：
1. **（中）换端点降档目标取旧 `status`**：旧库 claude 的 `status=已接入` 时降成 `verified` 而非 `unverified`，首次派发不自动验证、G1 无提示、左栏误显绿点（`src/resources.rs:296-300`、`:270`；测试 `:2810` 未覆盖该状态）。建议下一轮：端点由官方翻成第三方时一律降 `unverified`（无视旧 status），并补一条用例。
2. **（中低）DS 3.1 未核实**：自动验证的外发隔离强度继承 `blind=true`，claude 是否真加 `--restricted`、pi 是否加 `--no-context-files` 仍未证实（本轮约束未读 `src/channels/`）；这条路径用户盯不到。
3. **（中低）provider 为空的跨 provider 同名目标会被判失败**：老图/手填 Target 会出现「首次派发直接失败」，G1 文案不预告（`resources.rs:1574-1585`；替代方案需复用 `channels::build_invocation` 的默认 provider 解析，本轮不可动）。
4. **（低）两套排序表不一致**：Rust 把 native/verified 并列，TS 分两档，左栏与节点面板/G1 下拉在同一 agent 混档时顺序可能不同（DS 2.7 未采纳，无行为影响）。
5. **（低）`smoke_agent_models` 仍对原生条目冒烟**（DS 2.9 未采纳，当前 UI 无入口），且该路径在持资源库锁时冒烟（`resources.rs:1975`），首次验证此时只能等库锁。
6. **（低）留痕无机读的验证结果字段**（DS 2.6 留后续期），失败原因仍在自由文本里，事后只能靠 `runner_note` 与 `error` 前缀分辨。
7. **（低）`orchestration.rs:562-564` 注释与测试名漂移**（明确不采纳，超出本轮 Rust 约束）。
8. **（低/挑剔）`resources.css` 还剩两处非 4 倍数间距**：`.res-dialog` gap 10px、`.res-field input/select` padding 6px 8px，属 TASK-07 遗留、本轮清单未点名。
9. **（nit）** 上述性能/竞态三条 + 报告「9 条 vs 10 条」笔误。