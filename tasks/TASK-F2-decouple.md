# TASK-F2 — 切断与上游的运行时冲突

## 背景

F1 改完了品牌字符串，但 fork 与上游 Coffee CLI **共用同一批运行时标识**。用户机器上 Coffee CLI 正在运行，本程序现在跑起来会和它抢文件、抢凭据、抢 hook 注册。

**在本期完成前不要启动本程序的窗口。**

先读根目录 `AGENTS.md`（本仓库是 Coffee CLI 的 fork，四条义务写在那里）与 `tasks/REPORT-F1.md` 的「C 节保留项」——本期做的就是那份清单里被判为「涉及迁移与业务逻辑，留待后续」的部分。

## A. 停掉启动时的 hook 自动安装（最优先）

`src/server.rs:3843` 在 Tauri setup 钩子里无条件调用 `hook_installer::install_all()`。它写入用户的真实 CLI 配置：

| 文件 | 写什么 |
|---|---|
| `~/.claude/settings.json` | 5 个事件的 hook，指向本程序 exe |
| `~/.claude/settings.local.json` | 清理旧条目 |
| `~/.codex/config.toml` | `notify = ["<exe>", "__codex-notify"]` |
| `~/.kimi-code/config.toml` | 9 条 `[[hooks]]` |
| `~/.config/opencode/plugins/` | 插件脚本 |
| Hermes | 插件与 `hermes plugins enable` |

后果：本程序一启动就把用户 Claude Code / Codex / Kimi 的 hook 注册从 Coffee CLI 抢过来。这既会破坏用户在用的上游程序，也违反本项目铁律——**未经用户明确同意，绝不写 `~/.claude/settings.json` 或任何全局 CLI 配置**。

改法：

1. **删掉 `src/server.rs:3843` 的 `install_all()` 调用。** 启动时一律不写外部配置。
2. **删掉 `src/server.rs:121` 的 `install_for_tool()` 自动调用**（那是启动台重扫时触发的）。
3. 保留 `hook_installer` 模块本身与它的全部函数和测试，**只断掉自动触发**。改成一条已有的 Tauri command 暴露出去（沿用现有命令注册方式），前端**本期不加入口**——先确保没有任何代码路径能在用户没点之前写外部配置。
4. 在 `src/hook_installer.rs` 顶部注释里写清：本 fork 不自动安装 hook，必须由用户显式触发；原因是会与上游 Coffee CLI 抢注册。

验收要能证明：全仓库搜不到任何在启动路径上调用 `install_all` / `install_for_tool` / `dispatch_install` 的地方。

## B. 改掉共用的持久化标识

**一律不迁移、不读取、不复制、不删除 `~/.coffee-cli` 下的任何东西——那是上游程序的数据。** 本程序用全新的 `~/.aicross`，首次运行就是空的，这是预期行为。

### B1. 配置目录 `~/.coffee-cli` → `~/.aicross`

七处，全部改：

```
src/hook_installer.rs:580   home.join(".coffee-cli").join("hooks")
src/marketplace.rs:38       home()?.join(".coffee-cli").join("marketplace")
src/server.rs:3166          home.join(".coffee-cli").join("cache").join("heatmap-counts.json")
src/server.rs:3449          .join(".coffee-cli")
src/skills.rs:96            home()?.join(".coffee-cli").join("skills")
src/skills.rs:100           home()?.join(".coffee-cli").join("skills-library")
src/tool_config.rs:72       dirs::home_dir()?.join(".coffee-cli").join("tools.json")
```

这七处硬编码了同一个字符串。**不要逐处替换字面量**——在 `src/paths.rs` 里加一个返回根目录的函数（`~/.aicross`），七处都改成调它。`paths.rs` 的模块注释本来就写着它的职责是收拢这些路径，正好补上。

### B2. keyring service 名

`src/server.rs:3480` `const KEYRING_SERVICE: &str = "coffee-cli"` → `"aicross"`。

