# AGENTS.md — aicross

本文件是仓库约定，所有写代码的 agent 先读完再动手。

## 项目是什么

aicross：多厂商 agent 的派发引擎与结果查看器。用户在自己常用的宿主（Claude Code / Codex / Kimi Code / pi agent 等，装 ai-cross skill）里说话，模型提议派发、用户采纳；aicross 以命令行拉起各家 CLI、隔离盲审、核对真身、留痕，并在查看器里把各路结果并排交给用户裁决。现行产品说明在 `docs/PRODUCT.md`；08-30 至 09-30 的定案原文在 `docs/history/PRODUCT-0830-0930.md`，只作追溯。`docs/history/proto/index.html` 是 09-30 之前的界面原型，只作历史参考。

## 技术栈

- Tauri 2（Rust 后端）+ React 19 + TypeScript + Vite 前端。窗口是自绘标题栏加五页：开始、资源、skill、模式、派发记录（含查看页）；终端、PTY、xterm 已随 TASK-40 下线。
- 目录：`src/`（Rust，`Cargo.toml` 在根目录）、`src-ui/`（前端）、`skills/ai-cross/`（ai-cross skill 源码）、`skill-bundle/`（由 `scripts/sync-skill.py` 从 `skills/ai-cross` 生成、编进可执行文件的内置包）、`windows/`（安装器的 PATH 处理）、`tasks/`（任务书与完成报告）、`docs/`。
- Windows 优先（开发机是 Windows 11），macOS/Linux 不能破坏但不强求验证。
- 现有 Python 脚本（`skills/ai-cross/references/cc_switch.py` / `verify_model.py` / `usage_probe.py`）当子进程调用，不重写。

## 许可与来源

MIT（`LICENSE`）。仓库代码均为本项目自写；10-25（TASK-64）起不含 Coffee CLI 的代码，`D:\work\coffeecli` 只作历史参考，不从中复制文件或函数。早期提交里有它的代码（AGPL-3.0），只存在于历史中。

## 纪律

- 密钥：只按子进程环境变量传；不回显、不落盘到本仓库任何文件、不写日志。读 `~/.pi/auth.json`、`~/.cc-switch/*.db` 之类凭据文件前，任务书里必须有明文授权。
- CLI 参数与模型 ID 会漂移：本地有事实源的运行时读（`~/.codex/models_cache.json`、`~/.kimi-code/config.toml`、`~/.pi/agent/models-store.json`），不手抄进代码。
- 多行 prompt 绝不走 argv（Windows 在第一个换行截断），走 stdin 或文件。
- 退出码不可信：`claude -p` 出错可能 exit 0，pi 正确应答可能 exit 1。判定成功看输出体。
- 界面文案：中性简短说明体；按钮是 2–6 字动词短语（打开 / 刷新 / 接入 / 冒烟）；状态词与动作词成对（已接入 ↔ 移除）；不用"门禁 / 落库 / 作用域"这类内部词；不拟人化；不用 emoji。
- 不做的事：编辑器、通用 agent 框架、新协议、额度/美元核算、"agent 在跑"的动画。
- 界面文案与命名的完整规范见下一节；新增任何用户可见字符串前先读它。

## 界面文案与命名规范（2026-09-15 用户裁定）

准星：中性、简短的说明体。完整规则与对照表沿用 `D:\work\skillmanager\AGENTS.md`「界面文案规范」与 `C:\Users\keros68\.codex\guidance\copywriting.md`，这里只写本产品的命名决定。

- **同类项一套模板。** 预设一律「X模式」：共享模式 / 经济模式 / 盲审模式 / 擂台模式 / 评审模式 / 蜂群模式；用户自建的预设叫「自定义模式」（多个时「自定义模式 · 名称」），不加「（我的）」；规划中的写「X模式（规划中）」。角色名两字：出方案 / 执行 / 验收 / 验证 / 拆分 / 顾问 / 仲裁。
- **一个概念一个词。** 派发 / 合并 / 采纳 / 重派 / 叫停 / 接入 / 移除 / 验证；同一动作不在不同页面换词（「冒烟」与「验证」、「接入」与「添加」不得并存）。
- **按钮 = 动词短语 2–6 字**；**标签 = 名词短语**；**状态词与动作词成对**（已接入 ↔ 移除、已并入 ↔ 合并、未验证 ↔ 验证）。
- **说明句 = 事实 + 后果**，一句说完；同一要点只说一遍；条件→能力用「……后可……」。
- **不用内部词**：门禁 / 落库 / 口径 / 判定 / 闸门 / 留痕 / 对账 / wire / G1–G4 / 任务编号，界面上一律展开成读者能懂的话（留痕→记录，对账→核对，G3→结果）。
- **标点全角**（，。；：？！（））；半角只留在路径、命令、代码标识符、版本号。
- **不拟人化，不用 emoji，不用英文式破折号。**
- 术语表在 `docs/MANUAL.md` 末尾维护（概念 → 唯一用词 → 出现位置）；改文案时按表复核，不只改被指出的那一处。

## 命令

```bash
cd src-ui && npm install && npm run build     # 前端
cargo check && cargo test                     # 后端（根目录）
npm exec @tauri-apps/cli@2 -- tauri dev      # 开发窗口（根目录；tauri CLI 未装为本地依赖）
```

## 交付流程

1. 每期一份任务书 `tasks/TASK-NN-*.md`，实现者读完再动手。
2. 完成后写 `tasks/REPORT-NN.md`：改了什么、怎么验证的（命令与结果原文）、没做什么、不确定处。没跑过的不要写"已验证"。
3. Conventional Commits，英文；一期一个或几个 commit，不 push（push 由用户做）。
4. `node_modules/`、`target/`、`dist/` 进 `.gitignore`。
