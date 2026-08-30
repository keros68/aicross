# REPORT-00：第 0 期桌面骨架

## 完成内容

- 建立 Tauri 2 + React 19 + TypeScript + Vite 工程，前后端分别位于 `src-tauri/` 与 `src-ui/`。
- 实现三栏窗口、系统浅色/深色主题、可折叠侧栏、tab 开关和默认 shell 设置。
- 实现静态工具注册表与 `~/.aicross/tools.json` 同 id 覆写；支持 PATH 检测、Kimi Windows 备用路径、2 秒版本超时和检测状态。
- 实现基于 `portable-pty` 的多终端 tab，支持输入、resize、关闭进程、运行状态和本次进程环境变量。
- 实现项目目录选择、最近项目、懒加载文件树、复制路径和系统默认程序打开文件。
- 实现目录基线、刷新对比、新增/修改/删除、文本行级 diff、二进制大小变化和重置基线。快照忽略 `.git`、`node_modules`、`target`、`dist`，避免对依赖和构建产物做大规模哈希。
- 实现 Claude Code、Codex、Kimi Code CLI、pi 会话元数据读取；兼容任务书中的 `~/.pi/sessions/` 和本机实际使用的 `~/.pi/agent/sessions/`。界面只显示元数据，不展示或保存对话全文。
- 添加 AGPL-3.0 全文、NOTICE、三行 README、`.gitignore` 和根级 `npm run tauri dev` 入口。
- 添加工具检测、PTY、目录快照/diff、pi 会话元数据解析四项 Rust 测试。

## 自动验证

### Rust 检查

命令：

```text
cargo check --manifest-path src-tauri/Cargo.toml
```

结果原文：

```text
   Compiling aicross v0.0.0 (D:\work\aicross\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.65s
```

### 前端构建

命令：

```text
cd src-ui
npm run build
```

结果原文：

```text
> aicross-ui@0.0.0 build
> tsc && vite build

vite v7.3.6 building client environment for production...
transforming...
✓ 46 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                   0.45 kB │ gzip:   0.29 kB
dist/assets/index-KZBm8pI9.css   14.65 kB │ gzip:   3.75 kB
dist/assets/index-BjuU_djG.js   502.19 kB │ gzip: 139.61 kB

(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rollupOptions.output.manualChunks to improve chunking
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
✓ built in 1.02s
```

构建成功；存在一个 502.19 kB 的体积警告，不影响第 0 期运行。

### Rust 测试

命令：

```text
cargo test --offline --manifest-path src-tauri/Cargo.toml -- --nocapture
```

结果原文：

```text
running 4 tests
test sessions::tests::pi_parser_keeps_only_requested_metadata ... ok
test workspace::tests::snapshot_reports_modified_text_and_diff ... ok
test pty::tests::portable_pty_runs_a_child_and_reads_output ... ok
claude: detected=true, version=2.1.251 (Claude Code)
codex: detected=true, version=版本未知
kimi: detected=true, version=0.39.1
pi: detected=true, version=版本未知
powershell: detected=true, version=PowerShell 7.6.5
test tools::tests::embedded_registry_detects_all_five_tools ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.70s
```

Codex 与 pi 已检测到，但本机 `<bin> --version` 没有在约定条件下返回可用版本，因此按任务书显示“版本未知”。

### 格式检查

命令：

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

结果：退出码 0，无输出。

## 窗口 smoke

`npm run tauri dev` 已实际执行。当前受控命令环境在 Vite 启动 esbuild 子进程时返回：

```text
Error: spawn EPERM
The "beforeDevCommand" terminated with a non-zero status code.
```

分开启动前端与 Tauri 后，Rust 完成链接并运行 `target\debug\aicross.exe`；受控命令会话没有桌面窗口句柄。尝试按桌面控制流程启动同一 exe 时，桌面控制返回：

```text
Computer Use was not approved to use aicross
```

因此不能把以下项目记录为已完成的人工界面 smoke：

| 验收项 | 状态 | 已有证据 |
|---|---|---|
| 打开目录 | 未验证 | 后端路径校验、基线重建和最近项目逻辑已编译 |
| 启动台显示五项检测状态与版本 | 界面未验证 | 本机检测测试五项均 `detected=true` |
| PowerShell tab 输入回显 | 界面未验证 | Windows ConPTY 子进程与输出测试通过 |
| Claude tab 进入交互界面再关闭 | 未验证 | Claude 已检测；未自动操作终端界面 |
| 文件树展开 | 未验证 | 懒加载和项目内路径限制已编译 |
| 修改记录列出文件并显示 diff | 界面未验证 | 快照、修改识别和行级 diff 测试通过 |
| 记录 tab 列出最近会话 | 界面未验证 | 四家实际样本结构已核对；pi 元数据解析测试通过 |

