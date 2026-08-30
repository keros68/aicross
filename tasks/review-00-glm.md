# 代码审查报告：TASK-00 骨架

## 1. 需求完整性

- **tasks/REPORT-00.md（验收清单第 2 条）**：手动 smoke 一项都没跑（打开目录、启动台检测、powershell/claude tab、文件树、diff、记录 tab 全部"未验证"）。任务书要求"逐条记录手动 smoke"，这是验收的一半，等于验收只完成了一半。建议：在普通桌面会话补跑并回填 REPORT。
- **src-tauri/src/workspace.rs:9**：`IGNORED_DIRS` 加了 `dist`，任务书 C/D 只规定 `.git / node_modules / target`。副作用是修改记录对 `dist/` 下的真实改动不可见，属超出任务书的静默偏差。建议：至少在 REPORT/界面提示里写明，或把忽略列表与任务书对齐。
- **src-tauri/src/pty.rs:225-245（launch_command）**：powershell 分支忽略 `tools.json` 的 `defaultArgs`，按文件名是否含 "powershell/pwsh" 硬编码 `-NoLogo`；用户在 `~/.aicross/tools.json` 覆写 powershell 项的参数完全无效（只有非 powershell 工具才用 `default_args`）。与任务书 B"每项默认启动参数"不符。建议：统一走 `default_args`，settings 只决定二进制名。
- **src-ui/src/components/Workspace.tsx:19**：任务书 C 说 `.git/node_modules/target` "默认折叠并置灰"，实现里这些目录无法展开查看（懒加载正常，但没有任何入口进入被忽略目录）——可接受，但与"折叠"字面含义有差。
- **src-ui/src/App.tsx / styles.css**："默认开启的 tab 用粗体"没有实现——所有启用的 tab 样式一致，无默认/可选之分。
- **src-ui/src/App.tsx:131-136**："窗口变窄时三栏可折叠"只做了手动折叠按钮，没有任何随宽度自动折叠的行为；靠 `overflow: hidden` 保证不出横向滚动。若任务书本意是自动折叠，则未做。
- **src-tauri/tauri.conf.json:17-21**：`bundle.icon: []` 且 `targets: "all"`，`tauri build` 打包阶段会因缺图标失败。本期只要求 `cargo check`/`npm run build` 绿，不算硬缺项，但"能开窗口"的发布形态不完整。
- **AGENTS.md 交付流程第 3 条**：REPORT 自述未能创建 commit（`.git/index.lock` 权限），仓库目前没有本期 commit。流程缺项。

## 2. 逻辑正确性

