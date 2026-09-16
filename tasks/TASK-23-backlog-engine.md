# TASK-23 待办册清理 · 引擎批（09-16）

用户授权把积压的坑清掉。本期只做 `tasks/fix-04-r5-backlog.md` 里**引擎侧**、彼此不相干、改动都很小的那一批。**只做下面列出的条目，不要顺手改别的**；每条改完在 backlog 里把该条用 `~~…~~` 划掉并在末尾注明「已修（09-16，TASK-23）」。

同时另有两期在飞，**这些文件一律不要碰**（避免合并冲突）：`src-ui/src/components/center/**`、`src-ui/src/components/right/orchestration/**`、`src-ui/src/console-channel.ts`、`src-ui/src/commander/**`、`src-ui/src/resources/**`、`src-ui/src/components/resources/**`、`src-ui/src/components/left/**`。本期基本只动 `src/dispatch.rs` 与其测试。

## 要做的条目

### 3. 验证失败的 error 文本带 ANSI 转义（小）
现在留痕与 Reply 里会写成 `验证命令失败（exit=Some(1)）\e[33m…`。两处改：把验证命令输出里的 ANSI 转义剥掉再写；`exit=Some(1)` 改成 `exit=1`（没有退出码时写「exit=未知」，不要出现 `None`）。剥转义要覆盖 CSI（`\e[…m` 一类）与 OSC（`\e]…\a` / `\e]…\e\\`），不要只删 `\e[`。

### 8. diff.patch 新增文件头带引号没被改写（小）
git `--no-index` 对新文件输出的是 `+++ "after/..."`（**带引号**），R3-21 的行头改写只认不带引号的形态，于是新增文件的补丁头没被改写。补上带引号的形态；引号里可能有 git 的八进制转义，保持原样不要解码。

### 9. kimi 留痕里 identity / usage 是空串（小）
kimi 不报用量与真身，现在留痕里就是两个空字段，看起来像丢了数据。改成明示「kimi 不报用量」「kimi 不报真身」（措辞按 `AGENTS.md` 的文案规范，中性说明体），不要写成 0 或「未知」。只改 kimi 这一路，别家不受影响。

### 23. 补丁新建文件回退后留下空目录（小）
回退时删掉了补丁新建的文件，但它们所在的新目录留了下来。回退后把**本次补丁新建且现在已空**的目录一并删掉，逐层向上直到遇到非空目录或项目根。**不要删项目根，也不要删本来就存在的空目录**——只删这次新建出来的。

### 24. 验证脏路径 note 未走 20 条截断（小）
其它地方的清单都按 20 条截断并注明「另有 N 条」，验证脏路径这一处没走同一条路径。改成复用既有的截断函数，口径与别处一致。

### 34. 额度恢复时间的误识别（小）
`resets \d` 会命中「server resets 3 connections」这种与额度无关的正文。收紧：只有在有 reset / retry-after / 额度语义的上下文里才认；没有这些语义时的 ISO 兜底不要取正文里第一个时间戳（会抓到日志时间），改成不给恢复时间。

### 35. 额度状态码漏判（小）
`status 429`（空格、无冒号）与 `HTTP/1.1 429` 两种写法现在不识别，方向是漏判（该标额度用尽却没标）。补上这两种形态。**注意不要把「429」这个裸数字在任意位置都算命中**——要求它出现在状态码语境里。

## 不做

- 不碰 21、22、25、26 这几条（涉及回退语义与蜂群入口的设计取舍，留给后面单独处理）。
- 不做任何重构、不清理无关死代码、不动留痕 schema 字段名。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。

**每条都要有能红的测试**：ANSI 剥离（含 CSI 与 OSC 两种）、带引号的补丁头改写、kimi 留痕的两句说明、回退后空目录被清掉且既有空目录不受影响、脏路径清单的 20 条截断、误识别的两个反例（`server resets 3 connections` 不给时间、日志时间戳不被当恢复时间）、漏判的两个正例（`status 429`、`HTTP/1.1 429`）。每条先确认去掉实现会红，并在报告里写清怎么红的。

保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF，其它 Rust LF）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-23.md`，逐条对应 3/8/9/23/24/34/35。
