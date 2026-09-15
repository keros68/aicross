# REPORT-15 总指挥启动器与 agent 名称统一

## 完成内容

1. 默认总指挥

   - `commanderAutoStart` 只在默认有效、工具探测完成且默认 CLI 已安装时返回真；
     `CenterPanel` 据此把空白总指挥 tab 直接交给原生 PTY。
   - 会话上方增加细行，显示显示名、首次模型和目录；「更改」将 tab 还原为
     空白 `graph` 启动器。该 tab 同时写入本次自动启动集合，避免用户主动更改后
     又被默认值立刻启动。
   - 默认不成立、未安装或探测未完成时仍显示启动器。模型与 thinking 档改在原生
     CLI 会话内进行，手册已写明。

2. 名称与图标

   - 前端新增 `resources/agent-display.ts`，统一 Claude Code、Codex、Kimi Code、
     pi agent，并为机器 id `kimicode` 映射 Kimi Code。启动器、资源左栏、接入页、
     模型分组、启动卡、tab 和工具设置均取该表。
   - Rust 在 `resources.rs` 导出四个显示名常量；资源库 `NATIVE_AGENTS` / pi 骨架
     和四个 ToolDescriptor 都引用这些常量。旧 `resources.json` 的 Kimi Code CLI /
     pi 名称会在 `ensure_agents` 时迁移。
   - pi 检查结论：`pi` 指向 `C:\Users\keros68\.local\bin\pi.cmd`，再指向
     `D:\pi-official\pi.exe`；安装包为 `@earendil-works/pi-coding-agent` 0.85.1（MIT）。
     包内只发现 `clankolas.png` 与文档截图／吉祥物，没有可复用的官方 SVG、ICO 或
     PNG 标志。README 的 `https://pi.dev/logo-auto.svg` 是远程引用，许可不在本地
     材料中明确。因此使用小写字母徽标 `pi` 回退，不引入远程资产，NOTICE 无需新增条目。

3. 启动器形态

   - agent 卡改为定宽 CSS 网格；四卡同宽、48px 同高、图标与名称左对齐，窄窗口
     自动换行，选中态仍为 1px 边框色。
   - 参考 Coffee 设置页的固定网格、统一内边距和水平卡片形态，未复制 Coffee 代码。

4. 文档与测试

   - `docs/MANUAL.md` 补充默认直启、会话细行、更改入口和 CLI 内模型／thinking 调整，
     并更新术语表。
   - 新增／更新测试：
     - `task15_builtin_agent_display_names_are_canonical`：Rust 资源名称、旧库迁移，及
       工具注册表与资源库同源。
     - `TASK-15：四个 agent 显示名从同一张前端表取`：前端统一表与 `kimicode` 映射。
     - `TASK-15：默认有效且已安装时，新总指挥 tab 直接启动；其余情况显示启动器`。
     - `TASK-15：会话细行的“更改”回到空白 graph 启动器，不落到系统终端`。
     - `TASK-15：卡片网格固定列宽和行高，窄窗口可换行`：读取
       `CenterPanel.css`，断言 `.commander-agents` 实际使用 `display: grid` 与
       `grid-template-columns: var(--commander-agent-grid)`，并断言卡片高度消费
       `--commander-agent-card-height`。
     - pi 徽标回退字母的 `AgentBadge` / `agentEmblem` 断言。

## 测试先红

先新增前端显示名／自动启动测试后运行：

```text
FAIL  src/commander/__tests__/commander-10.test.tsx
Error: Cannot find module '../../resources/agent-display'
```

实现统一表后该定向测试转绿；随后补齐名称、默认启动、网格和 pi 回退断言。

主线程独立做了红绿核对：临时把四处实现分别改回旧名称、关闭自动启动、改变
网格列宽并恢复 `π`，定向 Vitest 如期出现 5 个失败；恢复实现后同一组为
2 个文件、71 个测试全部通过。临时改动未保留。

## 验证

```text
$ cargo check --all-targets
Compiling aicross v0.1.0 (D:\work\aicross-ui)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.08s
```

```text
$ cargo test
test result: ok. 383 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out
```

```text
$ cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  30 passed (30)
Tests  472 passed (472)
```

```text
$ cd src-ui && npm run build
✓ 335 modules transformed.
✓ built in 622ms
```

以上四项由主线程在最终工作树独立复跑。`src/server.rs` 保持 4666 个 CRLF、
0 个 LF-only；`src-ui/src/tauri.ts` 保持 576 个 CRLF、107 个 LF-only，均未修改。
本期改动的 Rust 文件仍为 LF-only；其余新改文件保持各自原有换行类型，
`CenterPanel.tsx` 保留原有混合换行，未做整文件归一化。`git diff --check` 无输出。

`scripts/copy-audit.py` 已运行，报告 4 条既有命中：`DispatchOverlay.tsx`、
`SettingsModal.tsx`、`graph/stamp.ts`、`resources/types.ts`。本期未改这些文件，
未扩大到无关文案修复。

## 未做

- 未启动桌面窗口，未做真机 CLI／模型请求。
- 未修改 `D:\work\aicross` 主工作树，未 commit、未 push，未运行 `cargo fmt`。
