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
