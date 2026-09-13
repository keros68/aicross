# 代码审查 — 第 4 期「经济流水线」

## 1. 需求完整性

**1.1 「用户可在 G1 勾选把方案也发」未实现**
- 位置：`src-ui/src/graph/context.ts:119-123`、`src-ui/src/components/center/graph/DispatchOverlay.tsx:334-344`
- 问题：A 节明文要求盲验收可通过 G1 勾选补发方案文本。`contextPartsOf` 对盲节点直接把非父节点回复（方案）从 parts 里剔除，G1 的「本次外发」列表只渲染 parts 里已有的条目——只能减不能加。经济预设盖出的验收节点是单父 → `stamp.ts:32` 给 kind=`turn`，父回复也走 `盲` 分支被丢，结果经济流水线的验收节点在 G1 没有任何可补勾的方案项。
- 为什么：这是任务书里唯一给用户的盲审信息注入口，缺了它验收方永远看不到方案，只能靠 diff 猜意图。
- 建议：`contextPartsOf` 为盲验收额外产出「被排除的上游回复」候选列表（或单独导出 `excludedUpstreamPartsOf`），G1 渲染为默认不勾的复选项，勾选后进 `selection.messageKeys` 并让 `serializeContext` 保留。

**1.2 prompt 模板「可在设置里看、不可关」只做到后半句**
- 位置：`src-ui/src/components/common/SettingsModal.tsx:400-437`（角色页无模板展示）；模板只在 `src-ui/src/graph/dispatch.ts:190-201`
- 问题：C 节要求三类模板可在设置里查看。角色页只有 agent/档位/thinking 选择，没有任何模板文本展示。
- 建议：角色页底部加一段只读模板预览（从 `promptForRole` 或抽出的常量渲染），约 20 行。

**1.3 「用量表按阶段合计」未做合计**
- 位置：`src-ui/src/components/center/graph/DispatchOverlay.tsx:413-415`
- 问题：用量表只加了「阶段」列逐行展示，E 节要求按阶段合计（in/out/cache/耗时按 stage 小计）。
- 建议：按 `result.stage` reduce 出小计行插在各阶段末尾。

**1.4 遗留 8「live 测试 prompt 改走生产 serializeContext / BLIND_PROMPT_PREFIX」只算部分完成**
- 位置：`src/dispatch.rs:1705-1709、1925、1947、1978`
- 问题：Rust live harness 的 prompt 仍是手拼字符串；生产路径（前端 `buildRequest` → `serializeContext`）只由前端单测覆盖。真跑验证的组装路径与生产不一致。
- 为什么：盲审前缀、附件名裁剪、维度模板若漂移，ignored 真跑测不出来。
- 建议：REPORT 已披露此局限，可接受为过渡；后续让 harness 读一份由前端导出的 fixture prompt，或在 REPORT 交接里明确列为下期项。

**1.5 其余核对结果**：阶段推进按图层（`dispatchableNodeIds` + 逐层循环）✓；G4 修正轮长在图上、`acceptanceRound(working,…)<=3` 恰好允许 3 次修正共 6 节点（`DispatchOverlay.tsx:201`）✓；产物只追加 `Reply.artifacts`（`orchestration.rs:90-113`、`types.ts:23`）✓；盲目录恰三件套（`dispatch.rs:976-1017`）✓；内联规则（<30KB 全文本、仍拷目录、codex 提示、`upstream_artifacts` 为空才内联）✓；`roles.json` 与空节点填充（`orchestration.rs:311-361`）✓；小任务/链式/同厂商复核/理由行/50k 告警/宽泛脱敏提示/归类模型置灰 ✓；验证失败不送审 ✓。

## 2. 逻辑正确性

**2.1 `parseAcceptance` 子串误判可假 PASS**
- 位置：`src-ui/src/graph/dispatch.ts:173-179`
- 问题：`tail.toUpperCase()` 后 `lastIndexOf('PASS')`——英文总结里任何含 "pass" 的词（"all tests passed"、"surpassed"、"bypass"）都会命中，把无结论的回复判成 PASS 并终止闭环。
- 为什么：验收回复是自由文本，冻结模板只「要求」末尾写 PASS/NEEDS_FIX，模型经常附加英文说明；假 PASS 直接吞掉本应进行的修正轮。
- 建议：用 `\bPASS\b` / `\bNEEDS_FIX\b` 的 `matchAll` 取最后位置；顺带补边界测试（见 5.3）。

**2.2 `单写者` 只对 codex 有物理意义，claude/kimi 执行节点会空转 3 轮**
- 位置：`src/channels/mod.rs:146-151`（claude 固定 `--tools Read,Grep,Glob`）、`mod.rs:249`（pi 只读工具）、`src-ui/src/graph/dispatch-settings.ts` 无约束
- 问题：执行节点 write=单写者 + agent=claude/kimi 时，通道层根本没给写工具，`artifacts.diff` 恒空，验收必 NEEDS_FIX，自动烧满 3 轮。
- 建议：G1 在「单写者 && agent≠codex」时提示「该通道无写工具，产物为空」；或角色填充时把执行角色约束为可写通道。

