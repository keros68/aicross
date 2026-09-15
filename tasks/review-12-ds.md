总判：**可交付但有未闭合项** —— 契约四条与测试主体都对得上，「同阶段并行 + 修正轮上限 + 遗留节点 + VALIDATION.txt」主干能跑；但有 4 条真实缺口（并行验证路失败会吞掉验收的修正轮、attach 对未物化的哨兵 Ok 也生效、遗留节点做出不可派发的死路、claude/pi 盲审读不到 VALIDATION.txt）和 1 条界面文案与引擎行为不符，建议合并前至少修掉前三条或明确记为已知项。

## 按严重度排序

**1（中）同阶段「验证」那一路一失败，验收的 NEEDS_FIX 修正轮被静默吞掉**
- 位置：`src-ui/src/graph/dispatch.ts:797`（`if (results.some(status !== 'ok')) return continueRunning:false`）先于 `:800-813` 的验收分支；`runStageLoop` 在 `dispatch.ts:1373` 直接 `return done`。
- 问题：「验证门只看验收」只在 `validation_failed` 这一种状态下成立。经济/盲审第三段两路同批，若「验证」路以 `error/timeout/identity_mismatch` 收场、「验收」路返回 NEEDS_FIX，reduce 在验收之前就停了：不生成修正轮，`runStageLoop` 返回 done；验收节点 reply 是 `done`，`redraftNode` 只对 `failed` 开放（GraphTab.tsx:587、NodePanel.tsx:365），无法重派；此后单独重派失败的那一路，`acceptanceResults` 为空（`dispatch.ts:804`），修正轮永远不会补上。用户只能手动分叉。
- 复现：`reducePipelineStep(pipeline(1), [acceptResult('a1','NEEDS_FIX'), {...acceptResult('v1'), status:'error', error:'boom'}], {}, 1)` → `continueRunning=false`、图里没有新节点。
- 建议：先把「有验收 NEEDS_FIX 且未到上限」的判定做完，再落到失败停止；或把待补的修正轮记进 `validationRetries` 一类的状态，让后续批次能接上。补一条混合批次（验收 NEEDS_FIX + 验证 error）的测试。

**2（中）`attach_review_validation` 对「未物化」的哨兵 Ok 也生效（首验失败/取消时白跑一次验证命令）**
- 位置：`src/dispatch.rs:2985-2992`（`verified==false` 时 `prepared = Ok(("(未发送)", vec![], 0))`，第 2988 行；第 2992 行把它交给 attach），`src/dispatch.rs:3403-3412`（只看 `blind && flag` 且 prepared 是 Ok）。
- 问题：哨兵 Ok 与真实 `prepare_outbound` 成功不可区分。盲 + `review_material_validation` 的节点在首次真身验证失败（`Failed`/`Cancelled`）时仍会跑一遍验证命令（最长 节点超时/2，默认 300s，且会真的动项目/上游 worktree），并把 VALIDATION.txt 往一个从没创建过的盲目录里写：写失败 → 函数返回 Err，`validation_attached=false`（记录无误，只是白跑）；盲目录恰好已存在时则会写成功、给一个根本没派发的节点记 `validation_attached: true`。报告里「只在 … prepare_outbound 成功后生效」在这条路径上不成立。
- 复现：`attach_review_validation(&project, &settings{validation_command:"echo x"}, &task12_blind_node(true), &不存在的workdir, true, 60s, Ok(("(未发送)".into(), vec![], 0)))` → 命令被执行，返回 `Err("写入 VALIDATION.txt 失败…")`。
- 建议：把 `verified`/物化结果做成显式枚举（`Materialized(..)` / `Unsent`）传给 attach，Unsent 直接原样返回；补一条该分支的单测。