没有通过界面自动化向终端输入命令；Windows 桌面控制规则禁止自动操作终端应用。需在普通桌面会话中补跑上述人工 smoke。

## 未做

- 未做第 1 期资源事实源与模型清单接入；资源 tab 仅显示“第 1 期接入”。
- 未做第 2 期方案与编排；该 tab 默认关闭，启用后仅显示占位页。
- 未做第 3 期派发与结果屏。
- 未做编辑器、对话全文查看、额度或美元核算。
- 未 push。

## 提交状态

计划使用 Conventional Commit：

```text
feat: build phase 0 desktop skeleton
```

当前受控环境只允许读取 `.git`。`git add` 与最小重试 `git add -- README.md` 均未修改索引，原始错误为：

```text
fatal: Unable to create 'D:/work/aicross/.git/index.lock': Permission denied
```

因此本次未能创建 commit；`tasks/log-00.err` 与 `tasks/log-00.jsonl` 仍为未跟踪文件，未纳入实现。

## 不确定处

- 当前环境无法完成真实窗口手测，窗口布局、系统主题、目录对话框和 xterm 交互仍需人工确认。
- `npm install` 的普通安装在当前沙箱执行依赖脚本时遇到 `spawn EPERM`；使用 `npm install --ignore-scripts` 后，TypeScript/Vite 构建成功。普通桌面 shell 的安装路径未在本次验证。
- debug 构建将 WebView 数据放入已忽略的 `.aicross-webview/`，避免受控环境无法写 AppData；release 构建使用系统默认目录。

## 修正轮 R1

### 审查发现处理结果

#### 1. 需求完整性

1. 窗口 smoke：**未验证**。本轮没有可用的桌面窗口控制，未用编译或单元测试替代人工界面验收。
2. `dist` 额外忽略：**已修**。保留 `.git / node_modules / target / dist` 的快照忽略规则；修改记录面板明确显示“已忽略 .git / node_modules / target / dist”。
3. PowerShell 忽略 `defaultArgs`：**已修**。默认 shell 只决定启动文件，启动参数统一使用工具注册表的 `default_args`；新增测试 `powershell_launch_uses_registered_default_args`。
4. `.git / node_modules / target` 无法展开：**不采纳：审查判断与代码行为不符**。`muted` 只控制置灰样式，所有 `isDir` 节点仍调用 `listDirectory` 并可展开；未增加禁止展开逻辑。
5. 默认开启的 tab 未加粗：**已修**。工作区、资源、记录、启动台、修改记录使用 `default-tab`；可选 tab 即使当前激活也不因此加粗。
6. 窄窗自动折叠：**不采纳：本轮要求明确为手动折叠即可**。保留手动收起与展开，不增加自动行为。
7. 打包图标缺失：**已修**。`src-tauri/icons/icon.png` 不再被 `.gitignore` 排除，`tauri.conf.json` 已引用该 PNG。
8. 未创建 commit：**不采纳：本轮明确要求不要 commit**。

#### 2. 逻辑正确性

