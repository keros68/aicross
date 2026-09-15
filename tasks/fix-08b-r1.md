# TASK-08B 修正轮 R1

先读 `tasks/TASK-08-swarm.md`（契约 + B 节）、`tasks/REPORT-08B.md`，再读 `tasks/review-08b-ds.md`（与 `tasks/review-08b-glm.md`，若存在）。逐条处理，每条在 `tasks/REPORT-08B.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不动 `src/`（A 侧同步在做 `Node.swarm` 镜像、`builtin-swarm`、两条命令与 split.txt）；保持文件原有换行；不起窗口、不 commit、不 push。

## 必修（按严重度）

1. **G1 闸门窗口期阻断**（DS 2）：`swarmCheck` 为 `null`（核对未返回）时「确认派发」禁用并显示「正在核对文件范围…」；核对失败（命令 reject）按「无法核对」提示并允许用户选择继续。补测试。
2. **生成执行节点幂等 + 上限**（DS 3）：同一拆分节点再次点「按清单生成」时，已存在同件号的 draft 节点不重复建（提示「已生成」）；`swarm_limit` 在生成时也生效。补测试。
3. **合并动作幂等、逐件结果落图、失败分级**（DS 4）：合并进行中禁用按钮；逐件结果写回对应节点 Reply（`merged` → adopted + 正文附 commit；`conflict`/`test_failed` → 正文附 note，不 adopted）；只有「命令 reject/全部 skipped 且 note 说明引擎未接通」才显示「引擎未接通」，部分并入时逐件如实显示。补测试。
4. **蜂群卡片的动作约束**（DS 5）：swarm 节点不显示「选它」；「采纳」只对 `merged` 件可用；已 merged 的件不出「重派」（提示「已并入」）。补测试。
5. **重叠时可就地改文件范围**（DS 6）：G1 的重叠件路由卡内联一个 `files` 编辑框（每行一条），改完自动重核对；提示写明「改到不重叠或合并成一件」。补测试（纯函数：编辑后重核对入参）。
6. **盖预设菜单包含蜂群**（DS 7）：`GraphTab.tsx` 的两处预设列表都套 `withSwarmPreset`；补测试。
7. **拆分提示词与上限**（DS 8）：前端注入的拆分要求文本用 `swarm_limit` 拼「最多 N 件」；`roleDefaults('拆分')` 显式分支。A 侧 split.txt 落地后前端这段文本删除（报告写明位置）。
8. **每件只带自己的清单条目**（DS 9）：生成的执行节点 `user.text` 只含本件标题/文件范围/验收命令，不含整份清单；上下文照旧只带根材料。补测试。
9. **`Node.swarm` 持久化说明**（DS 1）：A 侧镜像并入前会被 save_graph 丢弃——在报告「合并对账」里写明依赖 A 侧 `GraphNode.swarm` 镜像；前端加载旧图缺字段时按无 swarm 处理（已是）。

## GLM 补充

GLM（`tasks/review-08b-glm.md`）判 B1–B6 完整实现；其第 1、2、4、5、6 条与上面第 3/2/4/1/8 条同题。GLM 额外的：

10. **G3 合并入口从图状态派生**（GLM 3）：「合并」按钮与逐件结果不依赖当次 overlay 会话——按图上 swarm 节点的终态与 Reply 里已记的合并结果推导，重开结果屏、单件重派后清单仍按当前图算；补测试（纯函数 `swarmMergeEntry(graph, stage)`）。合并请求键名按 camelCase（A 侧 R1 已定为 camelCase），去掉不必要的双写兼容或保留但注明。
