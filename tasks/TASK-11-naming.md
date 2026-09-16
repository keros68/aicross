# TASK-11 界面文案与命名统一（09-15 用户提出）

用户看到预设下拉「共享模式 / 经济模式 / 盲审模式 / 擂台 / 评审团 / 蜂群模式 / 手测三家流水线（我的） / 红蓝 · 后续期次」混排，要求把**全部按钮、命名规则、文字描述**统一成一套。规则见 `AGENTS.md`「界面文案与命名规范」，完整对照表在 `D:\work\skillmanager\AGENTS.md`「界面文案规范」，样板记录 `D:\work\skillmanager\docs\文案精简记录-2026-08-16.md`，全局准星 `C:\Users\keros68\.codex\guidance\copywriting.md`。三份都先读完。

## 范围

用户可见的每一个字符串：`src-ui/src/**`（按钮、标签、tab 名、菜单项、对话框标题、占位符、提示、错误文案、空态）、Rust 侧会进界面的字符串（`src/orchestration.rs` 内置预设与角色名、`src/dispatch.rs` 合并/派发 note、`src/resources.rs` 验证状态文案、`src/server.rs` 返回给前端的错误串）、`docs/MANUAL.md`。不动代码逻辑、className、事件、id（预设 id `builtin-*` 与角色的机器值不改，只改显示名）。

## 已定的命名

- 预设：共享模式 / 经济模式 / 盲审模式 / 擂台模式 / 评审模式 / 蜂群模式；用户自建 → 「自定义模式」（多个时「自定义模式 · <用户起的名>」），去掉「（我的）」；`FUTURE_PRESETS` 显示「红蓝模式（规划中）」「群聊模式（规划中）」。
- 角色两字：出方案 / 执行 / 验收 / 拆分 / 顾问 / 仲裁（已是；核对全部出现处一致）。
- 动作一词：派发 / 合并 / 采纳 / 重派 / 叫停 / 接入 / 移除 / 验证（「冒烟」全部改「验证」，「添加」改「接入」）。
- 界面上不出现：留痕、对账、口径、判定、闸门、门禁、落库、wire、G1/G2/G3/G4、CAID、R1 之类内部词与任务编号。

## 步骤

1. **盘点**：把 `D:\work\skillmanager\scripts\copy-audit.py` 移植为 `scripts/copy-audit.py`（根目录扫 `src-ui/src`，规则按本仓 AGENTS.md 的规范；只报告不改），先跑一遍出清单。Rust 侧用 grep 补盘点。
2. **术语表**：在 `docs/MANUAL.md` 末尾加「术语表」：概念 → 唯一用词 → 出现位置；同一概念多个词的，选一个，其余全部替换。
3. **统改**：按术语表与规范逐文件改；每处改动能删则删（先问能删哪句）。测试里断言了旧字符串的一并改；新增一条 vitest 读术语表核对内置预设显示名。
4. **复核**：改完再跑一遍 `scripts/copy-audit.py`，剩余项逐条说明为何保留；抽查双重否定与条件句。
5. **报告** `tasks/REPORT-11.md`：逐文件列「旧 → 新（规则编号）」，整句删除单列；体检脚本前后计数。

验证：`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`、`cargo check` 零 warning、`cargo test`。保持每个文件原有换行（`src-ui/src/tauri.ts` 混合换行只做字节级最小改动；`src/server.rs` 是 CRLF）；不 `cargo fmt`；不起窗口；不 commit、不 push。

注意：`phase/10`（控制台启动器）正在另一个工作树修正，会改 `CenterPanel.tsx`、`Explorer.tsx`、`TierTerminal.tsx`、`src-ui/src/commander/*`；这几个文件的文案本期照改，合并冲突由编排者处理。
