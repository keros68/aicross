# TASK-00 骨架：三栏窗口、终端 tab、文件树、修改记录、会话记录

先读 `AGENTS.md`。本期目标：一个能开窗口、能在 tab 里拉起本机 agent CLI、能看项目文件和改动的桌面壳。**不做**资源探测（第 1 期）、编排与讲解页（第 2 期）、派发闸门与结果屏（第 3 期）。

## 形态

三栏，每栏顶部是 tab。默认开启的 tab 用粗体；其余在设置里可开关。

| 栏 | tab |
|---|---|
| 左 | **工作区**（文件树）· **资源**（本期只放占位页，文字"第 1 期接入"）· **记录**（会话记录） |
| 中 | **启动台**（选 agent 开新终端 tab）· 终端 tab ×N |
| 右 | **修改记录** · 方案与编排（占位，第 2 期）· 任务列表（默认关） |

顶栏：产品名 aicross、当前项目目录、「打开目录」、「设置」。主题跟系统浅色/深色。窗口变窄时三栏可折叠，不出现横向滚动。

## 交付项

### A. 工程
- Tauri 2 + React 19 + TypeScript + Vite。`cargo check` 与 `npm run build` 都绿。
- `.gitignore`、`LICENSE`（AGPL-3.0 全文）、`NOTICE`（一句：界面形态参考 Coffee CLI，github.com/edison7009/Coffee-CLI；代码为独立实现）、`README.md`（三行：是什么、怎么跑、状态）。

### B. 启动台与终端 tab
- 静态注册表 `src-tauri/tools.json`（打包进二进制）：每项 `id / 显示名 / 二进制名 / 默认启动参数 / 说明`。首批：claude、codex、kimi、pi、powershell（普通终端）。用户覆写文件 `~/.aicross/tools.json`，同 id 覆盖。
- 启动时用 `where`（Windows）/`which` 检测每个二进制，启动台上标「已检测 / 未检测」和版本（跑 `<bin> --version`，2 秒超时；失败显示"版本未知"）。kimi 在 Windows 可能不在 PATH，补试 `~/.kimi-code/bin/kimi.exe`。
- 点启动台里的一项 → 新终端 tab：Rust 用 `portable-pty` 在**当前项目目录**拉起该 CLI；前端 `@xterm/xterm` 渲染；支持输入、resize、关闭（关闭即结束进程）。每个 tab 可指定额外环境变量（键值表单），值不回显到日志。
- tab 标题 = 工具名 + 序号；tab 上一个小状态点：运行中 / 已退出。

### C. 工作区（文件树）
- 读当前项目目录，目录懒加载展开；`.git`、`node_modules`、`target` 默认折叠并置灰。
- 点文件：复制路径按钮 + 在系统默认程序打开。不做编辑器。

### D. 修改记录
- 工具无关：打开项目时拍一次目录快照（相对路径、大小、mtime、内容哈希）；「刷新」再拍一次并对比，列出新增 / 修改 / 删除。
- 点文本文件显示行级 diff（前后快照内容都在内存或临时目录里；二进制只显示大小变化）。
- 「重置基线」把当前状态设为新基线。

### E. 记录（会话记录）
只读，列表：时间、工具、模型、项目目录、首条用户消息前 80 字。来源：
- Claude Code：`~/.claude/projects/**/*.jsonl`
- Codex：`~/.codex/sessions/**/*.jsonl`
- Kimi Code CLI：`~/.kimi-code/sessions/**/wire.jsonl`
- pi：`~/.pi/sessions/`（结构先看再定，能解析多少写多少）
字段名以实际文件为准，先各拿一份样本看结构再写解析；解析不到的字段显示"—"。绝不把密钥类字段读进内存以外的任何地方。点一条记录只显示元数据，不展示对话全文（第 0 期）。

### F. 项目与设置
- 「打开目录」选项目；最近项目列表（`~/.aicross/recent.json`）。
- 设置：各 tab 开关、默认 shell。存 `~/.aicross/settings.json`。

## 验收
1. `cargo check --manifest-path src-tauri/Cargo.toml` 与 `cd src-ui && npm run build` 输出贴进 REPORT。
2. `npm run tauri dev` 打开窗口；在 REPORT 里逐条记录手动 smoke：打开目录 → 启动台看到 claude/codex/kimi/pi 检测状态与版本 → 开一个 powershell tab 能输入回显 → 开一个 `claude` tab 能进交互界面再关闭 → 文件树能展开 → 改一个文件后修改记录能列出并显示 diff → 记录 tab 能列出最近会话。
3. 没做到的项如实写"未做"或"未验证"。

## 参考
- 形态：`docs/proto/index.html`（原型，三栏与 tab 的排法以本任务书为准，原型里的编排/结果屏本期不做）。
- 结构参考：`D:\work\coffeecli`（可读，按 AGENTS.md 规则不得复制）。发现它的 bug 记到 `docs/coffeecli-notes.md`。
