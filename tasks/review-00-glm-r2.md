# 第二轮复审

## 1. 逐条核实「已修」

### 需求完整性

| 项 | 位置 | 结论 | 依据 |
|---|---|---|---|
| dist 忽略+面板提示 | src-tauri/src/workspace.rs:10、src-ui/src/components/Changes.tsx:24 | 已核实 | `IGNORED_DIRS` 含 dist，hint 文案列出四项 |
| powershell 用注册表 defaultArgs | src-tauri/src/pty.rs:220-236、src-tauri/tools.json:31、pty.rs:269 | 已核实 | `launch_command` 两分支统一 `tool.default_args.clone()`，settings 只决定二进制；测试断言 |
| 默认 tab 粗体 | src-ui/src/App.tsx:25-27,203,273、src-ui/src/styles.css:145 | 已核实 | `default-tab` 类与 `defaultLeftTabs/defaultRightTabs` 集合对应任务书默认开关 |
| 图标 | src-tauri/tauri.conf.json:21、src-tauri/icons/icon.png、.gitignore | 已核实 | icon 存在、被引用、.gitignore 已不排除（MSI 生成未验证已在 REPORT 如实声明） |

### 逻辑正确性

| 项 | 位置 | 结论 | 依据 |
|---|---|---|---|
| 同步命令阻塞主线程 | src-tauri/src/lib.rs:36,44,68,83,102,115,125,134,146 | 已核实 | 重 IO 命令全部 async + `run_blocking`（lib.rs:249）；`write/resize/get_initial_state` 留同步但只碰内存锁，合理。遗留：`close_terminal` 见第 3 节 |
| Workspace 切项目残留 | src-ui/src/components/Workspace.tsx:11-22 | 已核实 | `useEffect([projectPath])` 清空 nodes/selection，根 div 再加 `key={projectPath}` 双保险 |
| mtime 假阳性 | src-tauri/src/workspace.rs:437-441 | 已核实 | `snapshot_content_changed` 仅比 hash；测试 :465 固定 mtime 不同+hash 相同不改 |
| spawn 失败进程泄漏 | src-tauri/src/pty.rs:80-93 | 已核实 | 两个失败分支都 `terminate_child`（含 taskkill 树杀）后返回错误 |
| monitor 争全局锁/中毒泄漏 | src-tauri/src/pty.rs:14-19,133-160,239-243 | 已核实 | 每终端独立 `Arc<Mutex<Child>>`，records 仅插入/移除时加锁；`lock_unpoisoned` 恢复中毒 |
| 整文件读入+无总量上限 | src-tauri/src/workspace.rs:240-290 | 已核实 | 64KiB 分块哈希；单文件 2MB、总 16MB 上限 |
| debug WebView 污染项目 | src-tauri/src/lib.rs:316-324 | 已核实 | `app_local_data_dir().join("webview-debug")`，不再写项目内 |
| `\\?\` 前缀 | src-tauri/src/lib.rs:47,278、workspace.rs:222、config.rs:53-59 | 已核实 | 全程 `dunce::canonicalize`，旧 recent 读入时剥前缀 |
| 会话字符串排序 | src-tauri/src/sessions.rs:59-66,231-236 | 已核实 | `timestamp_key` 转 `DateTime<Utc>` 排序；带时区偏移测试 :321 |
| 只杀直接子进程 | src-tauri/src/pty.rs:246-258 | 已核实 | Windows `taskkill /PID /T /F` + kill/wait，close/spawn 失败/Drop 三处调用；其他平台退化为 portable-pty 直接 kill（已声明） |

### 边界情况

| 项 | 位置 | 结论 | 依据 |
|---|---|---|---|
| --version 管道死锁 | src-tauri/src/tools.rs:88-137 | 已核实 | stdout/stderr 双线程 `read_to_end`，等待 2s + join 500ms 双超时；REPORT 实测读出 codex/pi 版本 |
| 大文本误标二进制 | src-tauri/src/workspace.rs:280-286 | 已核实 | 二进制仅由 NUL/无效 UTF-8 判定，超限走 `TextUnavailable`（binary=false），测试 :519 |
| symlink | src-tauri/src/workspace.rs:169-201,349-357 | 已核实 | `is_symlink` 透出、canonicalize 项目外拒绝（树与快照两处），junction 实测 :415 |
| 默认 shell 启动时报错 | src-tauri/src/pty.rs:222-227 | 已核实 | 先 `resolve_binary_name(default_shell)`，失败返回"未检测到默认 shell"且不进 openpty |
| 垃圾/重复会话 | src-tauri/src/sessions.rs:59-66,193-197 | 已核实 | `!recognized → None`，`(tool,id)` 去重后截断 300，测试 :321 |
| tools.json 静默吞错 | src-tauri/src/config.rs:15-25、src-tauri/src/lib.rs:286-288,328 | **修出新问题** | 解析失败确实报错了，但走的是 setup 失败 → `.run().expect()` **启动即 panic**，没有"透出到 UI"；且殃及 settings.json/recent.json（原先静默回退默认值，现在同样启动崩溃）。详见第 3 节 #1 |

### 安全

| 项 | 位置 | 结论 | 依据 |
|---|---|---|---|
| CSP | src-tauri/tauri.conf.json:12 | 已核实 | `default-src 'self'` + ipc connect-src + unsafe-inline style，级别符合建议 |
| 可执行确认 | src-tauri/src/lib.rs:225-232,106-121,336 | 已核实 | exe/bat/cmd/ps1/msi 大小写不敏感 + 原生 OkCancel 确认框，测试覆盖 |

### 代码质量

| 项 | 位置 | 结论 | 依据 |
|---|---|---|---|
| resolve_binary 分支合并 | src-tauri/src/tools.rs:22-35 | 已核实 | `let lookup = if cfg!(windows)...` 单段实现 |
| 锁样板 | src-tauri/src/lib.rs:226-244 | 已核实 | `read_lock/write_lock/mutex_lock` helper + 中文前缀 |
| Changes reset 无 catch | src-ui/src/components/Changes.tsx:30-40 | 已核实 | try/catch/finally + busy，失败保留现有结果 |
| Workspace 展开 catch | src-ui/src/components/Workspace.tsx:70-82 | 已核实 | catch 后 setError 并 return，保持折叠可重试 |
| 成功清 error | App.tsx:59,97,112,140；Workspace.tsx:17；Sessions.tsx:15；Changes.tsx:23,34 | 已核实 | 各成功路径均 `setError("")` |
| 终端主题监听 | src-ui/src/components/TerminalView.tsx:36,85-90,94 | 已核实 | `change` 监听更新 `terminal.options.theme`，cleanup 移除（浅色配色本身见第 3 节 #5） |
| 「配置变量」 | src-ui/src/App.tsx:243 | 已核实 | 按钮动词短语；后端错误均带中文操作前缀 |
| Workspace/Changes 重挂载一致 | src-ui/src/components/Workspace.tsx:22、Changes.tsx:41 | **未修全** | Changes 的 `key` 打在组件**返回的 div**上，`changes/diff` 是 Changes 自身 state，不随 key 重置——切项目后右栏仍显示旧项目的修改列表，直到手点刷新。见第 3 节 #3 |

### 测试覆盖

三项补测均落地且断言到位：workspace.rs:415（safe_path 穿越/绝对路径/项目外 junction）、workspace.rs:465（新增/删除/二进制/mtime/`@@` 行数）、config.rs:89（同 id 覆盖/新 id/缺字段报错）；附带 pty.rs:269、workspace.rs:519、sessions.rs:321、lib.rs:336。**已核实**。

## 2. 「不采纳」判断

- **§1.4 忽略目录可否展开**：理由成立。一审判断有误——Workspace.tsx:70-82 对所有 `isDir` 节点一视同仁调 `listDirectory`，`muted` 只加样式类（styles.css `.muted-node`），没有禁止展开逻辑。驳回正确。
- **§1.6 窄窗自动折叠**：理由成立（勉强）。任务书原文"窗口变窄时三栏可折叠，不出现横向滚动"按字面读是"可折叠"而非"自动折叠"，手动收起+响应式收窄+`overflow:hidden` 满足字面要求。属可辩护的解释，不算错。
- **§1.8 不 commit**：成立。本轮指令要求不 commit，REPORT 已如实记录。
- **§4.3–4.6**：成立。这四条一审本就写的是"未发现"，非缺陷项，无需改动。

## 3. 修正轮引入的回归 / 新 bug

1. **src-tauri/src/lib.rs:286-288,328（严重度：中）**：`read_json` 由"静默回 None"改为"解析失败返回 Err"后，settings.json / recent.json / tools.json 任何一个 JSON 损坏都会让 setup 失败 → `.run().expect("aicross failed to start")` 直接 panic，应用完全起不来、无任何 UI 提示。settings.json 本身由 config.rs:27-34 的非原子 `fs::write` 写出，写入中途崩溃即可自砖。建议：settings/recent 解析失败回退默认并透出警告，tools.json 失败降级为内置注册表 + UI 报错；write_json 改 temp+rename。
2. **src-tauri/src/lib.rs:210-215 + pty.rs:246-258（低）**：`close_terminal` 仍是同步命令（主线程执行），但 R1 给 `close()` 加入了 `taskkill /T /F` + `kill` + `wait`（同步等进程树退出）。杀顽固进程树时主线程/全部 IPC 会被卡住，与 2.1 修复方向自相矛盾。应一并 async 化。
3. **src-ui/src/components/Changes.tsx:41（低）**：如上——切项目后修改记录列表/diff 不重置，展示的是上一个项目的陈旧数据（基线其实已重建）。应在 `projectPath` 变化时清空 `changes/diff`（学 Workspace 的 useEffect）。
4. **src-ui/src/App.tsx:149-153（低）**：`closeTerminal` 中 `api.closeTerminal` reject 时仍移除 tab——进程可能还在跑但 UI 已无句柄；另有窄竞态：启动后立即关 tab，`open_terminal`（spawn_blocking，pty.rs:50 检查→:98 insert）尚未插入 record，close 落空后 open 才插入 → 幽灵终端。建议失败时保留 tab 或提示，后端 open/close 加同锁序检查。
5. **src-ui/src/components/TerminalView.tsx:110-114（备注）**：`terminalTheme` 的 light 分支背景仍是深色（#171a21 vs dark 的 #14161c），与 `.terminal-host` 固定深色一致，功能上自洽；但两套配色几乎相同，疑似浅色终端配色没做完，主题监听修复实际无感。

## 4. 对照 TASK-00 交付项 A–F

**一句话：代码层面 A–F 均有对应实现且自动验证绿，可以进入人工 smoke，但 F 的设置持久化存在"配置损坏即无法启动"的自伤路径（第 3 节 #1），建议修掉再验收。**

仍缺项：
- 验收清单第 2 条全部人工窗口 smoke（REPORT 自述未验证）。
- 配置文件损坏导致启动 panic（settings/recent/tools 三处，回归 #1）。
- `tauri build` 完整安装包（MSI/WiX 阶段因环境权限未跑通，图标阶段已验证）。
- 小项：切项目后修改记录列表不刷新（#3）、close_terminal 主线程阻塞（#2）、关 tab 竞态（#4）。