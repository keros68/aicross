# fix-12-r1 修正轮上限 / 同阶段并行 / 审查材料（TASK-12）复审修正轮

依据 GLM-5.3-Flash 与 DeepSeek 对 phase/12 的盲审（`tasks/review-12-glm.md`、`tasks/review-12-ds.md`）。编排者已定的取舍写在各条里。契约只追加。

## 必修（两家共同：2/5/6；DeepSeek 独有：1/3/4/7/8/9；GLM 独有：10/11/12）

1. **并行验证路失败吞掉验收修正轮（DeepSeek 1）**：`reducePipelineStep` 里「有非 ok 结果就停」先于验收分支。修法：先判「有验收 NEEDS_FIX 且未到上限 → 生成修正轮」，验证路的 error/timeout 只标该路失败、不阻断修正轮；验证路失败可从节点卡「重派」。测试：验收 NEEDS_FIX + 验证 error 的混合批次 → 修正轮照常生成。
2. **attach 对未物化的哨兵 Ok 也生效（DeepSeek 2 / GLM 1）**：首次验证失败/取消时 `prepared` 是 `Ok(("(未发送)", …))`，attach 仍跑验证命令并往不存在的盲目录写。修法：物化结果做成显式枚举（Materialized / Unsent），Unsent 直接返回、不跑命令、`validation_attached=false`。测试：Unsent 分支不执行命令（用 marker 文件断言）。
3. **遗留节点成为死路父节点（DeepSeek 3）**：遗留节点无 reply，挂在它下面的节点永远不可派，且它不可删。修法：`latestLeaf` / 列末端跳过遗留节点（回退到来源验收节点）；遗留节点不显示「分叉 / 盖预设」，列输入框对聚焦遗留节点时把父节点换成来源验收节点；给遗留节点「删除」出口（`removeDraftNode` 同类）。测试：遗留节点为焦点时新建节点的父是来源验收节点；删除后图上无残留。
4. **VALIDATION.txt 对 claude / pi 盲审不可读（DeepSeek 4）**：claude 盲节点 `--tools ""`、pi 盲无产物时 `--no-tools`，读不到文件。修法：文本已截到 8 KB，直接内联进盲 prompt（与 `diff.patch < 30KB` 同一做法），文件照写作为记录。测试：盲 prompt 正文含验证输出片段。
5. **讲解页 / 手册说验收「跑验证命令」与引擎不符（DeepSeek 5 / GLM 5）**：同改 `src/orchestration.rs:396、418` 的注释；：验收是盲、只读、看产物给 PASS / NEEDS_FIX；验证命令只在执行节点跑，输出经 VALIDATION.txt 附给验证路。改 `ExplainOverlay.tsx` 与 `docs/MANUAL.md` 的两处说法。
6. **验证命令按验证路数并发跑 N 次（DeepSeek 6 / GLM 2，编排者裁决）**：同一 run、同一项目目录的验证命令只跑一次，结果缓存给同批所有带 `review_material_validation` 的节点复用（按 run_id + project 键）；评审模式因此也只多跑一次。手册写明「派盲审前先跑一次验证命令」。测试：同批两路验证节点，验证命令只执行一次（marker 计数）。
7. **run_one 集成点无端到端测试（DeepSeek 7）**：复用盲节点 run_one 夹具，断言派发后 `workdir/VALIDATION.txt` 存在、留痕 `validation_attached: true`、超时取节点超时的一半（注入断言）。
8. **角色默认与跳过名单的两处可见变化（DeepSeek 8）**：接受 (a) 盲审模式整份不填默认（三路都得在 G1 选）——写进手册；(b) 「验证」角色空目标沿用验收默认——写进手册与讲解页。
9. **设置页文案（DeepSeek 9）**：「修正轮上限」说明补一句「验证命令未通过的重派与修正轮共用此上限；验证未通过到顶时该验收标『已跳过』」。

10. **验证失败停链后同父的盲「验证」兄弟永远「待派」（GLM 3）**：`appendValidationRetry` 与 `closeWaitingAcceptance` 只处理「验收」，并行长出的「验证」兄弟保持 draft 且父节点 failed → 永不可派。修法：两条路径里把同父「验证」兄弟一并标 `skipped_reason: '上游验证未通过'`。测试：验证未通过到顶 → 验证兄弟标已跳过、不在可派集合、节点卡不显示「待派」。
11. **结果屏遗留卡带出历史遗留（GLM 4）**：`leftoverNodes(graph)` 取全图再按来源过滤，同一验收重派再到顶会出两张卡。修法：本轮新增的遗留 id 由 `reducePipelineStep` / `appendLeftover` 随步状态传出，结果屏只画本轮新增；图上同一验收下已有遗留时更新它而不是再挂一个。测试：同一验收两次到顶 → 图上只有一个遗留节点、结果屏一张卡。
12. **设置页「修正轮上限」清空即 0（GLM 7）**：与「总请求上限」同口径，空则回默认 1。

## 保持

- GLM 6：用 `orchestration.rs?raw` 切片断言预设形状的前端测试脆但能红，Rust 侧已有同事实测试；保留，改成解析后按 id 查找而不依赖声明顺序即可（顺手）。
- 契约四条、上限语义（limit 1 派 1 轮、0 首轮即遗留）、遗留节点排除派发、同阶段并行的预设形状、`VALIDATION.txt` 落在 runs 目录不进项目、脱敏与截断、提示词关注点来自文件——核过没问题，不动。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src/server.rs` CRLF；`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-12-R1.md`（逐条对应本清单）。
