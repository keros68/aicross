# TASK-11 界面文案与命名统一 · 审查报告

**总判：可交付（有 1 个未闭合的 Rust 侧漏网词与几处低severity不一致；无回归）。** 注意：审查期间工作区仍在变动（编排者中途合入了 review-11-ds 修复轮，`dispatch.rs` 两处「留痕」错误串、`GraphTab`/`OrchestrationPanel` 的 `presetDisplayName` 接入、MANUAL 术语表两行都是在本次审查进行中被改掉的），以下结论基于会话内最后一次读取的状态。

## 问题（按严重度）

1. **[中低] `src/resources.rs:1294,1301,1305,1310` — 手动接入模型的 Rust 错误串仍叫「手动输入失败」**
   旧/现：`Err("手动输入失败：模型 ID 不能为空")`、`"手动输入失败：未知 agent：{…}"`、`"手动输入失败：pi 模型必须填写 provider"`、`"手动输入失败：该模型已存在"`。
   复现：前端同一条流程已改为「手动接入模型」（`ResourcesOverlay.tsx:486,538`），错误包装为「接入模型失败：」（`ResourcesOverlay.tsx:532`）；触达 Rust 校验（如模型已存在）时用户看到「接入模型失败：手动输入失败：该模型已存在」，两个动作词叠用。
   建议：四处改「手动接入失败：…」，与 agent 接入路径（`resources.rs:2153`）用词对齐。报告 Rust 盘点段未覆盖这四处。

2. **[低] 「验证失败」一词两义，与术语表的唯一用词冲突**
   `src-ui/src/graph/dispatch.ts:356`：节点 `validation_failed` 的状态显示为「验证失败」；`src-ui/src/resources/types.ts:115`：模型真身核对失败也叫「验证失败」。同一串字在结果屏与资源列表指两件不同的事。相关：「验证命令」（`SettingsModal.tsx:664`、`dispatch.ts:348,613,618`「验证命令未通过」、`MANUAL.md:96`）与蜂群「验收命令」（`MANUAL.md:75`）两个命令词并存，术语表均未收录。
   复现：接入页红字「验证失败：…」（真身核对）vs 结果卡状态「验证失败」（validation 命令非零）。
   建议：术语表补一行裁决（如 validation 类改「校验」或保留并在表中注明），至少消除「验证失败」同串双义。属既有问题，本期 glossary 把「验证」专属给真身核对后变为未闭合项。

3. **[低] 角色清单三处不一致（缺「验证」）**
   `docs/MANUAL.md:159` 术语表列 7 角色（含「验证」），与代码一致（`NodePanel.tsx:32`、`validate.ts:6`、`orchestration.rs:365,402`）；但 `AGENTS.md:45` 与 `tasks/TASK-11-naming.md` 均只列 6 角色（无「验证」）。
   建议：AGENTS.md 角色清单补「验证」，避免下期按 6 角色复核时把「验证」当漏网改掉。

4. **[低] `src-ui/src/components/resources/ResourcesOverlay.tsx:231` — 「建议加进 pi」**
   cc-switch 提示串用「加进」，是已退役的「添加」的同义词；审计脚本规则只匹配「添加」，未报。
   建议：改「建议接入 pi」。

5. **[低] `tasks/REPORT-11.md` 逐文件清单与末段补记不同步**
   第 155 行记 orchestration.rs 改为「未找到 resources.json，未接入资源」，第 229 行又记该后缀整句删除；代码现状是「未找到 resources.json」（`orchestration.rs:608`）。第 229 行记录的 `presetDisplayName` 接入（`GraphTab.tsx:376,641,763`、`OrchestrationPanel.tsx:148`）未回填进逐文件清单。按本仓交付规则（报告如实），建议把这两处并入对应小节。

6. **[极低/观察] 既有口语残留与机写标点**
   `docs/MANUAL.md:35`「验这一下防的是静默降级」、`:63-64`「一件大活拆成几件互不踩脚的小活……别进蜂群」——R8 口语体，本期手册改动未触及；`src/resources.rs:1334` `source: format!("申报({})")` 半角括号，机写值但会显示在来源处（R6 边缘）。均非本期引入。

## 核过没问题的点

- **六个内置预设名三处一致**：Rust `builtin_presets()`（`orchestration.rs:310–413`）= MANUAL 术语表 = 前端兜底 `BUILTIN_SWARM_PRESET`（`swarm.ts:65`），顺序一致，且有 `glossary-11.test.ts` 钉住（读 `?raw` 对表）。
- **没有按显示名匹配的逻辑被碰到**：擂台跳过角色填充按 id（`dispatch.ts:78` `builtin-arena`）、评审视角文本按 id（`stamp.ts:77`）、默认选中按 id `builtin-economy`（`OrchestrationPanel.tsx:84`）；`DispatchOverlay.tsx:458` 按「经济模式」名字匹配但该名未改。Rust 侧无任何按 preset name 的分支（全仓 grep 核过）。
- **用户预设旧数据可读**：显示层换名（`label.ts:17`），下拉选择、`saveUserPresets`、`listPresets` 仍走 id 与原名；旧名带不带「（我的）」本来就只在显示层拼，存储名未动。
- **旧图兼容**：`swarmRecordedResult` 同时识别「验收未通过」与旧「测试红」（`swarm.ts:469`，`swarm-08c.test.tsx:146` 有断言）；`RECONCILE_UNKNOWN_TEXT` 未改；印章名是运行期 Map 不落盘（`persist.ts:17`），重启后走「未盖预设不显后缀」分支（`GraphTab.tsx:763`）。
- **没改过头**：机器值 `STATUS_PENDING/FAILED`（待冒烟/冒烟失败）前后端一致保留（`resources/types.ts:80-81`、`resources.rs:30-32`）且界面不直接显示（走 `verificationLabel` 四态）；`smoke_*` 字段、命令名、i18n key、`builtin-*` id、`fixtures/prompts/split.txt` 全部未动；`server.rs` 中文只在测试数据；`channels/mod.rs:16` 的「判定」是发给模型的 prompt，不进界面。
- **内部词零漏网（src-ui 用户可见串）**：留痕/对账/口径/判定/闸门/门禁/G1–G4/任务编号/冒烟/添加 的命中全在注释、CSS 注释、测试与机器值；Rust 侧用户可见串本期已清（含审查中补掉的两处 `dispatch.rs:3328,3338`）。
- **状态↔动作成对**：接入↔移除、选定↔已选定、采纳↔取消采纳、验证↔重新验证/全部验证；「添加」按语境分流为 接入（资源/市场）/ 新建（节点/阶段）/ 记录（前提），各处抽查一致（SkillsPanel、ResourcesOverlay、PresetBuilder、SettingsModal、first-run.ts）。
- **标点**：zh-CN/zh-TW 抽查符合全角规则；剩余半角命中均为注释、非中文 locale（ja/en）与代码标识符。
- **测试断言同步**：`presets-05b-r2`（选定/已选定）、`swarm-08b`（验收未通过）、`consistency-06b`/`idle-and-tail-07b`（派发记录/核对）均已改；测试名/注释里的旧词不属用户可见串。
- 验证数字（vitest 402 过、build 过、cargo check 0 warning、cargo test 330 过）引自 REPORT-11 原文，本审查为只读、未重跑。