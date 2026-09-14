# TASK-04 复审 R3（DeepSeek deepseek-v4-flash via pi，只读盲审，2026-09-14 09:37 起）

读完了修正单、补课单、报告（我发现报告里还有一节未在指令中提到但已落地的「修正轮 R3c」，也一并核对了）与全部涉及文件。下面是逐条结论。

---

## 逐条核对（22 条）

| # | 结论 | 依据与理由 |
|---|---|---|
| R3-1 | 已核实修好 | `src/dispatch.rs:102-117` 两个 struct 已无 `rename_all`，与 `orchestration.rs:98-116` 同形；Rust 测试钉键名 `src/dispatch.rs:1846-1860`（`diff/changed_files/worktree/stdout_summary`、ChangedFile `path/content`）；前端 `graph/context.ts:70-72`、`DispatchOverlay.tsx:418` 全用 `?? []` / `|| ''`；`tauri.ts:429,469` 直接复用同一个 `ReplyArtifacts` 镜像，没有第二份类型可漂移。 |
| R3-2 | 已核实修好 | `graph/dispatch.ts:23-41` 新增 `failedNodesWithoutResults`，`DispatchOverlay.tsx:215` 只在 catch 里用它；有结果的节点不再被改写，错误进 `runError`。测试 `dispatch.test.ts:56-61`。 |
| R3-3 | 已核实修好（测试弱） | 循环每层先算 `missingTargetNodeIds`，命中就回 `confirm` 并只列该层（`DispatchOverlay.tsx:181-187`）；后端在 `project_path`/run 目录/写锁/快照**之前**返回节点级 error（`dispatch.rs:369-380`，文案含「未指定」）。测试 `dispatch.test.ts:63-71`、`dispatch.rs:1863-1874`。修正单要求的「把这段抽成纯函数」没做，测试只钉住 helper，推进循环本身无测试。 |
| R3-4 | 已核实修好 | `SettingsModal.tsx:201-207` 保存后广播；`OrchestrationPanel.tsx:87-97`（预设+角色）、`GraphTab.tsx:69-78`（预设）重拉；`dispatch.ts:45-56` 兜底只填空目标。测试 `dispatch.test.ts:73-83`。次要：GraphTab 不在该事件里重拉 roles，但 `list_presets` 已按 roles.json 填充，实际不影响。 |
| R3-5 | 已核实修好 | 图卡 `GraphTab.tsx:436-440`、右栏 `NodePanel.tsx:237-239` 均加重派，走 `redraftNode`（保留 target，只把 reply 置 draft）；测试 `dispatch.test.ts:300-305` 断言重派后回到 `dispatchableNodeIds` 且父节点不动。 |
| R3-6 | 已核实修好 | `channels/mod.rs:5-11` 明示三元组 + `:595-602` 只在 `known_model_route` 命中时放行；zai `glm-4.7→glm-5.3-flash` 仍报真身不符（测试 `mod.rs:994-1001`）；资源冒烟 `resources.rs:1151-1164` 记响应 id 并追加「路由别名」，测试 `resources.rs:2109-2131`。 |
| R3-7 | 已核实修好 | `dispatch.rs:1061-1075` 32KB 分界、raw 落 `runs/<run_id>/<node>.raw.txt`、留痕留指针+末尾 4KB；测试 `dispatch.rs:1876-1889`。 |
| R3-8 | 已核实修好（实现方式间接） | `project_path` 空串直接 Err「请先在左栏「工作区」选择项目目录」（`dispatch.rs:1566-1575`），`preview_core:272` 与 `dispatch_stage_core` 共用；G1 靠 `previewError` 显示该文案并禁用确认（`DispatchOverlay.tsx:378`、`:298` 同族），不是本地判空，但效果等同。测试 `dispatch.rs:1891-1899`。 |
| R3-9 | 已核实修好 | `dispatch.ts:212-214` 决策行含 `验证=<命令>`（写权限段本就有）；测试 `dispatch.test.ts:307-313`。 |
| R3-10 | 已核实修好 | `store/app-state.tsx:408-423` 关图 tab 时清 `graphFocus`；reducer 测试 `__tests__/app-state.test.ts:74-79`。 |
| R3-11 | 已核实修好（归因未坐实） | `ResourcesOverlay.tsx:309,341-342` 改为受控 state + 按钮，不再依赖原生 summary；「WebView2 原生 summary 不稳」这个根因无窗口无法证实/证伪，但 backdrop 的 `onMouseDown` 被 `:635` 的 `stopPropagation` 挡住，理论上点不到，所以原归因站不住。功能目标（鼠标能开）已满足。 |
| R3-12 | 已核实修好 | `dispatch.rs:1046-1059` 覆盖 diff / changed_files[].content / stdout_summary；盲目录 `REQUEST.md`/`diff.patch`/`changed/` 与附件各自再走一次 `redact_text`（`:1460-1490`）；测试 `dispatch.rs:1901-1927`（含 diff.patch 与 changed/a.txt 落盘后无密钥）。 |
| R3-13 | 已核实修好（测试弱） | 探测失败两处都 `unwrap_or(false)`（`:925`、`:417`）→ 回退内置 diff / 退回单写者；materialize 失败先删 `diff-*` 再返回 Err（`:948`，成功路径 `:969`）。但测试 `:1929-1947` 没按修正单注入假 git 路径，只造了「项目目录不存在」，钉不住「git 不可执行」这一路。 |
| R3-14 | **改法有问题** | 后端两半到位：拒绝验收/验证非只读（`:382`）、盲节点强制只读+不拿写锁（`:1042-1045,1104-1105`），测试 `:1949-1956`。但 `validate.ts:26-27` 只有 role 分支，**缺 `visibility=盲` 分支**；`validate.test.ts:59-67` 的盲用例同时写了 `role=验收`，走的是 role 分支——这条断言是假绿（`visibility=盲, role=执行, write=worktree` 现在返回空数组）。另外拒绝是**阶段级 Err**：G1 不拦（validate 只是胶囊提示），一按确认整层节点会被 catch 判 failed。 |
| R3-15 | 已核实修好 | `dispatch.ts:213-214` 加 cc-switch 单轮无工具、kimi 盲隔离提示；`orchestration.rs:336-339` 的 `_models` 死参已删。遗留：调用点 `orchestration.rs:461`、测试 `:815` 的 `let models` 变成未使用变量 → cargo 会打 2 条 warning，且 `presets_from` 每次白读一次 resources.json。 |
| R3-16 | **改法有问题** | `successful_validation_output`（`dispatch.rs:1041-1044`）按**请求里的** `node.write=="worktree"` 判断，但 worktree 创建失败会退回单写者并直接改项目（`:1112-1123`，`node.write` 不变）。于是「建 worktree 失败/非 git/`git worktree add` 失败」这些回退情形既不跑验证、又写「worktree 节点未跑验证命令，采纳合并后再验」（`:1195-1202`），既漏了验证门又与实际不符。测试 `:1959-1962` 只测纯函数，覆盖不到。 |
| R3-17 | 已核实修好 | 放弃改名 `STATE.abandoned-<时间戳>.md`（`:305-313`）；run 结束清 cancelled（`:1380`）；「继续」用 STATE 头的 run_id 取消孤儿（`GraphTab.tsx:187-193`）。测试 `dispatch.rs:1959-1985`。 |
| R3-18 | 已核实修好 | `dispatch.rs:2877-2884` 缺 `AICROSS_CC_SWITCH_PATH`/`PROVIDER` 任一即 panic 明示；`live_economy_pipeline` 里没有 `println+return` 静默分支，全程硬断言。 |
| R3-19 | **改法有问题** | 顶部 `REPORT-04.md:13`、`:1070` 已改成「修正单允许的 fallback」，但修正单点名的两段没按现状重写：`REPORT-04.md:850`（R1-5）仍写「单写者且目标不是 codex 时 G1 显示…」「优先选已接入 codex 可写通道」，`:859`（R1-14）仍写「待上游完成」。这两句在代码里已不存在（`grep 无写工具/待上游完成` 为零命中），报告与代码不符。 |
| R3-20 | 已核实修好（漏测试） | `dispatch.ts:197-199` 已是 `<120 && 无换行/测试/验证/新建/实现/并`；但 `dispatch.test.ts:152-157` 没补 194 字长句、多步词、换行三类用例——修正单写的「改测试」没做。 |
| R3-21 | 已核实修好（漏测试） | `dispatch.rs:974-978` replace 只作用于 `diff --git`/`---`/`+++` 行头；没有「内容行含 `a/before/` 不被改写」的专门断言，现有 git 用例钉不住。 |
| R3-22 | 已核实修好 | `DispatchOverlay.tsx:84/298/378`：确认页直接提示并禁用确认。 |

