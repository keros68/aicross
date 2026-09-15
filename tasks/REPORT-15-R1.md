# REPORT-15-R1 总指挥启动器复审修正

## 完成内容

1. 自动直启的目录顺序

   - `commanderNewTabLaunch` 统一按「当前工作区 → 已存默认目录」取 cwd。
   - 新建 tab 和冷启动首 tab 都消费这个结果，不再用存量默认反向切走工作区。
   - 测试：`TASK-15-R1：新总指挥 tab 有默认时直接生成启动参数，当前工作区优先于存量默认目录`。

2. 默认值晚到不接管已显示启动器的 tab

   - 新增 `tab id → started / launcher / changed` 纯状态机。启动器进入布局时登记 `launcher`，「更改」登记 `changed`，两者都不再被后到默认或探测变化覆盖。
   - 「+ → 总指挥」在默认可用时直接新建带工具的 PTY tab，不先挂空 graph，因此不渲染一帧启动器，也不调用启动器的模型清单请求。
   - 测试：`TASK-15-R1：探测 false→true 只启动一次`、`TASK-15-R1：另一 tab 写入默认不接管已显示启动器的 tab`、`TASK-15-R1：点“更改”后不再自动启动`。

3. 会话细行与 tab 标题跟随当前目录

   - tab 保留 agent/provider/model 启动快照，目录段在渲染时读 `folderPath`。`SET_FOLDER` 后重启会保留前三者并显示新目录。
   - 测试：`TASK-15-R1 换目录重启后保留 agent/model 快照，目录读当前 tab`、`tab 标题是 agent · 模型 · 目录名，pi 带 provider`。

4. G1 与编排面板显示名

   - `routeLine` 及 `agent/model` 数据契约保持机器 id，裁决上下文不变。
   - `DispatchOverlay`、`NodePanel`、`OrchestrationPanel`、`PresetBuilder` 的用户可见坐标在渲染层经 `agentModelDisplay`转成 `Claude Code / Codex / Kimi Code / pi agent`。
   - 测试：`G1 数据仍保留机器 id，四个渲染面统一转显示名`。

5. 核心接线测试

   - `CenterPanel` 真实调用状态机的 `availability / launcher-shown / changed` 三个动作，并在建 tab 时调用 `commanderNewTabLaunch`。
   - 删除纯转发的 `commanderAutoStart`、无生产价值的 `commanderLauncherToolPatch` 和 `markCommanderLauncherOpened`。
   - 测试：第 2 条三个 reducer 级用例，以及 `TASK-15-R1：CenterPanel 真实接入 tab 状态机与建 tab 直启`。

6. Rust 与前端显示名对照

   - Vitest 用 `node:fs` 读取 `src/resources.rs`，解析四个 `AGENT_NAME_*` 后与 `AGENT_DISPLAY_NAMES` 逐项比较。
   - 测试：`TASK-15-R1：前端显示名与 Rust 常量逐项一致`。

7. 四家 agent 图标统一

   - 移除 `brand-marks.ts` 里的第三方 SVG 路径与 `glyph` 分支。`AgentBadge` 只消费 fork 已有资产：`public/icons/tools/claude.svg`、`public/icons/tools/codex.svg`、`icons-inline/kimicode.png`、`icons-inline/pi.svg`。
   - 启动器卡、左栏资源、接入页、启动卡、历史行均接入 `AgentBadge`。`NOTICE` 只改 aicross 自己的资产段，上游 NOTICE 原文保持不变。
   - 测试：`claude / codex / kimi / pi 都走本地资产，未知项退回字母`、`AgentBadge 对四家都渲染已有图标，无字形分支`、`启动器、左栏资源、接入页、启动卡和历史行都接入 AgentBadge`。

8. 网格与文档

   - CSS 测试新增 `.commander-agent-card.is-picked` 只改边框色，不改 `width / padding / border-width` 的断言。
   - `PRODUCT.md`、`README.md`、`AGENTS.md` 已同步图标决定和四个显示名；`docs/proto/index.html` 文件头标明为历史快照。
   - 测试：`TASK-15：卡片网格固定列宽和行高，窄窗口可换行`、`文档同步四个显示名、Coffee 图标与冷启动直启`。

9. pi provider 与死分支

   - pi 的默认行、会话细行和 tab 标题改为 `provider / model`。
   - 自动直启 effect 中的死条件随旧实现删除；图标 `glyph` 类型、渲染分支和 CSS 选择器已删除。
   - 测试：`tab 标题是 agent · 模型 · 目录名，pi 带 provider`、`图标表无 simple-icons 路径数据与 glyph 分支`。

10. Kimi 常量与冷启动说明

   - `parse_kimi_config` 的显示名参数改用 `AGENT_NAME_KIMI`。
   - `MANUAL.md`、`PRODUCT.md` 与本报告明确：有效默认下，应用冷启动的首个 tab 直接进入会话；会话上方保留「更改」回路。
   - `src-ui/src/styles/global.css` 本轮没有必要改动，工作树中无该文件 diff。
   - 测试：`Kimi 探测调用引用统一常量`、`文档同步四个显示名、Coffee 图标与冷启动直启`。

## 测试先红

状态机与接线测试先加入后，定向 Vitest 在未实现状态机时产生 6 个失败：

```text
Test Files  1 failed (1)
Tests  6 failed | 31 passed (37)
```

失败点分别是当前工作区优先、探测 false→true 一次启动、已显示启动器不接管、「更改」不再启动、pi provider 标题，以及 `CenterPanel` 真实接线。实现后完整前端套件转绿。

## 验证

```text
$ cargo check --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.81s
```

无 warning。

```text
$ cargo test
test result: ok. 383 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out
```

```text
$ cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  31 passed (31)
Tests  482 passed (482)
```

```text
$ cd src-ui && npm run build
✓ 335 modules transformed.
✓ built in 838ms
```

`scripts/copy-audit.py` 运行完成，仍报 4 个既有命中：`DispatchOverlay.tsx` 的空值占位「—」、`SettingsModal.tsx`、`graph/stamp.ts`、`resources/types.ts`。本轮没有扩大到这些既有文案口径。

换行复核：`src/server.rs` 保持 4666 CRLF / 0 LF-only，`src/resources.rs` 保持 0 CRLF / 3799 LF-only，`src-ui/src/tauri.ts` 保持 576 CRLF / 107 LF-only且未修改。`AGENTS.md`、`README.md`、`docs/PRODUCT.md` 保持原有全 CRLF。`git -c core.whitespace=cr-at-eol diff --check` 无输出。

## 没做与边界

- 任务书点名的 `src-ui/public/icons/tools/kimicode.svg` 与 `pi.svg` 在当前仓库不存在。按「只用仓库已有文件」的硬性规则，Kimi Code 使用已有 `src-ui/src/icons-inline/kimicode.png`，pi 使用已有 `src-ui/src/icons-inline/pi.svg`；未新增、转换或下载图标资产。
- 未启动桌面窗口，未发真机 CLI 或模型请求。
- 未修改 `src/terminal.rs`、`src-ui/src/tauri.ts`、`src/server.rs` 或 `D:\work\aicross` 主工作树。
- 未运行 `cargo fmt`，未 commit，未 push。
