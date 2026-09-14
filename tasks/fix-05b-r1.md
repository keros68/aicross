# TASK-05B 修正轮 R1（第 2 期预设）

先读 `tasks/TASK-05-acp-and-polish.md` 第 0 节第 1 条与 B 节、`tasks/REPORT-05B.md`，再读 `tasks/review-05b-ds.md`（DeepSeek 盲审）与 `tasks/review-05b-glm.md`（GLM 盲审，若存在）。逐条处理，每条在 `tasks/REPORT-05B.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不起窗口、不发真机请求、不 commit、不 push。不动 `src/channels/`（05A 在另一工作树改它）。

## 必修（按严重度）

1. **备选重跑不得对可写节点重复写项目**（DS 2.2、2.3、3.3）：`write != 只读` 的节点默认不触发备选（G1 显示「可写节点不自动切备选」）；只读/盲节点才走备选重跑。worktree 节点若将来允许备选，必须复用已有 worktree 或先 remove 再建，不得静默降级成直接写项目。补测试：单写者主失败 → 不重跑、结果标「主失败，未切备选（可写节点）」；盲节点主失败 → 备选重跑成功。
2. **`pickOne` 误清同父不同角色节点**（DS 2.1）：清候选时要求 `kind`、`target.role` 与被选节点一致（或按 `recordStamp` 批次号）。先写失败用例再修：根下擂台候选 + `/advise` 顾问节点并存时，选它不动顾问的 adopted。
3. **擂台「2–4 家」可达**（DS 1.1）：盖 `builtin-arena` 时跳过 `applyRoleDefaults`（三路留空，G1 逐路选）；图界面给 draft 节点加「删除」（节点面板与右键/卡片按钮任一即可，只允许删没有子节点的 draft）；补 stamp 测试与删除的纯函数测试。做不到删除就把口径「固定 3 家」写进报告并在预设说明里注明。
4. **评审团「自定义」视角与 `user.text` 编辑入口**（DS 1.2）：节点面板加 `user.text` 编辑框（draft 节点可改），`builtin-jury` 第三个节点视角文本留空提示「自定义视角」；仲裁中性描述建节点后也可在面板改。补测试。
5. **前提抽查带附件**（DS 1.4）：`Premise` 加 `attachments: string[]`，设置页可勾选文件，`buildPremiseCheckGraph` 塞进 `root.user.attachments`；补 Rust 往返测试与 TS 建图测试。
6. **备选可在节点上设置并持久化**（DS 1.5）：`TargetEditor` 加一行备选下拉（写回 `target.fallback`，`cloneTarget` 已深拷贝），G1 的临时选择也写回节点。补测试。
7. **仲裁节点走盲语义**（DS 1.3、3.4）：`appendArbitration` 建的节点 `visibility=盲`，隔离目录只拷共同材料祖先的用户消息与附件（走现有盲分支），G1 显示本次外发清单；中性描述用「甲方/乙方」标签、不写 agent/model（DS 3.2），截断加省略号且优先保留含转折词的句子。补测试：描述不含双方 agent/model 名与「更好/更差/同意/反对/应该」词表。
8. **前提结论回写按 id**（DS 2.6）：台账加 `last_graph_id`，回写按 id 读图；`last_checked_at` 用回写时刻；把 `SettingsModal` 里的匹配逻辑抽成纯函数 `matchPremiseResults` 并测（两次抽查同名图、结论为空、未变）。
9. **备选留痕与文案**（DS 2.4、2.5、1.7、4.4）：备选那条留痕的 `billing_source`/`vendor` 用备选目标的；报告把「同 prompt」改成「换通道不续旧会话，发完整 prompt」；节点卡显示「已切备选：主 x → 备 y」整行；报告点明「已切备选」标记行会进下游上下文与图落盘的代价。
10. **角色清单**（DS 4.5、4.6、4.7）：`ROLES` 加「仲裁」；`roleDefaults` 补 `顾问→盲/只读`、`仲裁→盲/只读`；`parseAdvise` 用 `/^\/advise(?:\s|$)/`；台账写回失败要提示，不吞。
11. **测试补强**（DS 5.1）：`第三家厂商放行` 用例把 `k3` 放进 `models`；中性描述用例改为断言词表与标签；补 G1 把 `fallback` 下发到请求的测试；补 `appendArbitration` 无共同祖先分支；`live_arena_and_arbitration` 改成真实仲裁形态（走 `appendArbitration` 建节点、断言第三方厂商与两边不同、`enforce_live_success` 不得跳过仲裁段）。
12. **弱键与文案**（DS 2.8、1.6）：`PRESET_NODE_TEXTS` 取值要求 `preset.builtin`；结果屏标题改「第三方仲裁 · 第 N 阶段」并注明「系统不判定分歧，双方由用户选」；报告计数改为 11 条 vitest，「增删路数走图界面」改口。

## GLM 补充

GLM 与 DeepSeek 在第 1、2（部分）、3、4、5、6、7、9 条上结论一致。GLM 额外的：

13. `appendArbitration` 共同材料祖先 ≥2 时 `kind` 要按 parents 数定（≥2 → `accept`），别靠 `normalizeNodeKinds` 静默改（GLM 2.2）。
14. 「选它」按钮只在**同父**结果 ≥2 时显示，不是同阶段 ≥2（GLM 2.3）。
15. 前提回读：比较 `outcome.slice(0,500)` 而不是全文，避免每次打开设置页重写台账；建图时不写 `last_checked_at`，回写结论时才写（GLM 2.6，与第 8 条一起做）。
16. `live_arena_and_arbitration` 的 dispute 文本按 `node_id` 映射取正文，不能假设 `results[0]` 是 pi（GLM 5.2）；仲裁标签改甲方/乙方后 live 测试同步。
17. 备选重跑的调度记账：`active_agents` 按备选 agent 也串行（GLM 2.4c）；worktree 降级单写者给结果一个显式降级标记供界面显示（若第 1 条把可写节点排除在备选之外，这条只需在报告里说明）。
18. 报告：Rust 新测试是 7 条不是 8 条，按实数写。
19. 补 GLM 5.3 的三条测试：擂台盖章后目标为空；`appendArbitration` 产物过 `serializeContext` 只含根材料与中性描述、不含双方 reply；`fallbackFrom` 结果过 `reducePipelineStep` 修正轮照常生长。