---

## 特别核对的几处

- **A1 键名**：Rust 侧去掉 camelCase 后，图持久化反而被顺带修好——旧 build 会把 `changedFiles` 写进图，`orchestration::ReplyArtifacts` 只按 `changed_files` 读，`changed/` 全文会被静默丢掉；现在 IPC 与图模型一致。`DispatchArtifacts.diff` 少了 `#[serde(default)]`（`orchestration.rs:100` 有），前端若给出缺 `diff` 的 artifacts，整条命令会以「缺字段」失败而不是退化成空 diff——低危但不对称。
- **A2**：catch 只标「本轮没拿到结果」的节点；异常发生在下一层 `buildRequest`/`previewDispatch` 时 `activeNodes` 还是上一层的（已全部有结果），`failed` 为空 → 上一层 ok 节点安全，真实机崩溃场景（第 3 层组装抛 TypeError）不再污染已完成节点。
- **A3**：停在未指定模型这一层是**真的**回到 G1（不是只提示），且后端先于 run 目录/写锁/快照拒绝空 agent。
- **A4**：事件链完整；预设本身已由 `list_presets → fill_role_defaults` 按 roles.json 填好，前端 `applyRoleDefaults` 只是二次兜底。
- **A5**：两条入口都在焦点节点上（图卡需先单击选中，右栏点节点胶囊即可），重派后 `dispatchableNodeIds` 含它。
- **A6**：别名表只放行明示三元组，且判定时不再看响应 provider；zai 静默升级仍判真身不符。
- **A7/A8**：见上表，均已落地并有测试。
- **B12**：产物四处 + 盲目录三处都过 `redact_text(settings.redact)`，命中数累加进 `redacted`。
- **B13**：探测失败回退、临时目录清理都在；残留见下。
- **B14**：后端到位、前端缺盲分支（见 R3-14）。
- **B16**：见下（本轮唯一「会误导验收」的实现缺陷）。
- **B17**：三半都到位。
- **新缺陷扫描**：并发/写锁没有回归（盲节点不再抢项目写锁，单写者互斥逻辑与 `project_write_lock_is_mutually_exclusive` 未动）；留痕契约 `aicross-dispatch/1`、状态头 `aicross-state/1` 字段未增删；`Reply` 只追加 `artifacts`/`error_kind`，都是 `Option + default`，旧图兼容；第 2/3 期 `dispatchableNodeIds`/`applyDispatchResults`/`markPending`/`redraftNode`/`reducePipelineStep` 未改语义；`server.rs:3842-3857` 命令注册齐全，R3 没新增命令。

