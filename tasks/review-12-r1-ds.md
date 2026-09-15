## 总判

**可交付但有未闭合项** —— 12 条按清单口径全部落地、新测试静态核对能在旧实现下变红；未闭合的是第 3 条漏掉的「右栏『应用』给遗留节点盖预设」这条主力入口，另有 4 处低severity 残留（文案事实、测试接线/可移植性、修法边界）。本轮无 shell 工具，`cargo test` / `vitest` 未实际执行，运行结果按 `tasks/REPORT-12-R1.md` 采信，红/绿按代码路径静态核对。

## 逐条

**1. 混合批次不吞修正轮 —— 已修**
`src-ui/src/graph/dispatch.ts:788-803`：验收分支（`acceptanceRound > limit` 出遗留，否则 `appendCorrectionRound`）整体前移到 `results.some(status !== 'ok')` 之前，命中即 `continueRunning: true` 返回，同批 `error/timeout` 只留在该路（`failedResult` 走 `applyDispatchResults` → `reply.status='failed'` → 节点卡/面板「重派」`GraphTab.tsx:584-588`、`NodePanel.tsx:376-378`）。测试 `review-defaults-12.test.ts:164-170` 断言 `continueRunning=true` 且新增 `['执行','验收','验证']`；旧实现（先判非 ok）下 `continueRunning=false`、图不长，会红。

**2. 未发送不跑命令、不附材料 —— 已修**
`dispatch.rs:3363-3366` 显式枚举 `Materialized/Unsent`；`dispatch.rs:2987-2991` 首验失败/取消时为 `Ok(Unsent)`；`dispatch.rs:3493-3494` 命中 `Unsent` 直接原样返回、`validation_attached=false`，不跑命令、不写 `VALIDATION.txt`；`dispatch.rs:3004-3006` 仍按 `(未发送)` 走。测试 `dispatch.rs:7653-7677` 用 marker 断言命令未执行、文件未写；旧代码（哨兵 Ok 元组）会跑命令并建 marker → 红。

**3. 遗留节点不再是死路父节点 —— 部分**
已做：`layout.ts:68-83` `latestLeaf` 跳过遗留；`layout.ts:86-89` 新增 `continuationNodeId` 回退到来源验收，`GraphTab.tsx:228`（列修剪）、`GraphTab.tsx:386`（列输入框）接入；`GraphTab.tsx:589-593` 遗留可删（`mutate.ts:101-108` 放宽守卫），`GraphTab.tsx:594/603` 遗留不显示「分叉 / 盖预设」；测试 `review-defaults-12.test.ts:172-183` 旧代码下 `latestLeaf` 返回遗留即红。
**未闭合**：右栏「应用」没有这道闸。`OrchestrationPanel.tsx:138-152` 的 `applyPreset` 用 `focus.nodeId` 直接 `stamp`，按钮条件只有 `disabled={!focusNode || !selectedPreset}`（`OrchestrationPanel.tsx:237-243`），而遗留卡单击即设焦点（`GraphTab.tsx:557-561`）、`stamp.ts:41-95` 无遗留判断。复现：经济模式跑到上限出「遗留」卡 → 单击该卡 → 右栏选预设点「应用」→ 子图挂在遗留下，`dispatchableNodeIds` 首行就排除遗留、父节点永非 done → 全部节点永久「可派 0 路」。节点卡那个按钮藏了，主入口（右栏）没藏。

**4. VALIDATION.txt 内联进盲 prompt —— 已修**
`dispatch.rs:3501-3508`：文件照写（`3498`），随后把同一份（已脱敏、8 KB 字符边界截断）文本追加进 prompt（`验证命令输出：\n<text>`），对 claude `--tools ""` / pi `--no-tools` 都成立。测试 `dispatch.rs:7575-7595` 同时断言 `workdir/VALIDATION.txt` 内容、prompt 含备注与 `review-ok`；只写文件不内联时 prompt 断言必红。

