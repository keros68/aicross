# REPORT-35 控制台 @ 别家时补上它没看到的对话（09-21）

实现：Claude 子代理（codex 当日无额度，按「限额即换家」改派），分支 `console/catchup35`。实现者的 Write 工具拒写报告类 `.md`，本报告由编排者据其两轮回报与自己的复跑落盘。

## 改动文件（全部 LF）

| 文件 | 改动 |
| --- | --- |
| `fixtures/prompts/console-catchup-prefix.txt` | 新增，前缀一句话 |
| `src-ui/src/console-projection.ts` | 新增。`projectConsoleCatchup`、`consoleCatchupNotice`、`consoleReplyResumeKey`、`CONSOLE_CATCHUP_PREFIX`；四个长度常量文件内具名 |
| `src-ui/src/console-session.ts` | `ConsoleMessage` 末尾追加 `resumeKey?: string` |
| `src-ui/src/console-persistence.ts` | `messageFrom` 读盘白名单加 `resumeKey`（非空才留） |
| `src-ui/src/console-chat.ts` | `prepareConsoleTurn` / `dispatchConsoleTurn` / `dispatchConsoleTurnWithFallback` 末尾追加可选 `freshText?`、`originalRequest?`；`currentMessage = text`，`prompt = freshText ?? text` |
| `src-ui/src/components/center/ConsoleChat.tsx` | `send` 里算两份投影、追加「已附带此前 N 条对话给 X」、按条件给回复写 `resumeKey` |
| `src-ui/src/__tests__/console-projection.test.ts` | 新增，18 格 |
| `src-ui/src/__tests__/console-acp.test.ts` | 追加 2 格，原 8 格未动 |

`src/**`（Rust）与 `src-ui/src/tauri.ts` 未碰。

## 规则落点

- 锚点：`spokenByTarget` 只按 `resumeKey === consoleResumeKey(target)` 判。
- `resumeKey` 写入：`consoleReplyResumeKey(target, result, outbound)`，仅当 `status === 'ok'`、`fallbackTo === ''`、`outbound` 不以 `/` 开头；只在 `send` 追加单线聊天回复那一处用。`consoleResultMessage` 未改，模式派发结果没有这个字段。
- 过滤与标记：`segmentOf`；`isOkReply` 只收 `status` 为 `ok` 或未定义的回复；用户消息剥 `@` 后为空或以 `/` 开头的不收。
- 截断：`clipBody`（头 3000 + 尾 800，切点避开代理对）；总长 24000 从最早丢，首行 `[更早的 N 条对话已省略]`。

## 与任务书初稿的偏离（编排者裁决）

任务书原写「锚点按消息上 agent / provider / model 三项全等」。实现者指出这不成立：消息上的 `model` 来自 `consoleResultMessage` 的 `result.modelReported || …`，pi 是 `provider/model` 形状、一次性 claude 是真实型号，与 `target.model` 对不上，会导致每轮把全部历史重发。编排者核对 `console-chat.ts` 后确认，是任务书的设计错误。修正轮 R0 裁决：

1. 锚点改用新字段 `resumeKey`；编排者随后去掉「旧消息退回三项全等」的分支——`/` 命令那一轮不写 `resumeKey`，但三项全等，回退分支会照样把它当锚点。
2. `prompt` 用 fresh 投影、`currentMessage` 用增量投影，对齐引擎两个字段的语义（续聊发 `currentMessage`；新会话或 ACP 回退清会话后发 `prompt`，`src/dispatch.rs` `is_continuing` 分支）。界面报的条数在引擎内部回退时可能与实际发出的不一致，留痕里有真实全文。
3. 历史里的 `/` 原生命令不收；非 ok 状态的回复（正文是「状态：…」提示）不收。
4. 其余小偏离接受：多导出 `consoleCatchupNotice` / `CONSOLE_CATCHUP_PREFIX` 便于单测；总长按各段（含标记行）之和算，不含段间空行、前缀与当前消息；assistant 空正文按 `trim()` 判。

## 验证（编排者独立复跑，09-21 22:00）

| 命令 | 结果 |
| --- | --- |
| `cd src-ui && npx tsc --noEmit` | 退出码 0 |
| `cd src-ui && npx vitest run --configLoader native --pool threads` | 49 文件、697 测试全过 |
| `cd src-ui && npm run build` | 通过（有一条 chunk 体积提醒，非错误） |
| `cargo check --all-targets` | 实现者首轮跑过，退出码 0、无 warning；Rust 未改，编排者未重跑 |
| 换行 | 8 个改动文件 CRLF 均为 0 |

**能不能红**：实现者两轮各验一次（首轮改坏 3 处红 4 格，R0 改坏 7 处红 9 格）。编排者对自己的追加改动再验一次：去掉 `/` 条件并把 `spokenByTarget` 改成恒真，`console-projection.test.ts` 红 9 格，恢复后全过。

## 持久化路径

消息存盘读盘只有 `src-ui/src/console-persistence.ts` 一条路（localStorage `aicross:console-sessions:v1`）。写盘直接 `JSON.stringify`；读盘 `messageFrom` 逐字段重建、未列出的字段会丢，故白名单已加 `resumeKey`，有测试盖住。reducer `APPEND_CONSOLE_MESSAGE` 原样放入。

## kimi 一次性回退的 prompt（只报告）

走 argv：`src/channels/mod.rs:361-368` 为 `["-p", prompt, "-m", model, …]`，`input: None`；claude / codex / pi 走 stdin。按代码读不会在首个换行处截断：`mod.rs:260-262` 强制原生 `kimi.exe`（`.cmd/.bat` 直接报错），`src/process.rs:269-280` 用 `Command::new(program).args(args)` 直起、不经 cmd.exe。**未真机核对。** 另：`mod.rs:360` 对 kimi 的 argv 正文做 `crop_utf8_bytes(prompt, 30_000)`（头 3/5、尾 2/5），本期 24000 字符上限在中文下最坏约 72 KB，走到这条回退时中间的历史会被再裁，尾部 `[当前消息]` 能保住。

