# TASK-08 蜂群（第 3 期，按 CAID）

依据 `docs/PRODUCT.md`「第 3 期草案：蜂群」的已定口径。两块并行、各自工作树；契约先定在本文件。

## 契约（只追加）

- `Node` 追加可选 `swarm?: { item: number, files: string[], accept: string }`（TS）/ Rust `Node` 镜像 `#[serde(default)] Option<SwarmItem>`。拆分节点角色名 `拆分`，合并阶段不是节点，是结果屏动作。
- 拆分回复的结构化输出：正文里一个 ```json 围栏，形如 `{ "items": [ { "title": "…", "files": ["src/a/**", "docs/x.md"], "accept": "python -m pytest tests/test_a.py" } ] }`；解析失败按普通回复展示，G3 给「按清单生成执行节点」按钮时提示「未解析到清单」。上限 4 件（设置项 `swarm_limit`，默认 4，与并发上限同一处）。
- Tauri 命令 `check_swarm_plan(project_dir: String, items: [{ item, files }]) -> { dirty: bool, overlaps: [{ a: number, b: number, path: string }], unmatched: [{ item, glob }] }`：只读；`dirty` = `git status --porcelain` 非空；`overlaps` = 两条 glob 匹配到同一个现存文件（列出第一个撞上的路径）；`unmatched` = glob 在项目里一个文件都不匹配（提示，不阻断）。
- Tauri 命令 `merge_swarm_items(project_dir: String, run_id: String, items: [{ node_id, item, worktree, accept }], validate_command: String) -> [{ node_id, item, result: "merged" | "conflict" | "test_failed" | "skipped", commit: string, note: string, test_output: string }]`：项目必须干净（否则整批 `skipped` + note）；按 `item` 顺序：worktree 内 `git add -A && git commit -m "aicross swarm item N: <title>"`（无改动 → `skipped`）→ 项目 `git cherry-pick <sha>` → 冲突 → `cherry-pick --abort`、`conflict`、note 含 worktree 路径 → 成功后跑 `accept`（空则用 `validate_command`，都空则不跑）→ 非零 → `git reset --hard HEAD~1`、`test_failed`、`test_output` 截 8KB → 否则 `merged` + commit sha。每件写一条留痕 `aicross-dispatch/1`（追加字段 `swarm_item: number`、`merge_result: string`）。
- 冻结模板 `fixtures/prompts/split.txt`：拆分提示词（只读、输出清单 JSON、每件文件范围不重叠、有先后依赖的活合成一件、不超过 N 件）。

## A. 引擎（Rust）— codex

1. `Node.swarm` 镜像 + 留痕两个追加字段 + 往返测试。
2. `check_swarm_plan`：glob 用现有依赖或 `globset`（若需新增 crate 在报告里写明）；遍历项目文件沿用快照的排除目录规则；测试：重叠/不重叠/不匹配/脏树。
3. `merge_swarm_items`：按契约实现，全程走 `run_process_registered`，取项目写锁；每步失败都返回可读 note，不 panic；测试用临时 git 仓库 + 两个 worktree：① 两件顺序合并成功且各一个 commit；② 第二件冲突 → abort、第一件仍在；③ 验证命令失败 → reset 回第一件；④ 脏树 → 全 skipped。
4. `fixtures/prompts/split.txt` + 角色默认：`fill_role_defaults` 对 `拆分` 用「出方案」的默认目标、`write=只读`、`visibility=共享`。
5. `server.rs` 注册两条命令。
6. **内置预设 `builtin-swarm`（Rust 侧）**：`orchestration.rs` 内置清单加一条「蜂群模式」——一个节点：role `拆分`、`write=只读`、`visibility=共享`、目标留空由 `fill_role_defaults` 按「出方案」填；内置总数 5→6，既有断言同步；`PRESET_NODE_TEXTS` 的前端文本由 B 侧配。补测试。

验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。不跑 `cargo fmt`、保持文件原有换行（`server.rs` CRLF、其余 Rust LF）；不动 `src/channels/`；不起窗口、不发真机请求、不 commit、不 push。报告写 `tasks/REPORT-08A.md`。

## B. 界面（TS）— Claude 子代理

1. `types.ts` `Node.swarm?`；`tauri.ts` 两条命令与类型按契约先写（后端未接通时按 `dirty:false, overlaps:[]` / 全 `skipped` 回退）。
2. 内置预设 `builtin-swarm`（界面名「蜂群模式」）：盖在根材料上长出一个 `拆分` 节点（只读、共享、目标按「出方案」角色默认）；`ROLES` 加「拆分」；`roleDefaults` 补 `拆分→共享/只读`。
3. 解析：纯函数 `parseSwarmPlan(text) -> items | null`（取最后一个 ```json 围栏，校验字段，超 `swarm_limit` 截断并提示）；拆分节点完成后结果屏出「按清单生成执行节点」：为每件建 draft 节点（role `执行`、`write=worktree`、目标按执行角色默认、`user.text` = 标题 + 文件范围 + 验收命令、`swarm` 字段）；测试。
4. G1：本批含 `swarm` 节点时调 `checkSwarmPlan`，重叠对标红并禁用「确认派发」直到用户改 `files`（NodePanel 给 `swarm.files`/`accept` 编辑框）；`dirty` 提示「项目目录有未提交改动，蜂群合并要求干净」并禁用；`unmatched` 只提示。测试纯函数 `swarmGate(check) -> { blocked: boolean, reasons: string[] }`。
5. G3：当本阶段全部 `swarm` 节点终态后出「合并」按钮 → `mergeSwarmItems` → 逐件结果行（已并入 + commit / 冲突待裁决 + worktree 路径 / 测试红 + 输出 / 跳过）；`merged` 的节点自动 `adopted`；冲突与测试红不自动重派。测试：结果行渲染与 adopted 写回的纯函数。
6. `docs/MANUAL.md` 加「蜂群模式」一节（怎么盖、清单长什么样、合并规则、冲突怎么办）。

验证同 A（vitest 用 `--configLoader native --pool threads`）。不动 `src/`；不起窗口、不 commit、不 push。报告写 `tasks/REPORT-08B.md`。
