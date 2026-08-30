# AGENTS.md — aicross

本文件是仓库约定，所有写代码的 agent 先读完再动手。

## 项目是什么

aicross：多厂商 agent 的桌面派发控制台。用户在一个窗口里接入本机的 agent CLI（Claude Code / Codex / Kimi Code CLI / pi 等），看到调用方案，手动编排，在对话区 @ 各家模型干活；模型提议、用户裁决、控制台执行。产品说明在 `D:\work\agentteam\PRODUCT-console.md`，界面基准是 `D:\work\agentteam\console-proto\index.html`（可点击原型，假数据）。两者只读，不在本仓库改。

## 技术栈

- Tauri 2（Rust 后端）+ React 19 + TypeScript + Vite 前端；终端用 `@xterm/xterm`，pty 用 `portable-pty`。
- 目录：`src-tauri/`（Rust）、`src-ui/`（前端）、`tasks/`（任务书与完成报告）、`docs/`。
- Windows 优先（开发机是 Windows 11），macOS/Linux 不能破坏但不强求验证。
- 现有 Python 脚本（`D:\work\agentteam\dist\ai-cross\references\cc_switch.py` / `verify_model.py` / `usage_probe.py`）当子进程调用，不重写。

## 与 coffeecli 的关系（必须遵守）

`D:\work\coffeecli` 是第三方项目（AGPL-3.0）。**可以读它的源码学结构、可以借鉴形态、可以顺手记下它的 bug（记到 `docs/coffeecli-notes.md`）；不得复制它的任何文件或函数。** 本仓库每个文件都自己写。本仓库 license 为 AGPL-3.0，`NOTICE` 注明形态参考自 Coffee CLI。

## 纪律

- 密钥：只按子进程环境变量传；不回显、不落盘到本仓库任何文件、不写日志。读 `~/.pi/auth.json`、`~/.cc-switch/*.db` 之类凭据文件前，任务书里必须有明文授权。
- CLI 参数与模型 ID 会漂移：本地有事实源的运行时读（`~/.codex/models_cache.json`、`~/.kimi-code/config.toml`、`~/.pi/agent/models-store.json`），不手抄进代码。
- 多行 prompt 绝不走 argv（Windows 在第一个换行截断），走 stdin 或文件。
- 退出码不可信：`claude -p` 出错可能 exit 0，pi 正确应答可能 exit 1。判定成功看输出体。
- 界面文案：中性简短说明体；按钮是 2–6 字动词短语（打开 / 刷新 / 接入 / 冒烟）；状态词与动作词成对（已接入 ↔ 移除）；不用"门禁 / 落库 / 作用域"这类内部词；不拟人化；不用 emoji。
- 不做的事：编辑器、通用 agent 框架、新协议、额度/美元核算、"agent 在跑"的动画。

## 命令

```bash
cd src-ui && npm install && npm run build     # 前端
cargo check --manifest-path src-tauri/Cargo.toml   # 后端
npm run tauri dev                             # 开发窗口
```

## 交付流程

1. 每期一份任务书 `tasks/TASK-NN-*.md`，实现者读完再动手。
2. 完成后写 `tasks/REPORT-NN.md`：改了什么、怎么验证的（命令与结果原文）、没做什么、不确定处。没跑过的不要写"已验证"。
3. Conventional Commits，英文；一期一个或几个 commit，不 push（push 由用户做）。
4. `node_modules/`、`target/`、`dist/` 进 `.gitignore`。