## 已知限制 / Later

- 升级前存下的回复没有 `resumeKey`：升级后每家第一次被发消息时会把现存对话补一遍（受 24000 上限约束），之后正常。不做迁移。
- 走备选的那一轮不写 `resumeKey`，目标会话里可能已有那条用户消息，下次会重复补一次；方向是安全的（重复不丢）。
- `ConsoleChat.tsx` 的 `recentTargets` 用 `dispatchModel ?? model` 匹配 `model.id`，pi 的值是 `provider/model` 形状对不上。既有行为，本期未动。
- 模式派发的共享节点是否也带控制台历史；长对话用摘要代替丢弃。

## 盲审（09-21，两家只读，原文见 `review-35-glm.md`、`review-35-gemini.md`）

GLM-5.3-Flash（读文件，16 分钟）与 Gemini 3.8 Flash（材料内联，8 分钟）均判 **PASS**，无高、中级问题，两家没有共同问题。编排者逐条核对后的处理：

| 来源 | 条目 | 处理 |
| --- | --- | --- |
| GLM 低 2 | 引擎内部 ACP 回退那轮 `sessionId` 为空，仍会被记成锚点 | 采纳：`consoleReplyResumeKey` 加 `sessionId === ''` 不记，补一格测试 |
| Gemini 低 1 | 会话失效重开靠下层默认值回退 `freshText`，不直观 | 采纳：显式传 `fresh` |
| Gemini 存疑 1 | 重开时条数与刚报的相同会出现两条一样的提示 | 采纳：条数不同才再报 |
| Gemini 存疑 2 | 断言 Windows 下多行 argv 必在首个换行处截断 | 不成立：本机实测直接起 `.exe` 传多行参数完整到达；截断只发生在经 `.cmd` 垫片转发时，引擎强制原生 `kimi.exe`。GLM 存疑 4 另指出 ACP 回退的一次性调用走 task-file。真机 kimi 回退路径留窗口核对 |
| GLM 低 1 | 派发前就报「已附带」，`prepareConsoleTurn` 抛错时通知滞留 | 不改：与现有用户消息滞留同类，紧跟着有错误提示 |
| GLM 低 3 | 「省略 N 字」按 UTF-16 码元计 | 不改：有意取舍，测试已钉 |
| GLM 低 4 | `kept.length === 0` 且 `omitted > 0` 的分支现有常量下不可达 | 不改：单段上限约 3.8k，远小于 24000 |
| GLM 低 5 | 成功但空正文的回复显示为「已完成。」，会被当回复补给别家 | 不改：发生率低，投影层分不出合成文案；记 Later |
| GLM 低 6 | 锚点消息滑出 200 条窗口后该家整段重发一次 | 不改：重复不少发，内含于既有截断 |

采纳三条后复跑：tsc 0、vitest 697 全过、build 通过。

## 窗口真跑（09-21 22:30，编排者，dev 窗口，项目 `aicross-demo`）

codex 当日无额度，清单里的 Codex 换成 Claude Code（opus）。判据取 `.dispatch/` 留痕与 `~/.aicross/runs/*/tasks/*.txt`（一次性 CLI 实际发出的全文）。

| 项 | 结果 |
| --- | --- |
| 1 与 Claude 聊一轮（让它起代号，答「朝露既散」），再 `@kimi 上面那个代号是什么` | 过。界面「已附带此前 2 条对话给 Kimi Code」；kimi 收到 `[用户]…` / `[模型回复 …] 朝露既散` / `[当前消息]…`，`@kimi` 已剥；答「朝露既散，一共四个字」 |
| 2 紧接着再 `@kimi` | 过。无提示；只发当前这句；同一 ACP 会话；答「散既露朝」 |
| 3 回到 Claude（不带 @）问 kimi 刚才的结果 | 过。「已附带此前 4 条对话给 Claude Code」；只含 kimi 那两轮，不含 Claude 自己说过的；答「散既露朝」。Claude 回复上显示的 model 是 `claude-opus-5`、派发目标是 `opus`，三项全等对不上，锚点靠 `resumeKey` 命中 |
| 4 `@kimi /sk` | 过。照常弹出 kimi 的 `/skill:*` 补全 |
| 5 留痕与界面条数一致 | 过（见下一条的例外） |

真跑发现并已修：标记写成了 `[模型回复 kimi/kimi 不报真身]`——消息上的 `model` 是引擎报回的占位文字。改为从 `resumeKey` 取派发目标的 agent / model（`routeOf`），没有键或键坏了退回消息上的字段；补一格测试（vitest 698）。重启窗口复核：`[模型回复 claude/opus]`、`[模型回复 kimi/kimi-code/k3]`。

真跑顺带验到的两件事：

- **kimi 多行 argv**：窗口重启后旧 ACP 进程已退，引擎提示「ACP 会话已重置，改一次性完整上下文」并回退一次性 CLI。实际发出的是全量投影（8 段 + 当前消息，多行），完整到达，kimi 答对。`prompt` 用全量投影的裁决在真机上成立；直接起 `kimi.exe` 传多行参数不截断。
- **既有引擎问题（不在本期，backlog 94）**：上述回退那一轮，`.dispatch/` 留痕的「任务全文」记的是增量那份（2 段），实际发出的是全量那份（`tasks/<node>.txt`）。`write_record` 传的是 `outbound` 而不是 `oneshot_outbound`。

## 未做

会话失效重开（`freshText` 路径）没有在窗口里触发过，只有单测覆盖。
