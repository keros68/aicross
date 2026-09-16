# fix-15-r1 控制台启动器（TASK-15）复审修正轮 + 图标换 coffee 那套

依据 GLM-5.3-Flash 与 DeepSeek 对 ui/phase15 的盲审（`tasks/review-15-glm.md`、`tasks/review-15-ds.md`）与用户 09-15 晚的追加要求（`docs/PRODUCT.md`「用户 09-15 晚四点反馈的定案」第 2 条）。契约只追加。

## 必修（两家共同：4/5/6；DeepSeek 独有：1/2/3/7/8/9；GLM 独有：10）

1. **自动直启用默认里记的目录并切走工作区（DeepSeek 1，高）**：自动直启的 cwd 顺序改成「当前工作区优先，默认目录只在没有工作区时用」（`activeSession.folderPath || commanderDefaults.cwd`），与启动器同序；不再因默认目录反向 `SET_FOLDER`。测试：工作区 A、默认 B → 会话起在 A 且左栏仍是 A。
2. **默认值晚到把用户正在用的启动器顶掉（DeepSeek 2）**：启动器一渲染就登记该 tab「用户已看到启动器」，之后默认/探测变化不再自动接管该 tab；「+ → 控制台」的判定移到建 tab 处（有默认直接建带工具的 tab，不先画一帧启动器、不白跑一次模型清单）。测试：状态机纯函数——探测 false→true 只产生一次启动；另一 tab 写了默认不接管已显示启动器的 tab；点更改后不再启动。
3. **细行目录段不随换目录更新（DeepSeek 3）**：细行的 agent/模型固定，目录段读 `t.folderPath`；`RESTART_TERMINAL` 换目录后 tab 标题同步。
4. **G1 路由卡 / 结果卡 / 节点面板仍画机器 id（DeepSeek 4，编排者裁决：只改渲染层）**：`agent/model` 标识串不动（进裁决上下文与测试），渲染处经 `agentDisplayName()` 显示「Claude Code · fable」这类；同类处 NodePanel、OrchestrationPanel、PresetBuilder 一并。
5. **核心接线无测试（DeepSeek 5）**：自动直启判定抽成可测的状态机（tab id → 已启动 / 已看过启动器 / 已更改），reducer 级测试钉住三条路径；删掉无生产调用的 `commanderLauncherToolPatch` 或让 CenterPanel 真用它；REPORT 的红绿描述改成对应真实回退点。
6. **两端同源（DeepSeek 6）**：vitest 用 `node:fs` 读 `src/resources.rs` 抽出四个 `AGENT_NAME_*` 与前端表比对（仿 CSS 断言先例）。
7. **图标换 coffee 那套（用户要求）**：四家 agent 的标志统一用 fork 里上游自带的 `src-ui/public/icons/tools/{claude,codex,kimicode,pi}.svg`（pi 用 `icons-inline/pi.svg` 同一图）——启动器卡、左栏资源、接入页、启动卡、历史行全部同一来源；`brand-marks.ts` 的 simple-icons 路径与 `glyph` 分支删掉，`NOTICE` 里 simple-icons 与 OpenAI 撤除条目相应改为「沿用上游 Coffee CLI 的工具图标」（保留上游 NOTICE 原文）。测试：`AgentBadge` 对四家渲染 `<img src="icons/tools/…svg">`（或内联同一文件），无 simple-icons 路径残留。
8. **网格测试与文档（DeepSeek 7、8）**：补 `.is-picked` 不改宽度的断言；`docs/PRODUCT.md:234/243` 同步（π 字形 → coffee 图标；四个显示名）；`README.md`、`AGENTS.md` 简介里的「Kimi Code CLI / pi」改；`docs/proto/index.html` 文件头注明历史快照。
9. **小项（DeepSeek 9）**：pi 的细行/标题带 provider；删死条件与无生产者分支；开应用即进会话这一行为写进手册与报告。

10. **GLM 补充**：`src/resources.rs:940` 还有一处裸 `"Kimi Code"` 字面量，改引常量；冷启动首个 tab 也会自动直启（有默认时首屏直接进会话、看不到一键行）——这正是用户要的，写进手册与报告并在启动器保留「更改」回路；`src-ui/src/styles/global.css` 若本期没有必要改动就还原。

## 保持

- 一次性集合挡住探测返回后的二次启动、「更改」不被默认顶回、显示名 Rust 侧一处定义与旧库迁移、commander.json 读写与消费快照逻辑——核过没问题，不动。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；改名后跑 `scripts/copy-audit.py`。保持文件原有换行（`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-15-R1.md`（逐条对应本清单）。
