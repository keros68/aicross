# TASK-08B 修正轮 R1 复审

逐条核对（编号按 `tasks/fix-08b-r1.md`）：

1. **G1 窗口期阻断 — 已核实修好**。三态闸门 `swarm.ts:234-264`：pending 一律 blocked、error 默认 blocked 且给「仍要派发」、确认按钮 disabled 链里有 `swarmGateState.blocked`（`DispatchOverlay.tsx:632`）；测试 R1-1 两条分别钉纯函数与组件渲染。idle（无蜂群件/无项目目录）不拦，符合原口径。
2. **生成幂等 + 上限 — 已核实修好**。`swarm.ts:154-212` 跳过已有件号、按「已有 + 新建 ≤ limit」截断、一个没建时返回原图引用；`DispatchOverlay.tsx:414` 把 `settings.swarmLimit` 传进生成；R1-2 钉住。
3. **合并幂等/逐件落图/失败分级 — 已核实修好**。`swarm.ts:319-321` adopted 件剔出请求，`DispatchOverlay.tsx:427-428`、`:729` merging 与空集双禁用；`swarm.ts:425-442` 逐件 note 追加 Reply（同行去重）、仅 merged 标 adopted、引擎未接通批一字不写；`swarm.ts:355-361`/`:399-416` 只有 IPC 缺失（或空错误文本）才说「未接通」，其余按原文；R1-3 三条钉住。
4. **卡片动作约束 — 已核实修好**。`swarm.ts:463-472`：蜂群件无「选它」、采纳仅 merged/已 adopted、已并入无重派并给「已并入」；「升级」一并关掉有写明且合理（同一条 retry 路径重开 worktree 会重复应用）。
5. **重叠就地改范围 — 已核实修好**。`DispatchOverlay.tsx:541-547` 重叠卡内联 SwarmEditor，写回 `updateNodeSwarm` 后 graph 变化重跑 `:217-229` 的核对 effect；文案与重核对入参被 R1-5 钉住。
6. **盖预设菜单两处 — 已核实修好**。`GraphTab.tsx:109` 与 `:160` 都套 `withSwarmPreset`，R1-6 断言恰好两处且无裸 `setPresets(f.presets)`；右栏 `OrchestrationPanel.tsx:53/:75` 本就有。
7. **拆分提示词与上限 — 已核实修好**。`stamp.ts:9-19`/`:87` 动态拼「最多 N 件」，两个盖章入口（`GraphTab.tsx:370`、`OrchestrationPanel.tsx:140`）都传设置；`types.ts:135` 显式分支；split.txt 落地后的删除位置已写明（报告 389-392 行）。
8. **每件只带自己那条 — 已核实修好**。`swarm.ts:188-189` 指定上游 = 根；`context.ts:125` 指定上游只保留 upstreams 的 reply 段，整份清单进不来；R1-8 用 serializeContext 钉住。拆分模板消息仍进上下文，报告已声明，可接受。副作用：`dispatch.ts:135` 只对「共享」续聊，各件自然独立会话（GLM 2-6 随之消解，成立）。
9. **`Node.swarm` 持久化说明 — 已核实修好**（该条要求就是写说明+钉测试）。报告 412-418 行写明依赖 A 侧镜像及丢失后果；R1-9 钉住旧图缺字段按无蜂群处理。
10. **G3 合并入口从图状态派生（GLM 3）— 未修**。入口仍从当次 `results` 派生（`DispatchOverlay.tsx:422`）：重开结果屏 `results=[]` 整个合并区消失（`:720`），单件重派后合并清单缩水。报告 447-451 行的不采纳理由（runId 留痕归属、「按批还是按拆分」需与 A 侧对齐）有实质，但这是 fix 清单明列的第 10 条，本轮未闭合。camelCase 半边按「保留但注明」处理（`tauri.ts:560-561` 双写 + 注释），那半边算完成。

**新缺陷检查**：既有路径（擂台/仲裁/备选/回滚/心跳/对账）无回归——`arbitrationCandidates` 与 `swarmCardActions` 对非蜂群节点行为不变，`stamp` 新参可选，G1 闸门对非蜂群批恒 idle。两个小瑕疵，均非阻断：(a) 蜂群批内任何图提交（含改备选 `DispatchOverlay.tsx:592`）都会把核对打回 pending、确认按钮短暂禁用——正确性无碍，与被不采纳的防抖同源；(b) 跨拆分节点件号仍可重复（第 2 条按字面只管同一拆分），两批同跑时合并请求 item 撞号、排序交错，一期图里两个拆分属低概率。

---

**结论：R1 之后 TASK-08B（B 侧界面）可交付**——10 条里 9 条修好且有测试钉住，第 10 条未闭合但不阻塞本期交付。

仍未闭合的项（按严重度）：
1. 合并入口绑定单次 overlay 会话（fix 第 10 条）：重开结果屏合并区消失、单件重派后清单缩水——图上数据（adopted、Reply note）不丢，但冲突件可能在重开后无处重并；留待与 A 侧定「按批还是按拆分」。
2. 跨拆分节点件号可重复，同批合并请求 item 撞号（低概率、低危害）。
3. 蜂群批内核对无防抖，任何图变更都短暂禁用确认按钮（已声明留到下一轮，纯体验）。
4. （跨树前提，非 B 侧缺陷）A 侧 `GraphNode.swarm` 镜像并入前，每次 save_graph 落盘都会抹掉 swarm 字段——已写进报告合并对账，A-1 并入时须一并核对。