---

## 会导致误判的断言 / 真机上会难受的路径

1. **假绿断言（必须点出）**：`validate.test.ts:59-67`「盲验收 worktree 报错」——该节点同时是 `role=验收`，断言被 role 分支满足，`visibility=盲` 分支根本不存在。R3b 这条等于没覆盖。
2. **偏弱断言**：`dispatch.rs:1929-1947` 没注入假 git 路径（修正单明确要求）；`dispatch.rs:1959-1962` 只测 worktree 提示的纯函数；`dispatch.rs:1876-1889` 之外没有内容行改写断言；`R3-20/R3-21` 的测试更新缺失。
3. **真机会难受**：验收/验证节点写权限被改成非只读时，G1 不拦（只有一个胶囊提示），一按确认→后端阶段级 Err→**整层节点被判 failed**（不是只失败那一个），与 R3-3 的节点级 error 取向不一致；同一 agent 的多路层会被一起拖死，用户得逐个重派。
4. **真机会难受**：worktree 回退（非 git 项目 / `git worktree add` 失败 / git 不可用）后仍跳过验证，直接把未验证的项目改动交给盲验收，且提示文案说「采纳合并后再验」（实际已在项目里）。
5. **测试强度**：推进循环（`DispatchOverlay.tsx:170-225`）没有纯函数、没有测试；R3-2/R3-3 要求的抽取未做，回归只能靠窗口手测。
6. **未必炸但会误导**：`cargo test/check` 会打 2 条 `unused variable: models`（`orchestration.rs:461,815`），R3c 报告自己贴了这两条 warning 却没顺手清掉；`presets_from` 每次多读一次 resources.json。
7. **承诺漏做**：修正单「不采纳」里唯一的落地项——「继续」提示加一句「若上次是崩溃退出，请先手动检查残留进程」——没加（`GraphTab.tsx` 的 `graph-unfinished` 区域），报告也没记。

