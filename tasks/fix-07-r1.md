# TASK-07 修正轮 R1

先读 `AGENTS.md`、`tasks/TASK-07-resources-return.md`、`tasks/REPORT-07.md`，再读 `tasks/review-07-glm.md`（另一厂商盲审）。逐条处理，每条在 `tasks/REPORT-07.md` 末尾新增「修正轮 R1」一节里写「已修：怎么改的」或「不采纳：原因」。验证跑 `cargo check`、`cargo test`、`cd src-ui && npx vitest run`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不起窗口。不要 commit，不要 push。范围约束不变：不动 `src-ui/src/graph/`、`components/right/orchestration/`、`src/dispatch.rs`、`src/channels/` 的逻辑；`GraphTab.tsx` 的事件监听编排者已认可（纯追加），保留。

## 必修（编排者窗口实测）

1. **pi 探测只读到 5 个模型**：`~/.pi/agent/models-store.json` 是旧清单，与 `pi --list-models` 求交集把 deepseek、qwen-token-plan-cn 两个 provider 和 glm-5.3-flash 都丢了。改为以 `--list-models` 为主（它才是运行时已鉴权清单，本机 31 个、三个 provider）；models-store 只用来补 contextWindow 等元数据，store 里没有的模型照样收入。补测试：list-models 有、store 没有的模型必须出现在结果里。
2. **探测写盘后右栏不刷新**：接入页探测/冒烟/移除成功后已经派发 `aicross:resources-changed`，但右栏方案与编排的「尚未接入资源」提示与节点面板的模型下拉、`@` 弹层都没监听它。允许在 `components/right/orchestration/` 与 `components/center/graph/GraphTab.tsx` 里**只追加**一个监听（重新调 `list_dispatchable_models`），不改其它逻辑。

## 必修（GLM 审查）

3. **并发写 resources.json 无锁**（审查 2.4）：加进程级 `static Mutex` 包住每个命令的 load → 改 → save 序列；`smoke_agent_models` 改为每冒烟一条就重读、改、写一条，不整批长持旧 store。补测试：两条模型一成一败 → store 里一已接入一冒烟失败，errors 恰含一条。
4. **cc-switch 提示条被空 hints 覆盖**（审查 4）：冒烟命令不返回 hints 时前端保留旧值，或命令带回原 hints。
5. **claude 第三方端点判定**（审查 2.5）：除进程环境 `ANTHROPIC_BASE_URL` 外，读 `~/.claude/settings.json` 的 `env.ANTHROPIC_BASE_URL`；两处都没有时在模型 meta 记「未核对真身（官方端点）」而不是静默跳过。
6. **死接口与死文件**（审查 1、5）：给「全部探测」加按钮（接入页顶部），接 `detectAllResources`；三个 `pi_smoke_*.jsonl` fixture 要么被测试引用要么删除，REPORT 改口为「第 1 期 smoke 解析测试由第 3 期 channels 测试等价覆盖」。
7. **补测试**（审查 5）：申报 source 的消失条目不标「未在事实源中」；损坏的 resources.json 上 add/update 报错且文件字节不变；`load_or_default` 损坏不覆盖。
8. **报告口径**：前端「18 个用例」改为实际数字；「没做什么」补一条「新界面文案硬编码中文，未进 11 语言字典（与第 2/6 期口径一致，第 5 期统一处理）」。

## 次要（一并做）

9. `add_manual_agent` 不吞探测错误，返回给前端显示（审查 2.7）。
10. `resolve_cli` 注释记下与 channels `resolve_binary` 的已知漂移（kimi `.cmd` 探测能过、冒烟会拒），并把 kimi 的解析顺序改成先 `~/.kimi-code/bin/kimi.exe` 再 PATH，与 channels 一致（审查 2.6）。
11. claude 未检测到时也走合并（刷 missing 标记），与 codex/kimi 一致；或注释说明为何不同（审查 2.6 小问题）。
12. `t('…' as any)` 去掉 `as any`；`.resource-status.connected/.failed` 底色改用主题 token（审查 4）。
13. `write_json_atomic` 与 `orchestration.rs` 的重复：本期先在注释里互相指向，收敛留第 5 期（改 orchestration.rs 属禁区）。
14. 左栏点模型而无对话图 tab 时，模型行置灰并提示「先打开对话图」（审查 1）。

## 不采纳

- 移除手动 agent 的途径：本期需求外，记 Later。
- 三处中文状态字符串并存（Rust / TS / i18n）：第 1 期遗产，契约不改。
