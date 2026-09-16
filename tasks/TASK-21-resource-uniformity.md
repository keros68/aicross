# TASK-21 接入信息的统一（09-16 用户反馈）

用户第三次指出同一类问题：**同样的信息，四家 agent 显示成四个样子**。前两次改的是行结构与字号（TASK-19），这次是行上方的版本号、模型名本身、以及悬停详情。规则源见本仓 `AGENTS.md`「界面文案与命名规范」：一个概念一个词，同一类信息同一个形状。

用户原话：「不仅仅是悬停，上面显示版本号，claude code 加 claudecode、codex 是 codex 加版本号，最下俩只有版本号，其实只有版本号才对；然后模型名显示，有的有 id，有的只有模型简称，也很奇怪；还有 pi 的模型验证问题依旧没解决。」

## 现状（编排者已核实，别再花时间复查）

本机 `~/.aicross/resources.json` 存的是各 CLI `--version` 的**原样输出**：

| agent | 存的值 | 界面显示 |
| --- | --- | --- |
| claude | `2.1.272 (Claude Code)` | 同左 |
| codex | `codex-cli 0.154.0` | 同左 |
| kimi | `0.42.0` | 同左 |
| pi | `0.85.1` | 同左 |

悬停详情由 `src-ui/src/resources/types.ts` 的 `modelHint` 拼，字段是**有才出现**，于是：
- `sonnet` → 名字 / `模型 ID：claude-sonnet-5` / `状态：原生` / `claude --help 别名 (2026-09-15)`
- `gpt-5.6-sol` → 名字 / `状态：原生` / `读:C:\Users\keros68\.codex\models_cache.json (2026-09-15T19:21:46+08:00)`

差异有四处：`模型 ID` 行只在 `resolved_id != id` 时出现（`modelSecondaryId`）；来源串有三种形状（`读:<绝对路径> (RFC3339)`、`<命令> 别名 (日期)`、`申报(日期)`，见 `src/resources.rs` 的 `source_read` 与申报分支）；时间一处到秒带时区、一处只到日；路径是带用户名的绝对路径。

行内名字显示的是 `model.id`（派发时写进命令行的那个词）：claude 是别名 `sonnet`，codex 是真名 `gpt-5.6-sol`，kimi 带厂商前缀 `kimi-code/k3-256k`。

pi 的验证：库里 31 个模型，30 个 `unverified`、1 个 `verified`。`deepseek-v4-flash` 是 `unverified` 且带 `smoke_failure: 真身不符：请求 deepseek/deepseek-v4-flash，响应 deepseek/deepseek-flash`，`smoked_at: 2026-09-14T08:45:02`。**但 `src/channels/mod.rs` 的 `KNOWN_MODEL_ROUTES` 里已经有这条官方路由别名**（表是 09-14 之后才加的）——这条失败是加表之前留下的陈旧判定，自己永远不会翻案。另外「冒烟失败」被记成 `verification: unverified` 而不是 `failed`，界面上和「从没验过」共用一个状态词。

## 要做的

### 1. 组头只显示版本号

抽一个纯函数（放 `src-ui/src/resources/types.ts` 或同目录新文件）：从 `--version` 原样输出里取出版本号本身，取不到就原样返回。四家各一格测试，至少覆盖上表四个真实值，外加取不到版本号时的回退。行上只显示版本号；**原样输出挪进该行的悬停**，不要丢掉证据。

### 2. 模型名与悬停详情的形状固定

- **行内名字**：去掉与组头重复的厂商前缀（`kimi-code/k3-256k` → `k3-256k`；其余家当前没有前缀，规则要写成通用的「前缀与 agent 显示名同义时去掉」，不要写死 kimi）。别名与真名的取舍不变：行上仍是派发时写进命令行的那个词。
- **悬停详情**：前四行**固定出现、固定顺序、固定标签**，即使值与名字相同也要写：
  1. 名字（即行上显示的那个词）
  2. `模型 ID：<resolved_id 或 id>`
  3. `状态：<原生 / 已验证 / 未验证 / 验证失败>`
  4. `来源：<人话> · <YYYY-MM-DD>`
  之后是可选行，**顺序固定**：`额度用尽至`、`provider`、`登录状态`、`最近登录检查`、`最近验证`、`失败原因`。有才出现。
- 接入页的模型详情（`AgentCardModelMeta`）与左栏悬停必须走**同一个构造函数**，不要两处各拼一份。

### 3. 来源串统一

现在三种形状统一成 `来源：<人话> · <YYYY-MM-DD>`：

| 现在 | 之后 |
| --- | --- |
| `读:C:\Users\keros68\.codex\models_cache.json (2026-09-15T19:21:46+08:00)` | `来源：读取 ~\.codex\models_cache.json · 2026-09-15` |
| `claude --help 别名 (2026-09-15)` | `来源：claude --help 别名 · 2026-09-15` |
| `申报(2026-09-15)` | `来源：用户申报 · 2026-09-15` |

两处都要改：`src/resources.rs` 写入新值时用新形状；**显示层还要能把库里已有的老串归一**（用户不会为了改文案去重新探测一遍）。老串归一要有测试，三种老形状各一格。路径里的用户主目录缩成 `~`。

### 4. pi 的验证：别名表的修正要能传播到旧库

和 `resolved_source` 那条「静态值可以被新表覆盖」是同一个道理，照做：

- 加载资源库时，对带 `smoke_failure` 的**真身不符**记录，按当前 `KNOWN_MODEL_ROUTES` 重判一次：命中已知路由别名的，翻成已验证、把 `resolved_id` 记成实际响应的真身、清掉失败原因，并在来源/备注里留一句「按已知路由别名重判（日期）」。没命中的保持原样。
- 冒烟失败要记成 `verification: "failed"`，不要和「从没验过」共用 `unverified`。界面上「未验证」与「验证失败」本来就有两个词，现在数据层没分开。改完注意 `unverifiedCount` 与左栏「未验证 N 个」的口径跟着走。
- **不要为此发任何真实模型请求**：本项是纯粹的存量数据重判 + 状态分类，不许在加载路径里冒烟（DeepSeek 按次计费，用户明确要求少用）。

## 不做

- 不改「首次派发时自动验证第三方」这条既有规则，也不做批量自动冒烟。
- 不改行结构、字号、分组折叠（TASK-19/17 刚定，两家复审过）。
- 不动派发时写进命令行的那个词本身（只改**显示**）。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；跑 `scripts/copy-audit.py`。

新增行为要能红：版本号提取四格、老来源串归一三格、详情固定四行（用一个 `resolved_id == id` 的模型断言「模型 ID」行照样在）、已知路由别名重判（命中翻案 / 未命中保持）、冒烟失败记成 `failed`。每条先确认去掉实现会红，并在报告里写清怎么红的。

保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF，其它 Rust LF，`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-21.md`，逐条对应上面 1–4。