- **src-tauri/src/lib.rs:34（detect_tools）、:78（refresh_changes）、:42（set_project）、:127（list_sessions）**：全部是同步 `#[tauri::command]`。Tauri 2 中非 async 命令默认在主线程执行：`detect_tools` 串行跑 5 个 `<bin> --version`（各 2s 超时）最坏挂主线程约 10s；`list_sessions` 要 walk 4 个根目录并解析最多 1000 个文件 × 1 万行；`refresh_changes`/`set_project` 全量重哈希项目。主线程阻塞期间窗口拖动、resize、所有其它 IPC 全部冻结。建议：改成 `async fn`（或 `#[tauri::command(async)]` + `spawn_blocking`），检测/扫描放到线程池。
- **src-ui/src/components/Workspace.tsx:19、:74**：`TreeNode` 以 `relativePath` 为 key，切换项目后同名目录会复用组件实例，`children`/`expanded` 状态残留上一个项目的内容——切项目再展开同名文件夹显示的是旧项目的文件。`Changes.tsx:33` 用了 `key={projectPath}` 重挂载，Workspace 漏了同样的处理。建议：给 Workspace 根 div 或 `<TreeNode>` 外层加 `key={projectPath}`。
- **src-tauri/src/workspace.rs:262-267（compare）**：`left.modified != right.modified` 单独成立也判 "modified"——touch 一下文件（内容哈希、大小都没变）就进修改记录，且点开显示"文件内容无行级变化"，纯假阳性。建议：以 `hash != hash` 为准（mtime 只能做优化预筛，不能作为判据）。
- **src-tauri/src/pty.rs:79-91**：`spawn_command` 成功后 `try_clone_reader()`/`take_writer()` 失败直接 `return Err`，此时子进程已启动但未 kill、未入 records——进程泄漏，且前端会收到错误但进程在后台跑。建议：失败路径先 `child.kill()` + `wait()`。
- **src-tauri/src/pty.rs:125-155**：每个终端一个 monitor 线程，每 150ms 抢一次**全局** `records` 锁做 `try_wait` 轮询，与 `write`/`resize` 争锁；且锁中毒时 monitor 直接 return（:133-135），record 永不清理、reader 线程永久阻塞在 read 上——泄漏路径。建议：child 放进每终端独立的 `Arc<Mutex<Child>>`，monitor 只碰自己的锁；或用 reader 线程 EOF 事件驱动清理替代轮询。
- **src-tauri/src/workspace.rs:216**：`fs::read` 把每个文件完整读进内存再哈希，无大小上限；快照结构还各留一份 `content: Option<String>`（≤2MB/文件，baseline+current 两份）。大仓库或含大文件的目录会让内存翻倍上涨。建议：流式哈希（分块读），content 设总量上限。
- **src-tauri/src/lib.rs:214、:231**：debug 构建把 WebView 数据目录放进 `project/.aicross-webview`，而它不在 `IGNORED_DIRS` 里——每次"刷新"都重哈希 WebView 缓存文件，修改记录出现噪音条目，还污染用户项目目录。建议：数据目录移出项目（或加入忽略列表）。
- **src-tauri/src/lib.rs:45、:208（canonicalize）**：Windows 上 `canonicalize` 返回 `\\?\D:\...` verbatim 路径，顶栏项目路径、`recent.json`、传给 `open_file` 的路径全是 `\\?\` 前缀——显示难看，且 `tauri-plugin-opener`（ShellExecute）对 verbatim 路径的兼容性不保证，部分程序打不开。建议：用 `dunce::canonicalize` 或去前缀后再存/显示。
- **src-tauri/src/sessions.rs:43**：按 timestamp **字符串**排序，但格式混源：chrono `to_rfc3339()` 产出 `+00:00`，Claude/Codex 原文是 `Z` 或带本地时区偏移——带时区偏移的字符串序≠时间序，最近会话排序会错。建议：解析成 `DateTime<Utc>` 再排。
- **src-tauri/src/pty.rs:191-207（close）**：只 kill 直接子进程，agent 在终端里再起的孙进程不保证回收（ConPTY 关闭后多半退出但不保证）。第 0 期可接受，建议在代码注释或 REPORT 注明。

## 3. 边界情况

- **src-tauri/src/tools.rs:87-95（version_for）**：先等进程退出、后 `read_to_string` 读 stdout——若 `--version` 输出超过管道缓冲（几 KB 起），子进程阻塞在写管道上永不退出 → 2s 超时被杀 → 永远"版本未知"。REPORT 里 codex/pi 的"版本未知"很可能与此/超时有关。建议：起线程并发读 stdout/stderr，再带超时 join。
- **src-tauri/src/workspace.rs:222-226**：>2MB 的纯文本文件 `content=None`，被当"二进制"处理（只显示大小变化）——大文本文件误标。建议：二进制判定只看 NUL/UTF-8 有效性，超限单独提示"文件过大"。
- **src-tauri/src/workspace.rs:141-159（符号链接）**：目录 symlink/junction 的 `file_type().is_dir()` 为 false → 树里显示为文件、无法展开；`capture`（:212 `is_file()`）对 symlink 文件直接跳过——链接目标的内容变化不会出现在修改记录。建议：至少在 UI 区分 symlink，快照决定是否跟随。
- **src-ui/src/components/SettingsDialog.tsx:20-25 + src-tauri/src/pty.rs:226**：默认 shell 选 `pwsh.exe` 而机器只装 Windows PowerShell 时，要到点启动、ConPTY spawn 失败才报错。建议：启动前对 default_shell 也做存在性检测。
- **src-tauri/src/sessions.rs:90-193（parse_session）**：解析全失败的垃圾 jsonl 也返回 `Some`（id=文件路径、其余"—"），占据列表名额；Claude 同一 sessionId 的多个 jsonl 文件会产生重复条目，且 `Sessions.tsx:23` 的 React key `${tool}-${id}` 可能撞 key。建议：按 `(tool, id)` 去重，无任何有效字段的文件跳过。
- **src-tauri/src/config.rs:13-21（read_json）**：解析失败静默吞成 `None`——用户在 `~/.aicross/tools.json` 少写一个 `description` 字段，整个覆写文件被静默忽略，无任何提示。建议：覆写文件损坏时返回错误并透出到 UI。

## 4. 安全

- **src-tauri/tauri.conf.json:12（"csp": null）**：CSP 完全关闭。当前没有 `innerHTML` 注入点、xterm 渲染是安全的，但本应用的核心用途就是渲染外部 CLI 输出，后续接会话内容/web 资源时无 CSP 兜底。建议：配置基本 CSP（`default-src 'self'; style-src 'self' 'unsafe-inline'` 级别即可）。
- **src-tauri/src/lib.rs:104（open_file）**：对项目内任意文件走系统默认程序打开——`.exe`/`.bat` 会被直接执行。与任务书"系统默认程序打开"字面一致，但恶意仓库可借 UI 一键执行。建议：可执行扩展名加确认，或排除。
- capabilities 权限面（core:default + dialog:allow-open）：未发现过宽。
- 环境变量值：仅存前端内存与 IPC 传输、后端无任何日志落盘、`settings.json` 不存 env——未发现泄漏路径。
- shell 注入：未发现（全程 argv 数组，无 shell 拼接；`where.exe` 参数来自用户本机的 tools.json，同信任域）。
- 会话解析只取白名单元数据字段、不落盘：符合任务书"绝不把密钥类字段读进内存以外的任何地方"。

## 5. 代码质量

- **src-tauri/src/tools.rs:12-51（resolve_binary）**：windows/非 windows 两分支除命令名外逐行重复。建议：`let lookup = if cfg!(windows) { "where.exe" } else { "which" };` 合并为一段。
- **src-tauri/src/lib.rs（全文）**：`.read().map_err(|error| error.to_string())`、`.lock().map_err(...)` 样板重复 20+ 次。建议：抽 `fn lock<T>(lock: &RwLock<T>) -> Result<..., String>` 或换 `parking_lot`（无中毒错误）。
- **src-ui/src/components/Changes.tsx:26-30（reset）**：`await api.resetBaseline()` 无 try/catch——后端失败时未处理 rejection，且 UI 仍清空列表。建议：与 refresh 一致的 try/catch + busy 态。
- **src-ui/src/components/Workspace.tsx:60（activate）**：`setChildren(await api.listDirectory(...))` 无 catch——展开无权限目录时 unhandled rejection、界面无提示。建议：catch 后 setError。
- **src-ui/src/App.tsx:33-37、Workspace.tsx:14、Sessions.tsx:10**：加载失败设置 error 后，下次成功不清除旧错误，错误横幅常驻。建议：成功路径清 error。
- **src-ui/src/components/TerminalView.tsx:73（terminalTheme）**：主题只在挂载时读一次 `matchMedia`，系统浅深切换后终端配色不变（CSS 变量部分会变，终端不会）。建议：监听 `change` 事件更新 `terminal.options.theme`。
- 文案（AGENTS.md 规范）：整体合格（中性、无 emoji、按钮动词短语、已检测/未检测成对）。两处小问题：**App.tsx:316** "环境变量"按钮是名词不是 2–6 字动词短语（建议"配置变量"）；后端各处 `error.to_string()` 把英文 io 错误（如 "os error 2"）直接透进中文 UI。
- 重复实现：`Changes.tsx` 的 `key={projectPath}` 重挂载模式与 `Workspace` 不一致（见逻辑正确性第 2 条），属同一问题的两面。

## 6. 测试覆盖

现有 4 个测试覆盖：
1. `tools.rs::embedded_registry_detects_all_five_tools`——注册表五项齐全 + 本机 PATH/version 探测冒烟（依赖本机环境，非隔离）。
2. `pty.rs::portable_pty_runs_a_child_and_reads_output`——ConPTY 起 cmd/sh、读回显、应答 cursor query。
3. `workspace.rs::snapshot_reports_modified_text_and_diff`——modified 识别 + diff 含 `-old/+new`。
4. `sessions.rs::pi_parser_keeps_only_requested_metadata`——pi jsonl 的 session/model_change/message 解析。

最该补的 3 个：
1. **路径越界测试**（`workspace.rs::safe_path/list_directory`）：`..` 穿越、绝对路径注入、项目外 symlink、空 `relative_path`——这是全仓库唯一的安全不变量，目前零覆盖。
2. **tools.json 覆写合并测试**（`config.rs::load_tools`，需先把 `config_dir` 参数化）：同 id 覆盖、新增 id、字段缺失时整文件被静默忽略的行为（顺便修复它）。
3. **compare/diff 边界测试**（`workspace.rs`）：纯新增、纯删除、仅 mtime 变化（固化或消除假阳性）、二进制大小变化、`@@` 头行号计数——`line_diff` 的上下文计算完全没被测过。

（顺带：claude/codex/kimi 三个解析器只有 pi 有 fixture，格式漂移无护栏。）

---

## 对照 TASK-00 原始需求，是否完整实现？

**一句话：交付项 A–F 在代码层面均有对应实现且自动验证绿，但验收清单第 2 条的人工 smoke 一项未跑、若干实现与任务书有偏差，不能算完整交付。**

缺项/偏差列表：
1. 手动 smoke 全部未验证（打开目录→检测状态→powershell/claude tab→文件树→diff→记录 tab，REPORT 自述）。
2. 未创建 Conventional Commit（AGENTS 交付流程）。
3. powershell 工具忽略 tools.json 的 `defaultArgs`（硬编码 `-NoLogo`）。
4. 快照/文件树额外忽略 `dist`，超出任务书范围且未声明。
5. "默认开启的 tab 用粗体"未实现；"窗口变窄自动折叠"未实现（仅手动折叠）。
6. `bundle.icon` 为空，`tauri build` 打包不可用。
7. REPORT 中 Codex/pi 版本探测显示"版本未知"，`--version` 采集路径存在管道死锁隐患，属半成品。
