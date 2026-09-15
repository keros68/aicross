# TASK-08C 蜂群联调收口（08A/08B 并入 main 后）

依据 `tasks/TASK-08-swarm.md`、`tasks/REPORT-08A.md`「修正轮 R1/R2」与 `tasks/REPORT-08B.md`「修正轮 R1」的对账段，以及 `tasks/review-08b-r1-{ds,glm}.md` 未闭合项。在 main（已含 8a、8b、09）之上做，前端为主；Rust 只允许改 `src/orchestration.rs` 里 `builtin-swarm` 预设与 `preset_text` 相关行。

1. **拆分提示词单一来源**：前端盖章消费 Rust 预设目标上的 `preset_text`（`Target.preset_text`，空串 = 无）；`stamp.ts` 里为 `builtin-swarm` 注入的 `swarmSplitText(limit)` 及其两处调用点删除；「最多 N 件」由 Rust 侧 split.txt 模板占位或前端在 `preset_text` 后追加一行「本次上限 N 件」（二选一，报告写明）。`withSwarmPreset` 只在引擎清单里没有 `builtin-swarm` 时兜底。补测试。
2. **合并入口按图派生**（08B-R1 第 10 条）：纯函数 `swarmMergeEntry(graph, stage)` 从图上 swarm 节点的终态、Reply 里已记的合并结果推导「可并件 / 已并件 / 待裁决件」，结果屏与重开结果屏都按它渲染；单件重派后清单按当前图算。补测试。
3. **跨拆分节点件号续号**：同图内第二个拆分节点生成的件从已有最大件号 +1 起编，G1 文案与合并排序不撞号。补测试。
4. **G1 首帧与空目录**：`projectDir` 为空时核对不发但闸门显示「未选工作目录」并阻断；首帧（effect 前）按 pending 处理，不留可点窗口。补测试。
5. **合并请求只发 camelCase**（A 侧已定），删掉双写与 `SWARM_CHECK_FALLBACK` 死常量、过期注释。
6. **已并入件的保护**：已 `merged` 的件不可「取消采纳」，兄弟卡 `pickOne` 不清它的 adopted。补测试。
7. **每件上下文只带根材料**：生成的执行节点上下文不带拆分节点的用户消息（拆分要求）——`upstreams=[root]` 已做，核对 `context.ts` 的指定上游分支不会把拆分节点的 user 消息带进去，带了就改。补测试。
8. **前端只读校验集合加「拆分」**（GLM 交接项）：`validate.ts` 的只读角色集合与 `ROLES`、Rust 护栏同口径。补测试。

验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行；不起窗口、不发真机请求、不 commit、不 push。报告写 `tasks/REPORT-08C.md`。