**3（中）遗留节点会成为分支末端 → 新节点永远不可派（静默死路），且不可删**
- 位置：`src-ui/src/graph/layout.ts:68-82`（`latestLeaf` 取最深节点，遗留节点是到顶后唯一最深节点）；`src-ui/src/graph/dispatch.ts:96`（可派要求每个父节点是材料或 `reply.status==='done'`）；`GraphTab.tsx:592-610`（分叉 / 盖预设 对任何聚焦节点都显示）；`graph/mutate.ts:101`（`removeDraftNode` 只收 `draft`，遗留节点没有 reply → 图上「删除」永不出现）。
- 问题：遗留节点无 reply 且永远不会有，所以挂在它下面的节点（列输入框发送、分叉、盖预设三条路都能造出来）永久不可派；工具条「可派 0 路」、派发按钮禁用，没有任何提示。用户按 NodePanel 那句「处理完可在来源验收节点上再分叉」走是对的（验收节点 reply done，可派），但遗留节点自己的分叉/盖预设与主列输入是陷阱。
- 复现：经济模式跑到上限出「遗留」卡 → 双击或点选该卡 → 按「分叉」/在列输入框发一句 → 新节点 draft、父节点遗留 → 可派数一直是 0。
- 建议：`latestLeaf`/列末端跳过遗留节点（回退到来源验收节点）；遗留节点隐藏「分叉/盖预设」；或在 `dispatchableNodeIds` 里把遗留父节点视为已满足；顺带给它一个删除出口。

**4（中低）VALIDATION.txt 对 claude 盲审（以及无产物的 pi 盲审）不可读，验证输出到不了审查员**
- 位置：`src/channels/mod.rs:173-176`（claude 盲：`--tools ""`，零工具）、`src/channels/mod.rs:269-274`（pi 盲：prompt 里没有 `BLIND_ARTIFACT_NOTICE` 就 `--no-tools`）；`src/dispatch.rs:3430-3437`（只写文件 + 追加一句指路，不内联正文）。
- 问题：功能产出只放在文件里。claude 盲节点没有任何工具；pi 盲节点在评审模式（父是材料、无 upstream_artifacts → 不加 notice）也是零工具。这两类 验证 节点收到「验证命令输出见 VALIDATION.txt」但读不到该文件，等于这项功能对它们没生效（经济/盲审模式选 claude 做验证路就是空转）。
- 复现：经济模式第三段验证路选 claude、验证命令 `echo hello` → 盲目录有 VALIDATION.txt，但 claude 进程参数是 `--tools ""`，回复不会引用验证输出。
- 建议：文本已截到 8 KB，直接像 `diff.patch < 30KB` 那样内联进盲 prompt（最省事、对所有厂商都成立）；或至少在附了 VALIDATION.txt 时给盲节点开只读工具。

**5（中低）讲解页 / 手册把「跑验证命令」安在验收节点上，与引擎行为不符**
- 位置：`src-ui/src/components/right/orchestration/ExplainOverlay.tsx:43,46`、`docs/MANUAL.md:66-67,71-72`；事实源：`src/dispatch.rs:3189`（只有 `role=="执行" && write!="只读"` 的节点跑 `settings.validation_command`），preset 里验收/验证是盲 + 只读且 target 无 validation_command（`src/orchestration.rs:395-414`、`Target` 无该字段）。
- 问题：「验收（盲，跑验证命令并给 PASS / NEEDS_FIX）」「一路验收跑验证命令」描述了一个不存在的动作；盲验收的上下文还刻意跳过带产物的直接父回复（`context.ts:107-109`），所以验收门既没跑命令、也看不到命令输出（输出只在验证路的 VALIDATION.txt 里）。这违反 AGENTS.md「说明句 = 事实 + 后果」。
- 复现：读代码即可；也可真跑经济模式，看验收节点的 workdir 无 VALIDATION.txt、引擎只为执行节点跑命令。
- 建议：文案改成「验收（盲，看产物给 PASS / NEEDS_FIX）」「两路验证盲审同一份产物并附验证命令输出」；若确实想让验收门看到输出，就把 `review_material_validation` 也放开到验收角色（需要在任务书层面确认）。

