# TASK-05B 修正轮 R2（第 2 期预设，复审 R1 的收尾）

先读 `tasks/fix-05b-r1.md`、`tasks/REPORT-05B.md` 的「修正轮 R1」一节，再读 `tasks/review-05b-r1-ds.md`（DeepSeek 复审）与 `tasks/review-05b-r1-glm.md`（GLM 复审，若存在）。两家对 R1 的 19 条基本核实修好，本轮只收尾巴。每条在 `tasks/REPORT-05B.md` 末尾「修正轮 R2」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不跑 `cargo fmt`、不整文件重排、保持文件原有换行；不动 `src/channels/`；不起窗口、不发真机请求、不 commit、不 push。编排者已真跑 `live_arena_and_arbitration`（1 passed，61s）。

## 必修（按严重度）

1. **备选重跑后的 session 归属**（DS 2）：`run_fallback` 把备选的新 `session_id` 写回 Reply，而节点 target 仍是主目标；下游同目标「共享」子节点经 `continuationSession` 会把备选 session 交给主通道，主通道 `-S/-r` 一个不存在的会话就报错。改法：Reply 的 session 只在「实际作答的通道 == 节点 target」时写；备选作答时 session 置空并在 `runner_note`/结果里记「已切备选，不续聊」，下游节点发完整 prompt。补测试：TS `continuationSession` 对带 `fallbackFrom` 的父回复返回空；Rust 备选重跑结果 `session_id` 为空。
2. **前提附件改文件选择器**（DS 1）：设置页每条前提的附件用 `@tauri-apps/plugin-dialog` 的 `open({multiple:true})`（仓库已有用法，`CenterPanel.tsx` 里搜 `open(`）选文件，存绝对路径；保留手输入口也可，但保存前校验：相对路径拒绝并提示、同名 basename 提示「附件文件名重复」。补测试：校验纯函数（相对路径、重名）两条。
3. **主/备两次尝试的原始输出转存互相覆盖**（DS 5）：`record_stdout` 写 `run_root/<node_id>.raw.txt`，备选会覆盖主尝试的；改成按 attempt 后缀（`<node_id>.raw.txt` / `<node_id>.fallback.raw.txt`）或带留痕文件名。补测试：主备两条留痕各自的 raw 转存路径不同且都存在。
4. **备选留痕的 vendor 不猜**（DS 6）：前端下发 `fallback` 时带 `vendor`（`DispatchNodeRequest.fallback` 追加可选 `vendor`，两侧镜像 `#[serde(default)]`），Rust 优先用下发值，没有再 `infer_vendor`。补测试。
5. **组件级断言补三处**（DS 3）：`DispatchOverlay` 的「可写节点不自动切备选」提示、「主失败，未切备选」结果行、「选它」按钮可见性，用现有 vitest 环境（若已有 `@testing-library/react` 或 `react-dom/server` 的 `renderToStaticMarkup` 就用；没有就把这三段抽成返回 JSX/字符串的纯渲染函数再断言）。三条测试。
6. **live 测试的同义反复断言**（DS 4）：`assert_ne!(arb.vendor, first_vendor)` 和 `assert_eq!(arb.write, "只读")` 由 helper 写死；改成从真实结果/留痕 frontmatter 读回 `vendor` 与 `write` 再断言；live 的中性描述改用产品同一个 `disputeExcerpt` 口径（Rust 侧没有就按同规则：200 字 + 省略号）。保持 `#[ignore]`。
7. **仲裁只读进 Rust 强制**（DS 8）：`dispatch.rs` 只读强制的角色集合加「仲裁」，与前端 `validateNode` 一致；补一条测试。
8. **文案**（DS 7）：仲裁编排区标题按报告口径「第三方仲裁 · 第 N 阶段」（N = 阶段序号），说明行不变。
9. **报告降级表述**（DS 1/7/11/16）：R1 报告里把「备选重跑成功」「不含立场词」「G1 把 fallback 下发到请求」三处按实际断言改写（离线只能断言重跑发生；词表断言只对短 fixture 成立；下发只有纯函数级），别写成它没验到的东西。

## GLM 补充

GLM 复审（`tasks/review-05b-r1-glm.md`）判 19 条全部闭合、可交付；GLM 额外的三处瑕疵：

10. **「选它」可见性按 kind/role 收窄**（GLM a）：`sameParentsResultCount` 只数同父且 `kind`、`target.role` 与本节点一致的结果，与 `pickOne` 口径对齐；`R1-14` 加一条「同父 1 擂台 + 1 顾问不出选它」。
11. **`fallbackSkipped` 进留痕 frontmatter**（GLM b）：主失败未切备选时，留痕加 `fallback_skipped: 可写节点`（追加字段，旧记录不受影响），Reply 不加字段；补往返测试。
12. **G1 备选 onChange 只写 fallback**（GLM c）：`DispatchOverlay` 里备选下拉写回时只 patch `target.fallback`，不把本地改过的主目标一起落盘；补一条纯函数测试。
