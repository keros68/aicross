# REPORT-64 去掉上游代码，改 MIT；ai-cross skill 并入本仓

## 改了什么

上游代码（逐文件与 `D:\work\coffeecli` 对比定位）：

- 删除 `src/git.rs`、`src/tool_config.rs`、`src/windows_path.rs`。未提交改动数改为 `dispatch::git_dirty_count`（`git status --porcelain=v1 --untracked-files=all`）；手动 agent 的可执行文件改为 `resources::manual_agent_binary`；启动时的 PATH 与 GDK 处理改为 `src/startup_env.rs`。Linux 不再按 WebKit 版本自动切 X11，改为设 `AICROSS_FORCE_X11=1` 时才切；不再调高文件句柄上限。
- `server.rs` 窗口部分重写：三个窗口命令合为 `window_action`，前台显示统一走 `bring_main_window_to_front`；单实例转发、3 秒兜底显示、Windows 直角、macOS 关窗隐藏行为不变。
- `process.rs` 的 Job Object 段重写。
- 前端：`WindowFrame.tsx`（窗口按钮与缩放手柄）替换 TitleBar / ResizeEdges；`lib/os.ts` 替换 `lib/platform.ts`；`main.tsx`、`tauri.ts` 的 invoke 封装、`vite.config.ts`、`global.css` 的基础段与窗口按钮样式重写。
- 安装器：删 `installer.nsi`，改用 Tauri 默认模板加 `installerHooks`（`windows/nsis-hooks.nsh` 调 `windows/path-entry.ps1`）。
- 删 `scripts/rebuild-icons.py`、`src-ui/resize_icon.js`、`NOTICE`、`TRADEMARKS.md`；`LICENSE` 换 MIT；`docs/proto/` 移到 `docs/history/proto/`；`.gitignore`、`Cargo.toml` 注释重写；去掉未使用的 `tauri-plugin-global-shortcut`。版本 0.2.0。

skill 并入：`git subtree split` 取 xiaoyu-skill 中 `skills/ai-cross` 的 33 个提交，`git subtree add` 到 `skills/ai-cross`；`scripts/sync-skill.py` 默认源改为本仓 `skills/ai-cross`，内置包重新同步（除 README 外只有换行符从 CRLF 变 LF）。skill README 的安装命令改为 `keros68/aicross`。

## 验证

```
cd src-ui && npx vitest run   → Test Files 22 passed, Tests 240 passed
npm run build                 → built in 889ms
npx tsc -b; npx eslint src    → 无输出
cargo test                    → 694 passed, 10 ignored
cargo check --release         → 无告警（单实例分支只在发布构建编译）
skills/ai-cross: python -m unittest discover -s tests → Ran 70 tests, OK
npx skills add D:\work\aicross --list → Found 1 skill: ai-cross
```

对比：上游全部源文件中长度 ≥ 40 的行与本仓编译代码逐行比对，剩余命中均为 API 惯用写法（`useState`、`addEventListener`、`tauri::Builder::default()` 之类）与 Tauri / Vite 官方模板配置。

真机（v0.2.0 NSIS 安装包，静默安装到当前用户）：
- 安装后用户 PATH 末尾多出 `C:\Users\keros68\AppData\Local\aicross`，原有内容与值类型（REG_SZ）不变；卸载后与安装前逐字相同；重装后又加回。
- 安装版带调试端口启动：首帧后显示；最大化 / 还原（最大化时缩放手柄隐藏）、双击页头切换最大化、最小化后再次启动唤回同一窗口（进程数 1）、真实鼠标拖页头移动窗口、拖右下角缩放、`aicross open <run_id>` 转发到已开窗口进入查看页（命令退出 0）、关闭按钮结束进程，均通过。
- `aicross config show --host claude` 读到 `D:\aicross-data` 的配置，退出 0。
- `windows/path-entry.ps1` 另在临时注册表键上测过：首次添加、重复添加不变、`%USERPROFILE%` 不展开、REG_EXPAND_SZ 与 REG_SZ 类型保留、移除时忽略大小写与结尾 `\`。

## 发现

0.1.2 及之前的安装包从未把安装目录写进 PATH：内联的 `-EncodedCommand` 约 1300 字符，超过 NSIS 单条字符串 1024 字符上限，被截断后 PowerShell 执行失败（本机装着 0.1.2，用户 PATH 里没有安装目录）。新钩子改为把脚本文件打进安装包再 `-File` 执行。MSI 走 WiX 的 Environment 元素，不受影响，本次未重测。

## 没做

- 历史提交里仍有上游代码（AGPL-3.0），不改写历史。
- 本机 `port/`（已被 .gitignore 排除、从未公开）未删。
- macOS / Linux 未验证；Linux 自动切 X11 的逻辑已去掉，个别驱动白屏时需设 `AICROSS_FORCE_X11=1`。