**6（低）盲审/评审预设下，验证命令会按验证路数并发跑 N 次（评审模式此前完全不跑）**
- 位置：`src-ui/src/graph/dispatch.ts:1106-1113`（所有「验证+盲」都带 flag）、`src/orchestration.rs:418-436`（盲审 2 路验证、评审 3 路验证）、`src/dispatch.rs:3403-3427`（每节点各跑一次）。
- 问题：同阶段 2–3 个盲验证节点各自在项目目录跑全局验证命令，并发上限内同时启动；评审模式从「不跑项目命令」变成「一次跑 3 遍」，`cargo test`/`npm test` 会争 target/.pytest_cache。报告已把「评审模式也会附 VALIDATION.txt」记为取舍，但代价（并发、耗时、评审模式行为变化）没写进手册/设置页。
- 复现：设好验证命令 → 盖评审模式派发 → 项目里同刻 3 个验证进程，3 个盲目录各一份 VALIDATION.txt。
- 建议：明确裁决并写进手册；或同一 run 内对同目录验证命令去重/串行后复用同一份输出；或只对带 `upstream_artifacts` 的验证节点开。

**7（低）Run_one 集成点无测试，报告的「能不能红」清单也没覆盖删掉 `dispatch.rs:2992` 那一行**
- 位置：`src/dispatch.rs:2992`（唯一调用点）对比 `src/dispatch.rs:7493-7584`（全部测试直接调 `prepare_outbound`/`attach_review_validation`/`review_validation_text`）。
- 问题：把 2992 行删掉，所有 TASK-12 Rust 测试仍绿（报告的回退清单只回退了函数体）。「ran_one 在盲目录物化后、派发前跑命令 + 记录 validation_attached」这条契约没有端到端覆盖；「超时取节点超时的一半」也没有断言。
- 建议：复用现有盲节点 run_one 夹具加一条：盲 + flag 的节点派发后断言 `workdir/VALIDATION.txt` 存在、留痕 frontmatter 里 `validation_attached: true`。

**8（低）角色默认与预设跳过名单带来两处可见行为变化（与「用户自建预设不受影响」的表述有出入）**
- 位置：`src/orchestration.rs:607-611`+`src-ui/src/graph/dispatch.ts:76`（跳过名单扩到 blind/jury）、`dispatch.ts:55-62`+`orchestration.rs:626-629`（验证→验收默认）。
- 问题：(a) 盲审模式整份不再填角色默认，其验收节点以前会拿到 `roles.acceptance`，现在 3 路全空，必须 G1 逐路选（避免两路验证同家，可以接受，但要在报告/手册里显式写）；(b) 任何用户自建预设里角色「验证」的空目标节点，盖章时现在会被填成验收默认模型（此前留空）。
- 复现：配好 acceptance 默认 → 盖盲审模式，三路模型为空；自建一个含空目标「验证」节点的预设 → 盖章后被填成 acceptance。
- 建议：接受则写进文档；不接受可只跳过「同阶段 ≥2 个验证路」的预设，或给验证角色单独的默认来源。

**9（低）设置页文案没说清「验证命令未通过的重派」也吃这个上限、且到顶不出遗留节点**
- 位置：`dispatch.ts:774-793`（两处共用一个 `limit`；验证路到顶走 `closeWaitingAcceptance` 标「已跳过」）、`SettingsModal.tsx:645-655`（只写「到上限后未闭合的条目生成遗留节点，不再派修正」）。
- 问题：默认从旧硬编码 3 变 1（设计已定），且上限同时压验证重派；文案只描述了修正轮这一半，用户不会知道验证未通过的重派也被压到 1 次、且到顶没有清单可看。报告在「没做/判断」里记了取舍，但界面/手册没写。
- 建议：字段说明补一句「验证命令未通过的重派与修正轮共用此上限；验证未通过到顶时该验收标『已跳过』」；若要按任务书让验证路也出遗留，则在 `closeWaitingAcceptance` 分支补 `appendLeftover`。

## 我核过没问题的要点

