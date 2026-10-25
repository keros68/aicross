# REPORT-65 skill 升级、软件自动更新、扩充主 agent

## 改了什么

- skill 升级：`skill_install::upgrade_at`（命令行 `aicross skill upgrade --host --path`，窗口 `skill_upgrade`）。开始页与 skill 页对别的软件装的旧版显示「需要升级到 X」与「升级」，确认框说明会备份；一键配好包含升级。启动时自动更新 aicross 自己装的旧版（`autoUpdateHosts`）。
- 开始页缺人提示 `setupHint`：只取审与干活两个模式的说明，去重、改写成一句话，给「去验证」「手选人选」；样式不再溢出卡片，左对齐。
- 软件自动更新：`tauri-plugin-updater`，命令 `app_update_check` / `app_update_install`，顶栏 `AppUpdateButton`；`tauri.conf.json` 写入公钥与 `releases/latest/download/latest.json`；`scripts/build-release.ps1` 用本机私钥签名并生成 latest.json。私钥 `~/.tauri/aicross-updater.key`，密码以 DPAPI 存在 `~/.tauri/aicross-updater-password.xml`，都不在仓库。
- 主 agent：`hosts::EXTRA_HOSTS` 九家，探测时写入资源库 `extraHosts`；skill 位置、调用写法、厂商见 `docs/MANUAL.md`「主 agent」。短命令卡片与「安装到项目」对它们隐藏。
- 主 agent 菜单厂商认不出时不显示。版本 0.3.0。

## 依据

skill 目录：OpenCode、Qoder、CodeBuddy 取官方文档；Antigravity 取 Google codelab（`~/.gemini/antigravity/skills`）；Gemini CLI、Hermes 取文档与多个二手来源一致的说法；WorkBuddy 取本机 `~/.workbuddy/.skill-list-cache.json` 的扫描目录；ZCode 取 skillmanager 源码里 08-16 的本机实测（读 `~/.agents/skills`）。Hermes、Qoder 的 `/ai-cross` 写法有文档；其余家的显式调用写法未核实，示例用自然语言。

## 验证

```
cargo test                         → 698 passed, 10 ignored
src-ui: npx vitest run             → 245 passed
npx tsc -b; npx eslint src         → 无输出
```

真机：
- 更新器：以本机地址为更新源打 0.3.0 与 0.3.1 两个签名包，装 0.3.0 后顶栏出现「更新到 0.3.1」，确认框内容正确；随后下载 0.3.1、安装，注册表版本变为 0.3.1，程序自动重新打开，PATH 里 aicross 仍只一项。点击确认的那一步，我的脚本报「找不到元素」但下载已开始，推测是确认框打开时焦点在按钮上被触发，未进一步查。
- 正式 0.3.0 包（GitHub 更新源）：GitHub 上还没有 latest.json，顶栏不显示更新，符合预期。
- 探测后主 agent 列表：claude、codex、kimi、pi、opencode、agy、hermes、zcode、codebuddy、workbuddy；Qoder 未装（PATH 里的目录已不存在），未列出，正确。
- 在 OpenCode 的 skill 目录放一份假 1.12.1，开始页显示「由其他软件管理 · 1.12.1」「需要升级到 1.23.0」「升级」；确认后备份到 `~/.config/opencode/.aicross-skill-backup/ai-cross-20261025-202218`，新版 1.23.0 写入，状态变「已安装 · 1.23.0」。测试文件随后删除，OpenCode 目录恢复原状。

## 没做 / 注意

- 0.2.0 及更早的版本没有更新器，要手动装一次 0.3.0。
- 发布 0.3.0 时 latest.json 必须随 Release 上传，之后每版都要上传，否则已装的版本收不到更新。
- 安装包从约 2.0 MB 增到 2.7 MB（更新器带入 HTTP 与签名校验依赖）。
- 扩展主 agent 的派发不走它们自己（不是派发通道）；除 Gemini CLI 外厂商需用户在模式页设定。
