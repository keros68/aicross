# REPORT-88 顶栏显示客户端版本号与手动检查更新

## 改了什么

- Rust 侧（`src/server.rs`）：
  - 新增 `app_version` 命令，直接返回可执行文件包版本号，窗口挂载即刻可读、不走网络。
  - `app_update_check` 改为返回 `AppUpdateStatus { current, available }`（`available` 为 `null` 或 `{ version, notes }`），保留失败原因返回给前端。
- 前端（`src-ui/`）：
  - `src-ui/src/tauri.ts`：新增 `commands.appVersion` 与 `AppUpdateStatus` / `AppUpdateAvailable` 类型。
  - `src-ui/src/components/common/app-update-view.ts` 与 `AppUpdate.tsx`：顶栏右侧常驻版本号按钮（如 `v0.3.11`），检测到新版时切换为主按钮「更新到 X」（下载中显示进度，安装失败显示「更新失败，重试」）。点击展开浮层，显示当前版本、检查结果（「已是最新版本」/「正在检查更新…」/ 失败原因 / 可更新版本及说明），提供「检查更新」按钮与（有新版时的）「更新」按钮；启动自动检查失败时顶栏保持安静不打扰，点开浮层可查看原因并重试。
  - `src-ui/src/styles/global.css`：新增 `.app-update`、`.topbar-version`、`.app-update-pop` 等样式。
  - 单元测试：`src-ui/src/components/common/__tests__/app-update-view.test.ts`、`src-ui/src/__tests__/tauri-commands.test.ts`、`src/server.rs` 内 `app_update_status_serializes_current_and_optional_target`。
- 文档（`docs/MANUAL.md`）：同步更新「软件更新」一节说明。

## 验证

```
python scripts/copy-audit.py
→ 新增与改动文件 0 处命中（全仓共 16 处既有命中，8 个既有文件）

cd src-ui && npm test && npm run lint && npm run build
→ Test Files  24 passed (24)
→ Tests  262 passed (262)
→ eslint . 无输出
→ tsc -b && vite build → ✓ built in 422ms

cargo check && cargo test --lib --bins
→ Checking aicross v0.3.11 ... Finished `dev` profile
→ test result: ok. 819 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 72.23s
```

## 没做 / 注意

- 未将 GitHub 更新检查绑入顶栏「刷新」按钮，避免离线或 GitHub 访问慢时拖慢本机页面刷新或报「刷新失败」。
- 未启动桌面窗口做手工点击截图验证，也未修改版本号、未重新打安装包（本机当前安装的 `C:\Users\keros68\AppData\Local\aicross\aicross.exe` 仍是此前打包的 0.3.11，重新打包安装或运行 `npx --yes -p @tauri-apps/cli@2 tauri dev` 后可看到新顶栏入口）。