这条最要紧：不改的话两个程序读写同一批凭据条目，互相覆盖。改完后本程序读不到上游存的凭据，**这是正确行为**，不要为此写迁移代码。

### B3. 环境变量

`COFFEE_CLI_TAB_ID` / `COFFEE_CLI_HOOK_PORT` / `COFFEE_CLI_TOOL` → `AICROSS_TAB_ID` / `AICROSS_HOOK_PORT` / `AICROSS_TOOL`。

注意 `src/hook_forwarder.rs` 里有按这些变量存在与否判断行为的分支（`COFFEE_CLI_TOOL=codex` 的 gate），改名要连判断一起改，别漏。

### B4. hook 脚本文件名

`scripts/` 下与代码里引用的：

```
coffee-cli-hook.py
coffee-cli-codex-notify.py
coffee-cli-opencode-plugin.js
coffee-cli-hermes-plugin.py
coffee-cli-island.js
coffee-cli-status.json
```

改成 `aicross-` 前缀。`include_str!` 的路径要跟着改（`src/hook_installer.rs:76`、`:79`）。
Hermes 那条还有 `hermes plugins enable coffee-cli-status` 命令字符串，一并改。

### B5. npm 包名

`src-ui/package.json` 与 `package-lock.json` 的 `name: "coffee-cli"` → `"aicross-ui"`。改完 `npm install` 要能跑通。

## C. install/ 目录

`install/install.ps1` 等脚本指向 `coffeecli.com`，跑起来装的是 Coffee CLI 不是本程序。本项目还没有发布渠道。

**本期直接删掉整个 `install/` 目录**，并在 REPORT 里记明：删除原因是它属于上游的发布渠道，本项目有自己的分发方式后再写。

`installer.nsi` 保留——F1 已确认它用的是 Tauri 注入的 `{{product_name}}`，跟着 `productName` 走，没有硬编码上游品牌。

## 边界

- 不删功能（`install/` 目录除外，那是发布脚本不是功能）。
- 不动 `LICENSE`、`NOTICE`、`TRADEMARKS.md`、`port/`、`docs/`。
- 不删版权头与 SPDX 行。
- **不写、不读、不删用户的 `~/.coffee-cli`、`~/.claude`、`~/.codex`、`~/.kimi-code`、`~/.config/opencode`、Hermes 目录下的任何文件。** 本期是要断开这些写入，不是换个地方写。测试里要用外部配置就用临时目录。
- 不加 npm 依赖、不加 cargo 依赖。
- 不 commit，不 push。

## 验收

命令与输出原文贴进 REPORT：

1. `cargo check` 通过。
2. `cargo test` 通过。上游自带 80 个测试，**数量不该减少**；改名会牵动一批断言，改测试可以，删测试不行。少一个都要在 REPORT 里说明。
3. `cd src-ui && npm run build` 通过。
4. 残留自查，逐条贴输出：
   - `grep -rn '\.coffee-cli' src/ --include='*.rs'` —— 只允许注释里作为历史说明出现，代码路径应为空。
   - `grep -rn 'COFFEE_CLI_' src/ --include='*.rs'` —— 应为空。
   - `grep -rn 'coffee-cli' src/server.rs` —— keyring 那条应已消失。
   - `grep -rn 'install_all()\|install_for_tool(' src/` —— 除定义处与显式 command 外，**启动路径上应为零**。
   - `ls install/` —— 应不存在。
5. **不要为验收去真机跑安装 hook 的代码路径。** 任何会写外部 CLI 配置的测试，必须用临时 HOME 或临时目录。

跑不了的写「未验证」。窗口手测由编排者在沙箱外做，不要声称做过。

## 交付

写 `tasks/REPORT-F2.md`：改了什么、怎么验证的（命令与结果原文）、没做什么、不确定处。特别要写清：**是否还存在任何在用户没有显式触发的情况下写外部 CLI 配置的代码路径**。不 commit。
