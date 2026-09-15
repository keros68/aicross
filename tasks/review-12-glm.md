# TASK-12 审查报告（phase/12 工作区未提交改动）

**总判：可交付但有未闭合项** —— 契约四项、上限/遗留、两路并行、VALIDATION.txt 附带的主路径都成立且测试到位；未闭合的是失败/旁路路径上的 5 个问题（无阻断级）。说明：本会话没有 shell 工具，无法实际执行 git/cargo/vitest，以下为对工作区现状的静态审查；REPORT-12 的运行结果按报告采信，「能否红」按代码位置静态核对。

## 问题清单（按严重度降序）

1. **`src/dispatch.rs:2985-2992` + `src/dispatch.rs:3403-3435`：节点不派发时（首次验证失败 / 已取消）仍会跑验证命令并写 VALIDATION.txt，留痕失真。**
   问题：`verified == false` 时 `prepared = Ok(("(未发送)", vec![], 0))`，随后 `attach_review_validation` 的门槛只有 `!blind || !review_material_validation`（3412 行），照常执行：跑一次验证命令（最长节点超时一半，且 `run_validation` 不挂 cancel key，取消后也会跑完）、往盲目录写 VALIDATION.txt、把 prompt 追加成 `(未发送)

验证命令输出见 VALIDATION.txt`、materials 增加条目、`validation_attached: true`。这与 2898-2900 行注释「验证失败也不该留下一个白铺的隔离目录」的既有口径相悖，失败记录看起来像附了材料。
   复现：盲「验证」节点带 `reviewMaterialValidation`，目标为第三方未验证模型且首次冒烟失败（或派发前点取消）→ 该节点留痕 `validation_attached: true`、task 文件含那句指路、且实际白跑了一遍测试命令。
   建议：`verified` 为假时跳过 attach（调用移入 `if verified` 分支，或给 `attach_review_validation` 增加 verified 参数直接返回 `(prepared, false)`）。

2. **`src-ui/src/graph/dispatch.ts:1113-1115` + `src/dispatch.rs:3423`：同阶段多路盲验证会并发重复跑同一验证命令。**
   问题：`reviewMaterialValidationOf` 按「验证 + 盲」逐节点判定，每路 `run_one_attempt` 各自跑一次 `run_validation`。builtin-blind 两路、builtin-jury 三路同阶段并发，且这些节点没有上游 worktree，`review_material_dir` 都落在项目目录——同一个目录里并发跑 N 遍测试命令。命令不保证并发安全（缓存写、端口、锁），也成倍拉长时间。REPORT-12「没做/判断」只提到 jury 也会附 VALIDATION.txt，没提并发重复执行这件事。
   复现：盖内置盲审模式（或评审模式），验证命令配 `npx vitest` 之类，派发 → 同一项目目录里两个/三个 vitest 进程同时起。
   建议：引擎按 (run, 实际命令, review_material_dir) 去重、共享一次结果；或前端只在同阶段该组合唯一时带 flag；至少在手册/报告注明此代价。

3. **`src-ui/src/graph/dispatch.ts:640-645、671-686`：验证失败停链后，同父的盲「验证」兄弟节点永远停在「待派」。**
   问题：`appendValidationRetry` 只把等待中的「验收」标 `failed`（645 行），`closeWaitingAcceptance` 也只关「验收」；挂在同一个失败执行节点下的「验证」兄弟保持 `draft`，节点卡经 `nodeStatusLabel` 显示「待派」，但父节点 failed 使其永远不满足 `dispatchableNodeIds`。这是本次新长出的并行路线带来的新死状态（旧行为里不存在验证兄弟）。到顶路径与重派路径同样受影响。
   复现：经济模式跑到第三层，执行节点验证命令未通过且到上限 → 图上验收节点标「已跳过」，但旁边「验证」节点一直显示「待派」，点派发集合里又永远没有它。
   建议：两条路径里把同父「验证」兄弟一并标 `skipped_reason: '上游验证未通过'`（或 failed）。