---

## 第 4 期是否可交付？

**达到可交付：22 条里 18 条已核实修好、4 条「改法有问题」都不是崩溃级，A1/A2/A3/A5 四条真机崩溃根因都已闭合，可以进窗口真派三层到 G4；但 B16（worktree 回退跳过验证）、B14（盲节点只读校验缺口+假绿测试）、B13/R3-19 的报告与测试欠账应在下一轮或本轮收尾时修掉。**

仍未闭合项（按严重度）：

1. **B16**：worktree 回退到单写者后仍跳过验证命令，且文案与事实不符（`dispatch.rs:1041-1044,1112-1123,1195-1202`）。
2. **B14**：`validate.ts` 缺 `visibility=盲` 只读校验；`validate.test.ts:59-67` 是假绿；后端把这类配置错误升级为整层失败（`dispatch.rs:382`）。
3. **B13 测试**：未注入假 git 路径，钉不住「git 不可执行」；且探测通过后 `git diff --no-index` 本体失败仍会把跑完的节点改成 error（`dispatch.rs:970-973`）。
4. **R3-19**：`REPORT-04.md:850,859` 两段仍是被 R2 取代的旧改法，与代码不符。
5. **R3-2/R3-3/R3-20/R3-21 的测试欠账**：推进循环无纯函数无测试；小任务 120 字/多步词、diff 内容行不被改写都没有断言。
6. **编译 warning**：`orchestration.rs:461,815` 未使用变量（+ `presets_from` 白读 resources.json）。
7. **「继续」提示缺残留进程一句**（修正单不采纳条款里唯一要求落地的话）。
8. R3-11 归因未坐实（只能窗口复核）；`DispatchArtifacts.diff` 缺 `serde(default)`；`raw_stdout/raw_stderr` 仍整条走 IPC（A7 只治留痕）；手动「重派」会连 artifacts 一起丢，下游盲验收退化成纯文本交付（既有行为）。

---
usage: {"input": 493, "output": 8173, "cacheRead": 193280, "cacheWrite": 0, "reasoning": 4120, "totalTokens": 201946, "cost": {"input": 6.902e-05, "output": 0.00228844, "cacheRead": 0.000541184, "cacheWrite": 0, "total": 0.002898644}}
