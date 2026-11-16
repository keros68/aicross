# REPORT-84（窗口与打包）：macOS 桌面端

按 `tasks/TASK-84-macos-desktop.md` 的 B 部分实施，由 Sonnet 在独立工作副本里完成，合并时改了一处。没有改 `src/`。

## 没验证的

- 窗口左上角的系统按钮没有在截图里看到：截图工具自己的标记盖住了那个位置。位置只由布局确认（顶栏左侧留出 88px）。
- 拖动与缩放窗口没有实测。
- `scripts/build-macos.sh` 里有私钥时生成更新包并写 `latest.json` 的那一段只通过了 `bash -n`，本机没有私钥，没有运行过。
- 更新清单里没有 macOS 一项时窗口不提示，这条依据是读代码（更新插件返回找不到平台，`AppUpdate.tsx` 丢弃这个错误），开发窗口里也没有出现提示；没有用真实的 `latest.json` 端到端测过。
- 最后一次重新打包只改了前端，`.dmg` 没有再跑 `hdiutil verify`。

## 改了什么

- `scripts/build-macos.sh`：只在 Apple Silicon 的 macOS 上运行；没有更新签名私钥时关掉更新包、照常出 `.app` 与 `.dmg`；有私钥和密码时生成签名的更新包，并把 `darwin-aarch64` 一项并进 `latest.json`。
- `src-ui/src/main.tsx`：macOS 上另设 150 ms 的计时器显示窗口。隐藏窗口里 WKWebView 不触发动画帧，原来要等 Rust 侧 3 秒的兜底才显示。计时器只在 macOS 上设（合并时加的条件：Windows 上首帧慢于 150 ms 时会先露出空白窗口）。
- `tauri.macos.conf.json`：窗口 1200×800，最小 800×560。
- `docs/MANUAL.md`、`docs/PRODUCT.md`：macOS 的安装、首次打开的放行步骤、命令行进 PATH 的一条 `ln -s`。
- `tasks/assets/task-84/`：五页与安装版窗口的截图。

## 验证

- `tauri dev` 打开窗口，五页都能显示，没有重复的标题栏，圆角由系统画。
- `scripts/build-macos.sh` 走没有私钥的那条路，产物 `target/aarch64-apple-darwin/release/bundle/dmg/aicross_0.3.11_aarch64.dmg`，4,485,059 字节；`.app` 11.12 MiB，只有链接器的临时签名。
- 打出的 `.app` 拷到临时目录能启动，启动日志为空。
- 经符号链接启动、临时 `AICROSS_HOME`：`aicross --version` 输出 `aicross 0.3.11`，`aicross config show` 输出配置 JSON，退出码都是 0。
- `npm run build` 通过，`npm test` 256 条通过。

## 没做什么

- `README.md` 第 13、84 行仍写只支持 Windows。
- 没有装 `llvm-tools`：打包去调试信息时有一条 `libLLVM.dylib` 的警告，不影响产物。

## 不确定处

- `latest.json` 合并时用这次打包的版本号与时间覆盖原值，两个平台要在同一个版本上打包。
- 手册里「仍要打开」是 macOS 惯用的按钮文字，macOS 27 上可能略有出入。
- `npm exec @tauri-apps/cli@2 -- tauri` 在这台机器上报「unrecognized subcommand 'tauri'」，脚本与验证用的是 `npx --yes -p @tauri-apps/cli@2 tauri`；`AGENTS.md` 里写的是前一种。