**2.3 写锁获取失败静默降级**
- 位置：`src/dispatch.rs:736-740`（`acquire_project_write(project).ok()`）
- 问题：锁中毒（`.lock()` Err）时 `ok()` 丢弃错误，单写者节点无锁继续在项目目录跑；另一路单写者可能同时写同一项目——互斥保证被静默放弃。
- 建议：获取失败时把错误写进节点结果（status=error），不要无锁运行。

**2.4 盲验收交付依赖 `node.kind` 的不对称，存在「空验收」路径**
- 位置：`src-ui/src/graph/context.ts:111-116`
- 问题：kind=`accept` 且父节点无 artifacts 时父回复全文进 prompt（有东西可验）；kind=`turn`（经济单父验收、全部修正轮验收）时父回复被盲规则整体丢弃。若快照 IO 失败使 `artifacts=None`（`dispatch.rs:741-745` 的 `.ok()`），验收 prompt 只剩用户消息、盲目录为空——无待验对象。
- 建议：把规则改为「父回复：有 artifacts → 走目录且不进 prompt；无 artifacts → 回退进 prompt」，不看 kind。

**2.5 目录快照无单文件大小上限**
- 位置：`src/dispatch.rs:549-551`
- 问题：`snapshot_directory` 把每个文件整个 `fs::read` 进内存，前后各一次；项目里有几百 MB 的数据文件时内存/耗时失控（200KB 上限只管 changed_files 内容，不管快照）。
- 建议：超过阈值（如 8MB）只记标记位，diff 按「Binary files … differ」处理。

**2.6 「继续」不处理孤儿子进程**
- 位置：`src-ui/src/components/center/graph/GraphTab.tsx:135-145`
- 问题：应用崩溃前已在跑的 CLI 子进程可能仍存活并写项目目录；「继续」把 pending 重置为 draft 直接重派同一单写者节点，两个进程并发写。
- 建议：继续前对 STATE.md 里记录的 run_id 逐节点 `cancel_dispatch_node`，或在提示文案里告知风险。

**2.7 轻微项**（各一行）
- `dispatch.rs:264-268`：`inspect_dispatch_state` 靠中文状态子串判未完成，与 `write_state` 文案耦合——正是遗留第 1 条在通道层修掉的那类写法；建议 STATE 头部加机器可读行。
- `dispatch.rs:435-439`：`validation_failed` 在 STATE 里被归为「失败」，与前端「验证失败」不一致；排查时会误导。
- `src-ui/src/graph/dispatch.ts:99-108`：`applyDispatchResults` 重建 reply，采纳标记在重派/修正后丢失。

写锁互斥与排队本体（全局锁 + 阶段内调度 + 「等待写锁」事件/STATE）、worktree 回退、G4 终止、stage 计算（`graphStage` 带环防御）、类型化错误 kind、收起后台续跑、串行推进下并发上限——核对无误。

## 3. 安全

**3.1 唯一值得一提的边角：changed/ 路径校验漏 Windows 盘符相对路径**
- 位置：`src/dispatch.rs:999-1006`
- 问题：只拒 `is_absolute()` 和 `ParentDir`；`C:x` 这类带前缀无根的路径两者都不命中，`changed.join("C:x")` 会落到盘符根。实际上 changed_files 的 path 全部来自后端自己的快照相对路径，模型无法注入，故不可达——属纵深防御的缺口。
- 建议：加一条 `relative.components().next() 是 Prefix 时拒绝`。

其余核对无误：REQUEST.md / diff.patch / changed/ 内容 / 附件 / 正文落盘前全部过 `redact_text`（`dispatch.rs:977-1054`）；`stdout_summary`、raw_*、error 走 `safe_trace`（`dispatch.rs:879-883`）；全程不读 `~/.pi/auth.json`、`~/.cc-switch`，cc-switch live 测试只从环境变量取路径；写权限只有 codex 拿 `workspace-write` 且仅当 `write!=只读`（`mod.rs:176-183`），验收/盲节点 cwd 在 run 目录，物理上碰不到项目目录；盲目录先验空再写（`dispatch.rs:966-975`）。未发现问题。

## 4. 代码质量

**4.1 两套原子写实现**
- 位置：`src/orchestration.rs:367-405` 与 `src/dispatch.rs:1253-1283`
- 问题：同语义的临时文件+rename+兜底删除各写一份。建议合并到一个 fs 工具模块。

**4.2 经济模板文案三处硬编码**
- 位置：`src-ui/src/graph/dispatch.ts:190-201`（生产）与 `src/dispatch.rs:1925、1947、1978`（live harness）
- 问题：同一段模板在 TS 与 Rust 测试里各写一份，改模板时 harness 会静默漂移。建议 harness 从 fixture 文件读，或加交叉注释。

