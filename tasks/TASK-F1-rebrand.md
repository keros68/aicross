# TASK-F1 — fork 后的改名换标

## 背景

2026-08-31，本仓库改为 **Coffee CLI 的 fork**（分支 `fork/coffee-base`）。上游源码已复制进来：Rust 在 `src/`，前端在 `src-ui/`，配置在根目录 `Cargo.toml` / `tauri.conf.json`。

fork 前的 aicross 自有代码移到了 `port/`（`src-tauri-old/`、`src-ui-old/`），**本期不动它**，下一期再搬。

fork 基座已验证可编：`cargo check` 与 `cd src-ui && npm run build` 都通过。

先读仓库根目录的 `AGENTS.md`「与 coffeecli 的关系」一节——它写明了本 fork 的四条义务，本期做的就是其中第 3 条。

## 本期只做一件事：改名换标

**不删功能、不搬 `port/` 里的代码、不改业务逻辑。** 删功能与移植是后面两期的事，混在一起会让编译失败难定位。

### A. 产品标识

| 位置 | 现值 | 改成 |
|---|---|---|
| `tauri.conf.json` `productName` | `Coffee CLI` | `aicross` |
| `tauri.conf.json` `identifier` | `com.coffeecli.desktop` | `com.aicross.desktop` |
| `tauri.conf.json` `app.windows[0].title` | `Coffee CLI` | `aicross` |
| `tauri.conf.json` `version` | `3.2.3` | `0.1.0` |
| `Cargo.toml` `name` | `coffee-cli` | `aicross` |
| `Cargo.toml` `version` | `3.2.3` | `0.1.0` |
| `Cargo.toml` `description` | 上游描述 | `多厂商 agent CLI 的桌面派发控制台` |
| `Cargo.toml` `repository` | 上游仓库 | `https://github.com/keros68/aicross` |

`Cargo.toml` 的 `license = "AGPL-3.0-or-later"` **不许改**。
改 `Cargo.toml` 的 `name` 会连带 `[[bin]]` 段、`build.rs`、`installer.nsi`、`src/` 里引用 crate 名的地方，一并跟上，改完 `cargo check` 必须过。

### B. 用户可见字符串

全仓库 `Coffee CLI` / `Coffee-CLI` / `coffee-cli` / `coffeecli` 共约 295 处。逐类处理：

- **界面文案与 i18n**：`src-ui/src/i18n/` 下 11 个语言文件（de/en/es/fr/ja/ko/pt/ru/vi/zh-CN/zh-TW）里的产品名一律改成 `aicross`。**`aicross` 全小写，任何语言都不翻译、不加空格、不首字母大写。**
- **安装器**：`installer.nsi` 里的产品名、安装目录、快捷方式名。
- **Rust 里的用户可见字符串**：窗口标题、托盘提示、错误提示里的产品名。
- **不要动**：`LICENSE`、`NOTICE`、`TRADEMARKS.md`、`port/` 下任何文件、源码里的版权头与 `SPDX` 行、指向上游仓库的 URL（那是致谢，不是品牌）。`README.md` 里「fork 自 Coffee CLI」那句是义务，保留。

### C. 上游商标：`Gambit` / `Sentinel` / `Hyper-Agent`

这三个是上游主张的商标，**不在 AGPL-3.0 授权范围内**（见 `TRADEMARKS.md` §1）。规则分两层，别做过头：

- **用户可见字符串里必须换掉**：i18n 文案、按钮名、面板标题、设置项名称、tooltip。换成中性的功能描述：
  - `Gambit` → `合并输入`（英文 `Merged input`）——它的功能是把一路键盘输入同时送进多个终端
  - `Sentinel` → `会话监控`（英文 `Session monitor`）
  - `Hyper-Agent` → 按 `src/server.rs` 里该处的实际功能命名
- **代码标识符保留不动**：变量名、函数名、文件名（`Gambit.tsx`、`gambit-history.ts`、`ActiveGambit.tsx`、CSS 类名）**一律不改**。它们不是面向用户的品牌使用，保留反而诚实反映代码来源；强行重命名 39 个文件只会增加破坏编译的风险，收益为零。

如果某处分不清是用户可见还是内部标识，**保守处理：留着不动，在 REPORT 里列出来让编排者判断**。

### D. 图标

上游图标是 line-md 图标集的 `coffee-filled-loop`（Apache-2.0，`NOTICE` 里有说明），上游**不主张**该图标的商标权，但它是他们的品牌视觉，换掉。

本期做法：`icons/` 下生成一套 aicross 的占位图标——深色圆角方块 + 白色等宽 `AC` 字样，尺寸对齐 `tauri.conf.json` `bundle.icon` 列的全部文件名（32/128/128@2x/256/512 png + icns + ico）。用 `icons/` 现有的生成脚本或 `src-ui/resize_icon.js`；没有可用脚本就用 Rust 或 Node 现场生成，**不要引入新依赖**。

`NOTICE` 里关于 line-md 图标的那段**不许删**——即使我们不再用那个图标，上游 NOTICE 是逐字保留的。

## 边界

- 不删任何功能、不删任何文件（图标除外）。
- 不动 `port/`、`docs/`、`tasks/` 下已有内容。
- 不加 npm 依赖、不加 cargo 依赖。
- 不删版权头，不 commit，不 push。

## 验收

必须实际跑过，命令与输出原文贴进 REPORT：

1. `cargo check` 通过。
2. `cd src-ui && npm run build` 通过。
3. `cd src-ui && npx tsc --noEmit -p tsconfig.app.json` 通过（这个仓库有这个文件）。
4. 残留自查，把输出贴进 REPORT：
   - `grep -rn 'Coffee CLI' src src-ui/src tauri.conf.json Cargo.toml installer.nsi` —— 剩下的每一处都要在 REPORT 里说明为什么保留。
   - `grep -rn 'Gambit\|Sentinel\|Hyper-Agent' src-ui/src/i18n/` —— 应为空。
5. `cargo test` 通过（上游自带的测试，改名不该破坏它们；有失败就说明改坏了）。

**跑不了的写「未验证」，不要写「已验证」。** 窗口手测由编排者在沙箱外做，不要声称做过。

## 交付

写 `tasks/REPORT-F1.md`：改了什么、怎么验证的（命令与结果原文）、没做什么、不确定处、以及 C 节里你拿不准留下的条目清单。不 commit。