1. 同步命令阻塞主线程：**已修**。工具检测、项目切换、目录读取、修改扫描、基线重置、diff、会话扫描、设置保存和终端启动均改为 async 命令；文件系统、进程检测和扫描工作由 `spawn_blocking` 执行。
2. Workspace 切项目复用旧 TreeNode：**已修**。项目路径作为工作区树根 key，切换时同时清空旧节点和选择状态。
3. 仅 mtime 变化误报修改：**已修**。`snapshot_content_changed` 仅以 SHA-256 hash 为修改判据；测试固定了 mtime 不同但 hash 相同时不产生修改。
4. PTY spawn 后 reader/writer 初始化失败泄漏进程：**已修**。两个失败分支都会结束子进程树并等待直接子进程退出，再返回中文前缀错误。
5. monitor 线程争用全局锁及锁中毒泄漏：**已修**。每个终端使用独立 `Arc<Mutex<Child>>`；monitor 轮询不再持有全局 records 锁，records 只在插入或移除时短暂加锁；中毒锁恢复内部值继续清理。
6. 快照整文件读入及文本内容无总量上限：**已修**。文件按 64 KiB 分块哈希；单文件文本保留上限 2 MB，总文本保留上限 16 MB，baseline/current 各自受限。
7. debug WebView 数据污染项目：**已修**。debug 数据目录改为系统应用本地数据目录下的 `webview-debug`，不再创建或更新项目内 `.aicross-webview`。旧版本留下的本地缓存目录仍在磁盘；受控环境拒绝递归删除，本轮未删除该历史缓存。
8. Windows canonicalize 带 `\\?\` 前缀：**已修**。项目路径与安全路径统一使用 `dunce::canonicalize`；读取旧 recent 记录时同时去除既有 verbatim 前缀。
9. 会话按 timestamp 字符串排序：**已修**。RFC 3339 时间先转换为 `DateTime<Utc>` 再排序；新增带时区偏移的排序测试。
10. 关闭终端只杀直接子进程：**已修（Windows）**。关闭、异常初始化和管理器释放时先执行 `taskkill /PID <pid> /T /F` 清理进程树，再调用 portable-pty 的 `kill` 与 `wait` 兜底；其他平台保留 portable-pty 的直接子进程清理。

#### 3. 边界情况

1. `--version` 管道死锁：**已修**。stdout/stderr 在进程启动后立即由两个线程并发读取，进程等待与读取线程都有超时；Windows `where` 结果优先选择 `.exe/.com/.cmd/.bat`，避免选中无扩展脚本。最终测试读出 `codex-cli 0.149.1` 与 pi `0.84.2`。
2. 大文本误标二进制：**已修**。二进制只由 NUL 或无效 UTF-8 判定；超过保留上限的有效 UTF-8 显示“文本内容未保留”，并保持 `binary=false`。新增大文本测试。
3. symlink/junction 处理：**已修**。文件树返回 `isSymlink` 并显示链接标记；项目内链接可跟随，快照跟随链接但对每个入口 canonicalize，项目外目标不列出、不扫描。路径测试用 Windows 目录联接实际验证项目外目标被拒绝。
4. 默认 shell 到启动时才报错：**已修**。启动 PowerShell 工具前先解析 `settings.default_shell`，未检测到时返回“未检测到默认 shell：…”；不会进入 PTY spawn。
5. 垃圾会话与重复会话：**已修**。没有识别出任何白名单字段的 jsonl 返回 `None`；最终列表按 `(tool, id)` 去重后截断。新增垃圾文件、时区排序和重复 id 测试。
6. `tools.json` 解析失败静默忽略：**已修**。文件不存在仍使用内置注册表；读取或 JSON/字段解析失败返回带文件路径的明确错误。覆写合并测试覆盖同 id 替换、新 id 添加和缺字段报错。

#### 4. 安全

1. CSP 关闭：**已修**。配置基本 `default-src / connect-src / img-src / style-src / font-src` CSP。
2. 可执行文件直接打开：**已修**。`.exe/.bat/.cmd/.ps1/.msi` 大小写不敏感匹配，调用系统打开前显示原生确认框；新增扩展名测试。
3. capabilities 权限面：**不采纳：审查未发现过宽权限，当前权限未改**。
4. 环境变量泄漏：**不采纳：审查未发现泄漏路径，当前仍只在本次前端状态与 IPC/子进程环境中传递**。
5. shell 注入：**不采纳：审查确认全程使用 argv，无 shell 字符串拼接；当前实现未增加拼接执行**。
6. 会话敏感内容落盘：**不采纳：审查确认只读取白名单元数据且不落盘；当前行为保持不变**。

#### 5. 代码质量

1. `resolve_binary` 平台分支重复：**已修**。查找命令名按平台选择，其余解析逻辑合并。
2. `lib.rs` 锁错误样板重复：**已修**。抽出 read/write/mutex lock helper，并统一中文操作前缀。
3. Changes 重置无异常处理：**已修**。reset 使用 try/catch/finally、busy 状态，失败不再清空现有结果。
4. Workspace 展开无异常处理：**已修**。目录加载失败显示错误并保持折叠；下次成功清除错误。
5. 成功后旧错误不清除：**已修**。App、Workspace、Sessions、Changes 的成功路径清除对应旧错误。
6. 终端主题不跟随系统切换：**已修**。监听 `prefers-color-scheme` change，实时更新 xterm theme，并在卸载时移除监听。
7. “环境变量”按钮与英文后端错误：**已修**。按钮改为“配置变量”；后端 I/O、序列化、锁、PTY、路径和打开操作均增加中文操作前缀，原始错误文本保留在冒号后。
8. Workspace 与 Changes 的项目切换重挂载不一致：**已修**。两者都按项目路径重建项目相关视图状态。

#### 6. 测试覆盖

1. 路径越界：**已补**。覆盖空路径、真实存在的 `..` 穿越、绝对路径注入、项目外 Windows junction/symlink。
2. `tools.json` 覆写合并：**已补**。覆盖同 id 覆盖、新 id 添加、字段缺失时报错。
3. compare/diff 边界：**已补**。覆盖新增、删除、二进制大小变化、仅 mtime 变化不算修改、`@@` 头行号计数。
4. 其他回归：**已补**。增加默认参数、可执行扩展名确认、大 UTF-8 文本、会话时区排序/去重/垃圾跳过测试；保留原有工具检测、PTY、文本 diff 和 pi 元数据测试。

### 最终自动验证

#### Rust 检查

命令：

```text
cargo check --manifest-path src-tauri/Cargo.toml
```

结果原文：

```text
    Checking aicross v0.0.0 (D:\work\aicross\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.96s
```

#### Rust 测试

命令：

```text
cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture
```

结果原文：

```text
warning: linker stdout: 正在创建库 D:\work\aicross\src-tauri\target\debug\deps\aicross_lib.dll.lib 和对象 D:\work\aicross\src-tauri\target\debug\deps\aicross_lib.dll.exp
  |
  = note: `#[warn(linker_messages)]` on by default

warning: `aicross` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.87s
     Running unittests src\lib.rs (src-tauri\target\debug\deps\aicross_lib-e6e129a316898a1b.exe)

running 11 tests
test tests::executable_extensions_require_confirmation_case_insensitively ... ok
test workspace::tests::compare_and_diff_cover_added_deleted_mtime_binary_and_hunk_counts ... ok
test sessions::tests::pi_parser_keeps_only_requested_metadata ... ok
test sessions::tests::sessions_sort_by_instant_deduplicate_and_skip_garbage ... ok
test config::tests::tools_overrides_merge_existing_and_new_entries_and_report_invalid_json ... ok
test workspace::tests::snapshot_reports_modified_text_and_diff ... ok
test workspace::tests::safe_path_rejects_traversal_absolute_paths_and_external_symlinks ... ok
test pty::tests::portable_pty_runs_a_child_and_reads_output ... ok
test workspace::tests::large_utf8_text_is_not_reported_as_binary ... ok
test pty::tests::powershell_launch_uses_registered_default_args ... ok
claude: detected=true, version=2.1.251 (Claude Code)
codex: detected=true, version=codex-cli 0.149.1
kimi: detected=true, version=0.39.1
pi: detected=true, version=0.84.2
powershell: detected=true, version=PowerShell 7.6.5
test tools::tests::embedded_registry_detects_all_five_tools ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.28s

     Running unittests src\main.rs (src-tauri\target\debug\deps\aicross-a58949d6294ae227.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests aicross_lib

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

#### 格式检查

命令：

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

结果：退出码 0，无输出。

#### 前端构建

命令：

```text
cd src-ui
npm run build
```

结果原文：

```text
> aicross-ui@0.0.0 build
> tsc && vite build

(node:19940) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v7.3.6 building client environment for production...
transforming...
✓ 46 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                   0.45 kB │ gzip:   0.29 kB
dist/assets/index-B3MFdQEo.css   14.65 kB │ gzip:   3.75 kB
dist/assets/index-B033gBCI.js   502.72 kB │ gzip: 139.80 kB

(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
✓ built in 1.54s
```

构建成功；502.72 kB 的 chunk 体积警告仍在，不影响本轮功能。

### 补充检查

`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 通过。

占位 PNG 已通过 Tauri 的应用构建和图标处理阶段。完整 `npm run tauri -- build --debug` 首次被受控环境的 esbuild `spawn EPERM` 中断；复用已生成的前端文件重试后生成了 `target\debug\aicross.exe`，并进入 MSI 图标处理与 WiX 校验，随后因 WiX 解压目录“拒绝访问。 (os error 5)”停止。因此图标配置已验证，安装包生成仍为**未验证**。

### 窗口 smoke

仍无可用桌面窗口控制，打开目录、启动台、PowerShell/Claude tab、文件树、diff 与会话记录的人工窗口 smoke 均为**未验证**。

### 提交状态

按本轮要求未 commit、未 push。

## 修正轮 R2

### 处理结果

1. 配置损坏：`settings.json` 与 `recent.json` 读取或解析失败时分别回退默认设置和空的最近项目；`tools.json` 失败时回退内置工具注册表。警告通过初始状态传到 UI 顶部，内容包含对应文件路径。`write_json` 改为在同目录写唯一临时文件、同步内容后 `rename` 替换目标，失败时清理临时文件。新增 `damaged_settings_do_not_prevent_loading` 测试；无效 `tools.json` 测试改为验证内置注册表回退和警告。
2. 终端关闭阻塞：`close_terminal` 改为 async 命令，进程树清理整体由 `spawn_blocking` 执行。
3. 修改记录残留：`Changes` 在 `projectPath` 变化时清空 `changes`、`diff`、忙碌状态和错误；异步刷新、重置与 diff 使用项目版本检查，旧项目请求完成后不会回写新项目界面。
4. 关闭 tab 与幽灵终端：前端仅在 `api.closeTerminal` 成功后移除 tab，失败时保留 tab 并显示错误。后端用同一把状态锁管理 `opening / records / cancelled`；实际 PTY 启动和进程清理在锁外执行。关闭先到、启动中关闭和启动完成后关闭均按序处理，取消后的新进程会立即清理，不插入终端记录。
5. 浅色终端：浅色 xterm 使用 `#f5f6f8` 背景、`#171a21` 前景和 `#2c4a8c` 光标；深色配置保持不变。`.terminal-host` 背景改为跟随 `--terminal-bg` 主题变量。

### Rust 检查

命令：

```text
cargo check --manifest-path src-tauri/Cargo.toml
```

结果原文：

```text
    Checking aicross v0.0.0 (D:\work\aicross\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.93s
```

### Rust 测试

命令：

```text
cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture
```

结果原文：

```text
   Compiling aicross v0.0.0 (D:\work\aicross\src-tauri)
warning: linker stdout: 正在创建库 D:\work\aicross\src-tauri\target\debug\deps\aicross_lib.dll.lib 和对象 D:\work\aicross\src-tauri\target\debug\deps\aicross_lib.dll.exp
  |
  = note: `#[warn(linker_messages)]` on by default

warning: `aicross` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 10.76s
     Running unittests src\lib.rs (src-tauri\target\debug\deps\aicross_lib-e6e129a316898a1b.exe)

running 12 tests
test tests::executable_extensions_require_confirmation_case_insensitively ... ok
test workspace::tests::compare_and_diff_cover_added_deleted_mtime_binary_and_hunk_counts ... ok
test config::tests::damaged_settings_do_not_prevent_loading ... ok
test sessions::tests::sessions_sort_by_instant_deduplicate_and_skip_garbage ... ok
test config::tests::tools_overrides_merge_existing_and_new_entries_and_report_invalid_json ... ok
test sessions::tests::pi_parser_keeps_only_requested_metadata ... ok
test workspace::tests::snapshot_reports_modified_text_and_diff ... ok
test workspace::tests::safe_path_rejects_traversal_absolute_paths_and_external_symlinks ... ok
test pty::tests::portable_pty_runs_a_child_and_reads_output ... ok
test workspace::tests::large_utf8_text_is_not_reported_as_binary ... ok
test pty::tests::powershell_launch_uses_registered_default_args ... ok
claude: detected=true, version=2.1.251 (Claude Code)
codex: detected=true, version=codex-cli 0.149.1
kimi: detected=true, version=0.39.1
pi: detected=true, version=0.84.2
powershell: detected=true, version=PowerShell 7.6.5
test tools::tests::embedded_registry_detects_all_five_tools ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.45s

     Running unittests src\main.rs (src-tauri\target\debug\deps\aicross-a58949d6294ae227.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests aicross_lib

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 格式检查

命令：

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

结果原文：退出码 0，无输出。

### 前端构建

命令：

```text
cd src-ui
npm run build
```

结果原文：

```text
> aicross-ui@0.0.0 build
> tsc && vite build

(node:13968) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v7.3.6 building client environment for production...
transforming...
✓ 46 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                   0.45 kB │ gzip:   0.28 kB
dist/assets/index-D3VFecDb.css   14.90 kB │ gzip:   3.78 kB
dist/assets/index-DDekUYT_.js   503.15 kB │ gzip: 139.91 kB

(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
✓ built in 1.51s
```

构建成功；503.15 kB 的 chunk 体积警告仍在，不影响本轮功能。

### 提交状态

按本轮要求未 commit、未 push。