4. **`src-ui/src/components/center/graph/DispatchOverlay.tsx:466-467`：结果屏遗留卡按「来源在本次 results」过滤，会把历史遗留一起带出来。**
   问题：`leftoverNodes(graph)` 取的是全图遗留，再用 `resultIds.has(source_node_id)` 过滤。同一验收节点被手动重派并再次到顶时会产生第二个遗留节点，两个遗留卡同屏出现；图上同一验收下也挂多个遗留节点。
   复现：上限 1 跑到遗留后，在结果卡对该验收节点点「重派」，再次 NEEDS_FIX 到顶 → 结果屏出现两张来源相同的遗留卡。
   建议：本轮新增的遗留 id 由 `reducePipelineStep`/`appendLeftover` 传出来（类似 `validationRetries` 的随步状态），结果屏只画本轮新增的；或至少按 created_at 只显最新。

5. **`docs/MANUAL.md:66-72`、`src-ui/src/components/right/orchestration/ExplainOverlay.tsx:43、46`、`src/orchestration.rs:396、418`：文案把「跑验证命令」安在验收那一路，与引擎机制不符。**
   问题：引擎的验证命令门挂在执行节点（`src/dispatch.rs:3189`：`role == "执行" && write != "只读"`），验收节点自己不跑任何命令；盲审模式整条预设没有执行节点，验收那一路实际既不跑命令、材料里也没有 VALIDATION.txt（flag 只给「验证 + 盲」）。用户按手册理解会在盲审模式期待验收路带命令输出。
   复现：读 MANUAL「一路验收跑验证命令，两路验证盲审」，实际观察盲审模式的验收材料——没有任何验证命令输出。
   建议：改成「验收给 PASS / NEEDS_FIX；验证命令输出由引擎附在验证路的 VALIDATION.txt 里」，两处 UI 文案与 orchestration.rs 注释同改。

6. **（备注）`src-ui/src/graph/__tests__/review-defaults-12.test.ts:236-245`：用 `orchestration.rs?raw` 文本切片断言引擎预设形状，脆且与 Rust 测试重复。**
   问题：切片依赖 `id: "builtin-economy"` / `"builtin-blind"` / `"builtin-arena"` 在源码里按此顺序出现（现盲审在擂台之前），挪动 preset 声明顺序即切出空串；断言的是源码字符串而非解析后的数据。Rust 侧 `task12_economy_*` / `task12_blind_*`（orchestration.rs:1599-1621）已覆盖同一事实。能红，不属同义反复，但维护成本高于收益。

7. **（备注）`src-ui/src/components/common/SettingsModal.tsx:652`：清空「修正轮上限」输入框即刻变 0。**
   `Number(e.target.value) || 0`，与相邻「总请求上限」的 `|| 1` 口径不同；0 的语义很重（任何 NEEDS_FIX 直接遗留、验证失败直接跳过），误清空即静默改变行为。语义本身符合任务书（上限 0 有测试钉住），仅提示统一口径（空则回默认 1）。

## 核过没问题的要点

