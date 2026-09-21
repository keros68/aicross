# TASK-35 控制台 @ 别家时补上它没看到的对话（09-21）

**只改前端 `src-ui/**` 与 `fixtures/prompts/`。不碰 `src/**`（Rust）。**

## 问题

控制台单线聊天里，每个 agent 各有自己的续聊会话（`consoleResumeKey` = agent + provider + model）。`@kimi 帮我看看上面的方案` 发出去的只有这一句（`ConsoleChat.tsx` `send` → `dispatchConsoleTurnWithFallback(…, outbound, …)`），kimi 的会话里没有此前用户与 codex 的那几轮，答不上来。`docs/PRODUCT.md` 写的是「@执行者：默认拿到当前对话的上下文」，现状与它不符。

做法参照 Ekko Studio 群聊的历史投影（`D:\work\agentteam\COMPETITIVE-LANDSCAPE.md` 09-21 节；只取思路，BSL-1.1 不抄代码）。区别：它每轮重建全量历史；我们各家有原生续聊，**只补目标没看到的那一段**。

## 契约（只追加）

新文件 `src-ui/src/console-projection.ts`，纯函数，不碰 store、不发请求：

```ts
export interface ConsoleCatchup {
  /** 发给目标的全文；没有可补内容时等于 outbound 原文 */
  text: string;
  /** 补进去的消息条数（截掉的不算） */
  included: number;
  /** 因总长上限被丢掉的更早消息条数 */
  omitted: number;
}

export function projectConsoleCatchup(
  messages: ConsoleMessage[],   // 不含本次刚输入的这条
  target: ConsoleTargetAgent,
  outbound: string,
  options: { fresh: boolean; mentions: ConsoleTargetAgent[] },
): ConsoleCatchup
```

规则：

1. **取哪一段**：`fresh` 为 false 时，从 `messages` 里最后一条「目标自己说的」之后开始取（`kind === 'assistant'` 且 agent / provider / model 三项与 `target` 全等，比较口径与 `consoleResumeKey` 一致）；找不到或 `fresh` 为 true 时取全部。
2. **收哪些**：`kind === 'user'`，与 `kind === 'assistant'` 且 `text` 非空且 `status` 不是 `error`。`kind === 'system'` 一律不收。
3. **怎么标**：沿用 `graph/context.ts` 的口径——用户消息 `[用户]\n正文`；模型消息 `[模型回复 agent/model]\n正文`（agent、model 取消息上的字段，缺的那项省掉，两项都缺就 `[模型回复 模型]`）。段间空一行。
4. **剥 @**：用户消息开头若是可识别的 `@显示名`，用 `parseConsoleMention(text, options.mentions)` 取 `text` 后的正文；剥完为空的跳过。
5. **长度**：单条正文超过 4000 字符时保留头 3000 + 尾 800，中间写 `…[省略 N 字]…`（N 为实际省掉的字符数）。全部段落合计超过 24000 字符时从最早的一条起丢，直到不超；丢过就在最前加一行 `[更早的 N 条对话已省略]`。两个数字做成文件内具名常量，不进设置页。
6. **拼装**：有可补内容时，全文 = 前缀模板 + 空行 + 各段 + 空行 + `[当前消息]\n` + outbound。前缀放 `fixtures/prompts/console-catchup-prefix.txt`，用 `?raw` 引入（同 `blind-prefix.txt` 的做法），内容一句话：`以下是本对话中你尚未看到的部分，仅供了解背景；只需回应最后的「当前消息」。`
7. **不补的情况**：`outbound` 以 `/` 开头（各家原生命令，前面加字会失效）；没有任何可补段落。此时 `text === outbound`、`included === 0`。

## 接线

1. `console-chat.ts`：
   - `prepareConsoleTurn` 增加一个可选参数，让 `prompt` 与 `currentMessage` 都用投影后的全文，`originalRequest` 仍是用户原话。**注意**引擎在续聊时发的是 `currentMessage` 不是 `prompt`（`src/dispatch.rs` `is_continuing` 分支），两个字段都要带上。
   - `dispatchConsoleTurnWithFallback` 末尾追加一个可选参数 `freshText?: string`：会话失效重开（`${runId}-fresh` 那条路）时用它代替 `text`；不传则行为与现在完全一致。现有调用与现有测试不改签名即通过。
2. `ConsoleChat.tsx` `send`：
   - `resumeId` 非空 → 用 `fresh: false` 的投影作 `text`，`fresh: true` 的投影作 `freshText`；`resumeId` 为空 → 直接用 `fresh: true` 的投影。
   - 真正发出的那一份 `included > 0` 时，在用户消息之后、派发之前追加一条 system 消息：`已附带此前 N 条对话给 <显示名>`；`omitted > 0` 再接 `（更早的 M 条已省略）`。会话失效重开走了 `freshText` 时，按 `freshText` 的条数再报一次。显示名用界面上现有的 agent 显示名取法，不另造。
   - 消息流里用户那条仍显示原文（含 `@`），不显示拼出来的全文。
3. 模式派发（`dispatchConsolePreset`）**不动**：盲节点必须保持只看原始材料，本期不给任何预设路径加历史。

## 测试

新文件 `src-ui/src/__tests__/console-projection.test.ts`，至少覆盖：

- 目标从未说过话 → 取全部；目标说过 → 只取它最后一条之后的；同 agent 不同 model 视为没说过。
- system 消息、`status: 'error'` 的回复、空正文不进。
- 用户消息的 `@显示名` 被剥；剥完为空的跳过。
- 单条超长的头尾保留与省略字数；总长超限时从最早丢、`omitted` 计数与首行提示。
- `/compact` 这类开头为 `/` 的原样返回。
- 没有可补内容时 `text === outbound`。

`console-acp.test.ts`（或同目录新文件）补两格：带 `freshText` 时会话失效重开发出的是 `freshText`；续聊请求里 `prompt` 与 `currentMessage` 都是投影全文、`originalRequest` 是原话。先确认去掉实现会红。

## 顺手核一件事（只报告，不修）

kimi 的 ACP 起不来时会回退一次性 CLI。查一下那条回退路径的 prompt 是不是走 argv：Windows 下 argv 在第一个换行处截断，本期的多行全文会被截掉。结论写进报告，不在本期改 Rust。

## 验证

`cd src-ui && npx tsc --noEmit`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。Rust 未改，`cargo check --all-targets` 跑一次确认零 warning 即可。

## 窗口核对清单（主线程做）

1. 与 Codex 聊两轮，再 `@kimi 上面的方案有什么问题`：出现「已附带此前 4 条对话给 Kimi Code」，kimi 的回答提到 codex 说过的内容。
2. 紧接着再 `@kimi 继续`：不再附带（它上一条之后没有新对话）。
3. 回到 Codex 发一句：附带的是 kimi 那两轮，不含 codex 自己的。
4. `@kimi /skill:` 补全与执行不受影响。
5. 打开 `.dispatch/` 留痕，确认发出的全文与界面提示的条数一致。

保持文件原有换行；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-35.md`。