**5. 验收职责文案 —— 已修（残留一处事实）**
`orchestration.rs:396-398、418-420` 注释、`ExplainOverlay.tsx:43、46、48`、`MANUAL.md:66-73` 全改为「验收（盲、只读，看产物给 PASS / NEEDS_FIX）」「派盲审前由执行节点跑一次验证命令…同批各验证路复用输出」。测试 `review-defaults-12.test.ts:364-372` 读真实文件断言三个串。残留：`MANUAL.md:73`（盲审模式段）与 `ExplainOverlay.tsx:46` 说「由执行节点跑一次验证命令」，但盲审/评审预设里没有执行节点，实际执行者是引擎在盲节点备料时（`dispatch.rs:3453-3477`）——见新发现 B。

**6. 同批验证命令只跑一次 —— 已修**
`dispatch.rs:3375-3383` 键 `run_id + 规范化 project`；`dispatch.rs:3453-3477` 用 `Arc<OnceLock>` + `get_or_init`（并发节点只跑一次、其余阻塞复用）；超时 `(timeout/2).max(1)`（`3472-3474`）；阶段收尾清缓存 `dispatch.rs:1880`（`dispatch_stage_core` 出口，每阶段一批一次）。测试 `dispatch.rs:7680-7710` 两路盲验证 marker 行数 == 1；旧实现（每节点各跑）会得到 2 行 → 红。

**7. run_one 集成点 —— 已修**
调用点唯一（`dispatch.rs:2994-2995`，`run_one_attempt` 内），`run_one`（test-only）→ `run_one_with_activity` → `run_one_attempt` 正是 `dispatch_stage_core` 的生产路径（`dispatch.rs:1841`）。测试 `dispatch.rs:7712-7751` 断言 `runs/node-1/VALIDATION.txt` 存在、frontmatter `validation_attached: true`、注入 hook 收到 `(project, 5s)`（`timeout_secs=10` 的一半）；删掉 2994-2995 即红。hook 有 `#[cfg(test)]` 门（`dispatch.rs:3386-3406`）。

**8. 角色默认与跳过名单的可见变化 —— 已修**
跳过名单两侧一致：`dispatch.ts:76` `['builtin-arena','builtin-blind','builtin-jury']` ↔ `orchestration.rs:607-611`；「验证」沿用验收默认：`dispatch.ts:53-62` ↔ `orchestration.rs:626-628`。手册写明「三路都要在确认派发页选择目标」（`MANUAL.md:72`）与「验证角色未指定目标时…沿用验收角色的默认目标」（`MANUAL.md:81`），讲解页同句（`ExplainOverlay.tsx:46`）；测试 `review-defaults-12.test.ts:349-362` 覆盖两侧。

**9. 设置页共用上限说明 —— 已修**
`SettingsModal.tsx:654`：「…验证命令未通过的重派与修正轮共用此上限；验证未通过到顶时该验收标「已跳过」。」与 `closeWaitingAcceptance` 的 `skipped_reason`（`dispatch.ts:678-689`）/`nodeStatusLabel`（`dispatch.ts:361`）一致；测试 `review-defaults-12.test.ts:373-376` 钉住两句原文。

**10. 停链后同父「验证」兄弟不再「待派」 —— 已修**
`dispatch.ts:641-646`（`appendValidationRetry`）与 `dispatch.ts:677-688`（`closeWaitingAcceptance`）都把同父 draft 的「验证」兄弟标 `skipped_reason: '上游验证未通过'`；状态词走 `nodeStatusLabel` → 「已跳过」，`dispatchableNodeIds` 因其父非 done 也不收。测试 `review-defaults-12.test.ts:208-221` 同时跑重派（`{}`）与到顶（`{e:1}`）两条路；旧代码不标兄弟 → 断言红。