**4.3 「续接时只发 current_message」判断重复**
- 位置：`src/dispatch.rs:303-310`（preview）与 `763-772`（run_one）
- 问题：口径各写一份，漂移时预览的脱敏计数与实发不一致。建议抽 `is_continuing(node)`。

**4.4 死代码**：`SettingsModal.tsx:464-470` 归类模型 input 已 `disabled` 但保留 `onChange`。

**4.5 confirm 列表与实际派发集合不一致**
- 位置：`DispatchOverlay.tsx:41-44`（全部 draft+target 节点）vs `:174`（实际按 `dispatchableNodeIds` 派发）
- 问题：重开半途的图时，确认页会展示并预览本轮不会跑的深层节点。建议对父未完成的节点标注「待上游完成」。

第 2/3 期回归：`stamp/mutate/layout/validate` 未动，`dispatchableNodeIds`、续接、采纳、留痕契约（schema 仍 `aicross-dispatch/1`）语义未变——未发现破坏。

## 5. 测试

验收第 2 条要求的四组都在且真断言：三 visibility 组装（`context.test.ts:59/74/91`，盲验收测试 `:280-294` 断言不含 `PLAN_TEXT`/`EXEC_TEXT`/`DIFF_EXEC`）；写锁互斥（`dispatch.rs:1570-1585` 跨线程限时断言）；结论解析三种（`dispatch.test.ts:112-116`）；小任务/链式（`:118-123`）。另有盲目录三件套、角色填充、无序 usage、宽泛脱敏等有效测试。

最该补的 3 个：
1. **`start()` 推进循环本身**：层级推进、`acceptanceRound<=3` 封顶、`validationRetries<3`、任一非 ok 即停——现在全部 cap 逻辑只活在组件闭包里，零覆盖。抽成纯函数 reducer 再测。
2. **`artifact_diff` 的删除/二进制/>200KB/非 UTF-8 分支**：`dispatch.rs:1541` 只测了修改+新增，`--- /dev/null`、`Binary files` 与 200KB 边界没钉住。
3. **`parseAcceptance` 边界**："passed" 误判（2.1）、PASS 与 NEEDS_FIX 同现、判定词落在 1200 字符截断线之外。

另：REPORT 第 13 行「按任务书指定 fallback 复跑」不准确——TASK-04 里没有指定任何 vitest fallback，属报告措辞失实（验证本身有效）。

## 6. 真跑

`live_economy_pipeline`（`dispatch.rs:1904-2017`）构造正确：模型按第 0 节第 4 条（`glm-5.3-flash`/`gpt-5.6-luna`/`kimi-code/k3-256k` 完整别名）、结构上恰 3 次请求、断言 diff 非空、盲目录恰三件套、pytest 通过（status=ok 蕴含）才进验收、`.dispatch` 恰 3 份记录 + STATE.md。会导致误判的点：
- **它验证的是后端三个 `dispatch_stage_core` 调用，不是前端自动推进循环**——编排者不要把它的通过当作「图上按钮跑通流水线」的证据（REPORT 有披露，但容易忽略）。
- pi 真身校验：zai 若把 `glm-5.3-flash` 再静默升级，`identity_mismatch` 会让测试失败——这是按设计失败，不是环境故障，别当 bug 修。
- 无替补分支、无 `user_data_root_is_writable` 降级：任何一次网络抖动都直接红；判实现问题前先重跑一次。
- `python -m pytest` 需在 PATH（经 `powershell.exe -Command` 调用），缺 pytest 会以「验证命令失败」红掉——先确认环境。
- 失败路径不清理临时项目目录（成功路径才 `remove_dir_all`），只会留垃圾不影响判定。

`live_cc_switch_route`（`dispatch.rs:2019-2067`）：env 未设置时打印后 `return`，`cargo test -- --ignored` 会显示 **ok——假通过**。编排者必须确认输出含 `CC_SWITCH_REQUESTS=1` 才算真跑；且 usage 正则仍只对 fixture 形状断言过（`channels` 测试），与真实 `cc_switch.py --usage` 输出的核对依旧欠着——这是遗留第 2 条的最后一环。

---

## 对照 TASK-04 是否完整实现？

**一句话：主干（0/A/B/C/D/E 的引擎与图上闭环、遗留 9 条中的 7 条）已完整实现且质量扎实，但有两处功能缺项、两处部分实现，外加两项按设计留给沙箱外验证的真跑。**

缺项列表：
1. G1 无法勾选把方案文本发给盲验收（A 节明文要求；1.1）。
2. prompt 模板「可在设置里看」未做（C 节；1.2）。
3. 用量表未按阶段合计（E 节；1.3）。
4. 遗留 8 部分满足：live harness prompt 未走生产 `serializeContext`（1.4）。
5. 遗留 2 最后一环（usage 与真实输出核对 + 1 次真跑）未验证——任务书允许留给编排者，REPORT 如实标注。
6. 验收 3 真机流水线未验证——同上，ignored 测试已备好且构造正确。
7. （正确性而非缺项）`parseAcceptance` 的 "passed" 假 PASS 风险建议合入前修掉（2.1）。