- **契约四项齐全且只追加**：`DispatchSettings.fix_round_limit`（serde default 1，camelCase 读写有测试，dispatch.rs:41-65、7469-7477）；`Node.leftover`（Rust `Leftover` 镜像 snake_case 与 TS `Leftover` 一致，types.ts:82-97 / orchestration.rs:66-73，`Option + #[serde(default)]` 旧图缺字段可读，roundtrip 测试 orchestration.rs:1647-1665）；`DispatchNodeRequest.review_material_validation`（default false，缺字段/显式 true 都测，7480-7489）；`DispatchRecord.validation_attached`（default，旧记录可读，7603-7608）。`tauri.ts` 仅两处字节级追加（432-443、493-494）。
- **VALIDATION.txt 不会进项目目录、不会进 diff**：盲目录在 `dispatch_data_root()/runs/<run>/<node>`（dispatch.rs:1735、2932-2933），不在项目内。
- **附带时机与内容**：attach 在 `prepare_outbound` 之后、通道执行之前（2985-2992 → 3014 → 3116+）；先 `safe_trace`（strip ANSI + 脱敏，不受 `redact` 开关影响）再按 8 KB 在字符边界截断并注明（3389-3398）；命令为空写一行「未配置验证命令」（3419-3421）；超时取节点超时一半且至少 1 秒（3422）；prompt 末尾追加一句、materials 追加文件名；备选重跑先清盲目录（2798-2803）再走同一 attach，文件随之重建；`run_one_attempt` 内仅一处调用点，备选循环结构未动（2758/2828 两处尝试各自经过它）。
- **修正轮上限**：验证重派 `count >= limit`、验收 `acceptanceRound > limit`（含本轮，即已派修正数），同一 `fixRoundLimit` 驱动两处，默认 1；上限 0 = 直接遗留 / 直接跳过；到顶后 `appendLeftover` 生成 `kind:'turn'`、无 target 无 reply 的遗留节点，条目提取（`- `/`*`/`•`/编号/（N））取不到整段截 20 行。旧行为写死 3 → 默认 1 是任务书要求的默认值变化，用户调回 3 即恢复旧口径；到顶时验证未通过仍沿用「验收标已跳过」旧行为。
- **遗留节点只读、不可派发**：`dispatchableNodeIds` 开头排除且塞入 draft+target 也排除（测试 149-153）；`nodeStatusLabel` 返回 null、`nodeLabel` 返回「遗留」；G1 确认集合来自 `dispatchableNodeIds`，遗留节点进不了确认与派发；NodePanel 走 `LeftoverList`（无 TargetEditor、可复制）；聊天列无回复槽。
- **两路同阶段并行**：Rust 内置预设经济第三段 `[验收, 验证]`、盲审 `[验收, 验证, 验证]`（orchestration.rs:396-435，真实解析断言 1599-1621）；验证门只看 `role === '验收'`（dispatch.ts:801-803，测试覆盖「验证路 NEEDS_FIX 不触发修正」）；修正轮里同父验证兄弟一起长（`verificationSiblings`，带盲审文本、盲、draft）；角色默认「验证」沿用验收、跳过名单 arena|blind|jury 两侧一致（dispatch.ts:76 / orchestration.rs:609）；用户自建预设与旧图不受影响（跳过名单外照常填充）。`PRESET_NODE_TEXTS` / 讲解页 / MANUAL 术语表（遗留、修正轮上限、VALIDATION.txt 三行）已补。
- **对非盲/非验证节点无影响**：attach 早退（flag off / 非盲，测试 7545-7565）；前端只对「验证+盲」装配字段，验收/共享验证/顾问不带（测试 323-331）。
- **回归面**：`nextStageStep` / `applyDispatchResults` / `runStageLoop` 主干未动，`fixRoundLimit` 经 `settings` 传入（DispatchOverlay.tsx:180、299）；swarm wire settings 带 `fixRoundLimit`（swarm.ts:45，Rust 端 `DispatchSettings` serde 接住）；设置往返（默认/缺失/越界夹 0–9）有测试；旧测试改动合理——R5-2 显式传 3 保旧期望、重试上限测试用 `{e:3}+limit 3` 仍测到顶、验收上限测试改为断言「只多一个遗留节点」与新语义一致、08C-5 补 `fixRoundLimit: 1`。
- **测试非同义反复**：Rust 真跑 `echo`/`exit 9` 命令并断言文件内容与 frontmatter；TS 断言结构而非回读自身常量；提示词关注点测试两端都读真实文件（`?raw` / `include_str!`），删文件末尾两行即红，与 REPORT-12「能不能红」一节的回退点对得上。
- **界面文案**：「遗留」「修正轮上限」「N 条未闭合」「复制清单/已复制」「遗留节点不派发；处理完可在来源验收节点上再分叉」简短中性，与 MANUAL 术语表用词一致（第 5 条指出的「验收跑验证命令」除外）。