**11. 同一验收不再挂第二个遗留 / 结果屏只看本轮 —— 已修**
`dispatch.ts:746-763` `appendLeftover` 先按 `leftover.source_node_id` 找既有节点复用 id（只更新 items/说明），`dispatch.ts:787/794` 收集 `leftoverIds`，四条 return 都带出（`802/828/831/834/836`），`runStageLoop` 累积去重（`dispatch.ts:1388-1390`），结果屏只画 `runLeftoverIds` 命中的（`DispatchOverlay.tsx:469-472`）。测试 `review-defaults-12.test.ts:223-231` 断言两次到顶后图上只有 1 个遗留且 id 相同；旧实现每次 `newNodeId()` → 2 个 → 红。

**12. 清空回默认 1 —— 已修**
`dispatch-settings.ts:26-32` `fixRoundLimitFromInput`：空白 → 1，`0` → 0，非法 → 1；`SettingsModal.tsx:652` 改用它。测试 `dispatch-settings.test.ts:41-45`；旧实现 `Number(x)||0` 下 `''`→0，测试无法通过（导出不存在）。

## 新发现的问题

**A（中）右栏「应用」可给遗留节点盖预设，仍是死路** — `OrchestrationPanel.tsx:138-152`、`:237-243`、`stamp.ts:41`、`GraphTab.tsx:557-561`
复现：经济模式跑到修正轮上限 → 单击「遗留」卡（获得焦点）→ 右栏选预设 → 点「应用」→ 新节点 `parents=[遗留]`，`dispatch.ts:92-104` 要求父节点 done/材料 → 永久不可派，工具条「可派 0 路」且无提示。
建议：`applyPreset` 先过 `continuationNodeId(graph, focus.nodeId)`（或对遗留节点禁用「应用 / 编辑编排的应用」按钮），与 `GraphTab.tsx:228/386` 同一口径。

**B（低）盲审/评审模式下「由执行节点跑一次验证命令」与引擎不符** — `docs/MANUAL.md:73`、`ExplainOverlay.tsx:46`
盲审模式/评审模式预设里没有执行节点（`orchestration.rs:416-436`、`src-ui/src/graph/dispatch.ts` 预设形状测试），真正跑命令的是引擎在盲节点备料阶段（`dispatch.rs:3453-3477`，`settings.validation_command`）。用户按文案会去找一个不存在的执行节点。经济模式的同句（`MANUAL.md:67-68`）可接受。建议改成「派盲审前引擎先跑一次验证命令，输出随盲节点一起附上」。

**C（低）第 2 条的接线没有端到端钉住** — `dispatch.rs:2987-2991` 对 `dispatch.rs:7653-7677`
新测试直接传 `Ok(OutboundPreparation::Unsent)`，只覆盖 `attach_review_validation` 内部；把 2990 行改回旧的 `Ok(Materialized("(未发送)", vec![], 0))`（即 DS#2 的原始缺陷）时，全部 Rust 测试仍绿。建议补一条：`FIRST_USE_STUB = Failed` + 盲 + `review_material_validation` 走 `run_one`，断言无 `VALIDATION.txt`、`validation_attached: false`、验证命令未执行（marker）。

**D（低）两条新 Rust 测试只写 PowerShell 命令，非 Windows 会失效/报错** — `dispatch.rs:7659`、`dispatch.rs:7685`（对照仓库既有写法 `dispatch.rs:4960-4963`）
`Set-Content`/`Add-Content` 在 `#[cfg(not(windows))]` 下走 `sh -lc`（`dispatch.rs:2616-2620`）：`task12_unsent_…` 的 marker 断言变成恒真（命令没跑成功也不会建文件），`task12_same_run_…` 会在 `read_to_string(&marker).unwrap()` 直接 panic。建议按仓库惯例补 `#[cfg(not(windows))]` 分支命令。