- 契约四条都在：`DispatchSettings.fix_round_limit`（`src/dispatch.rs:42-43,63`，serde default 1、camelCase，Rust/TS 两侧默认与 0–9 夹紧、往返都有测试）；`DispatchNodeRequest.review_material_validation`（`dispatch.rs:128-130`，default false、camelCase、前端仅「验证+盲」带）；`DispatchRecord.validation_attached`（`dispatch.rs:370-372`，struct 级 serde default → 新旧记录都会写 true/false，有测试）；`Node.leftover`（`orchestration.rs:60-74`，Rust 镜像 + serde default；旧图缺字段可读，新序列化出 `"leftover": null` 与既有 `Option` 字段一致，前端全部按真值判断）。
- 提示词文件末尾两条关注点确实来自文件本身：Rust 用 `include_str!` 断言（`dispatch.rs:7609-7621`），前端 `?raw` 读同一份（`ROLE_PROMPT_TEMPLATES`/`BLIND_PROMPT_PREFIX`），代码里没有复制文本；两条都不含 `PASS`/`NEEDS_FIX`，不影响 `parseAcceptance` 的尾部匹配。
- 修正轮上限语义自洽：`limit 1` → 派 1 轮修正（`acceptanceRound > limit`，`dispatch.ts:813`）；`limit 3` 第 4 轮出遗留；`limit 0` 首轮即出遗留；PASS 不生成；两处阈值（`acceptanceRound > limit`、验证重派 `count >= limit`）读同一个值；旧测试显式传 3 保持旧口径（`dispatch.test.ts:285`）。
- 遗留节点本身：`dispatchableNodeIds` 首行排除（`dispatch.ts:96`，强塞 target+draft reply 也排除，有测试）；`nodeStatusLabel` 返回 null、`nodeLabel` 返回「遗留」；`appendLeftover` 空正文/节点不存在不生成；条目提取按行形状（`- ` `* ` `1.` `2)` `3、` `（4）`），取不到整段截 20 行；面板只读 + 复制；结果屏只展示「来源验收节点在本轮结果里」的遗留卡。
- 同阶段并行成型：引擎预设经济模式第三段 `[验收, 验证]`、盲审一段 `[验收, 验证, 验证]`（`orchestration.rs:395-436`，Rust 测试 + TS 源码形状测试）；两路同父 → 同 stage；`stamp` 的 `PRESET_NODE_TEXTS` 下标与创建顺序一致（有测试）；修正轮把同父验证兄弟一起克隆且 `parents=[新执行]`（有测试）；验证角色在 `roleDefaults`、`validateNode`、引擎 `readonly_role`（`dispatch.rs:1696`）三处一致为只读。
- 审查材料附带验证输出：`VALIDATION.txt` 落在 `~/.aicross/runs/<run>/<node>/`（`dispatch.rs:1735` + `dispatch_data_root`，非项目目录）；盲节点无快照/无 artifacts（`before_snapshot` 仅非盲），不会进项目、不会进 diff；`REVIEW_VALIDATION_LIMIT=8*1024` 且截断落字符边界；`safe_trace` 脱敏不受 `redact` 开关影响；命令为空只写一行「未配置验证命令」；超时 `timeout/2`（下限 1s）；prompt 末尾追加指路句；非盲/开关关早退不写文件（有测试）；`run_one_attempt` 内只有 2992 一处调用，备选重跑清盲目录后会重建。
- 回归面：G1 只用 `dispatchableNodeIds` 取初始集合 → 遗留节点不进确认屏；`needsReview` 只看对账「未知」文本；`applyDispatchResults`/`nextStageStep`/蜂群/仲裁都按本轮结果行算，遗留节点不在结果里；设置往返有测试；旧测试的期望值改动（显式传 3、`swarmWireSettings` 加 `fixRoundLimit: 1`）都是契约追加的合理后果。
- 可红性：逐条核对了删实现后的断言路径——上限 1/0、遗留节点排除、验证重派读上限、`runStageLoop` 传值、`reviewMaterialValidation` 装配、提示词关注点，删对应代码都会红；没有发现同义反复（「默认上限 1」「引擎预设形状（源码扫描）」偏弱，但不是恒真）。
- 文案规范：新增用户可见串全部全角标点、无 emoji/拟人、无 G1–G4/留痕/对账这类内部词；「遗留」「修正轮上限」「VALIDATION.txt」与 MANUAL 术语表一致；「复制清单」是动词短语；除第 5 条那处事实错误外无问题。