**E（低）缓存键不含材料目录/命令，取首个调用者口径** — `dispatch.rs:3375-3383`、`3453-3477`
同一批内若某路盲验证带 `upstream_artifacts.worktree`（`review_material_dir` 会改用 worktree），另一路没有，两者共用第一路的正文与超时；`clear_review_validation_cache` 只在 `dispatch_stage_core` 正常出口调用（`dispatch.rs:1880`），提前 `?` 返回会留下一条永不清理的条目。影响面小（内置预设同批材料目录一致），记录备查。

**F（低）修正轮与验证重派同批时的组合未处理** — `dispatch.ts:801-803`
`addedCorrection` 为真即早返回，整批 `validation_failed` 的重派/关闭逻辑被跳过：并用例（同一阶段既有「验收 NEEDS_FIX」又有失败「执行」）下，失败执行的等待验收/验证兄弟会停在 draft、重派也不排期。内置预设不会出现这种批次（执行与验收不在同一阶段），用户自建预设才会。建议在早返回前把 `validationFailures` 一并走一遍，或在注释里记为已知取舍。

## 核过没问题的点

- 契约只追加且两条读写路径都在：`fixRoundLimit`（`dispatch.rs:42-43,58,63-65` ↔ `tauri.ts:442`，camelCase+default 1）、`Node.leftover`、`reviewMaterialValidation`（`dispatch.ts:1121-1122` 只对「验证+盲」装配）、`validation_attached`（`dispatch.rs:372`、`1959`? 见 `serialize_frontmatter` 测试 7790-7795）。
- 遗留节点排除派发是双保险：加 target+draft reply 也排（测试 `review-defaults-12.test.ts:151-161`）；`nodeStatusLabel` 返回 null、`nodeLabel` 返回「遗留」；聊天列无回复槽（`GraphTab.tsx:655`）、面板只读 + 「复制清单/已复制」（`NodePanel.tsx:41-63`）。
- 修正轮上限语义自洽：`limit 1` 派一轮、`0` 首轮即遗留（`dispatch.ts:791`）；验证重派与验收共用同一 `limit`（`dispatch.ts:817-820`）；旧测试显式传 3 保旧口径（`dispatch.test.ts:285`）、上限测试改为「只多一个遗留」（`dispatch.test.ts:318-324`）。
- `runStageLoop` 主干未动：`nextStageStep` / `applyDispatchResults` / 重复批次放回 draft / 抛错路径（`dispatch.ts:1360-1392`），`leftoverIds` 为可选字段，`DispatchOverlay.tsx:319/330` 两处都回填。
- 结果屏「已跳过」行（`DispatchOverlay.tsx:696`）现在会带上被关掉的「验证」兄弟，与第 10 条意图一致；`已跳过` 不是终态——用户重派失败的执行节点成功后该兄弟会重新进入可派集合并正常跑完（届时 `skipped_reason` 被新 reply 覆盖），不影响判定。
- 验证材料仍不落项目目录（盲目录 `run_root/<node>`，`dispatch.rs:2932` 一带）、8 KB 字符边界截断（`dispatch.rs:3441-3451`）、先脱敏后截断（`3417` `safe_trace`）、命令为空写「未配置验证命令」（`3418-3424`）；非盲/开关关早退（测试 7629-7651）。
- 新增用户可见串符合 AGENTS.md：全角标点、无 emoji/内部词（「遗留」「修正轮上限」「已跳过」「N 条未闭合」「复制清单」与 `MANUAL.md:194-196` 术语表一致）；唯一事实偏差是发现 B。
- 预设形状测试已按「保持」项改为解析后按 id 查找（`review-defaults-12.test.ts:291-301`），不再依赖声明顺序；Rust 侧同事实由 `orchestration.rs:1599` 一带测试覆盖。
- 旧图/旧设置兼容：`leftover` 为 `Option`+default、`fixRoundLimit` 缺失按 1（`dispatch-settings.ts:59-61`）、蜂群 wire settings 带该字段（`swarm.ts:45`，测试 `swarm-08c.test.tsx:220-225`）。