# REPORT-68 开始页拆成主 agent 与副 agent

## 改了什么

- `StartPage.tsx` 分四块：
  1. 主 agent：探测、在哪个软件里用、ai-cross skill 状态。
  2. 副 agent：每家一行，当前主 agent 标「当前主 agent」，「详情」进资源页。
  3. 配好：一键配好与审、干活两行。
  4. 怎么用。
- 「本机的 AI」与「也可在这些软件里用」两处说法去掉。只能当主 agent 的软件已在第 1 块列出。
- `start-view.ts`：
  - `workerAgents` 与 `workerText` 替换 `foundAgents` 与 `hostOnlyNames`。
  - 可用按资源页同一口径（可用、已验证）计数，未验证单独计数。
  - 系列名按可用加未验证的模型数排序，全写不截断；「其他」「未知」不写。
- `aicross resources` 的模型输出加 `vendor`，前端类型同步。
- 文档：
  - `docs/MANUAL.md`「开始」一节改为四块，术语表加主 agent、副 agent。
  - `docs/CONSOLE-CHANNEL.md` 写明 `vendor`。

## 验证

```
cargo test                 → 709 passed, 10 ignored
src-ui: npx vitest run     → 252 passed
npx tsc -b; npx eslint src → 无输出
```

窗口（开发版，本机数据，主 agent 选 Codex）：
- 主 agent 块：十个软件的选择、skill「由 skills-manager 管理 · 1.23.0」「需要升级到 1.24.0」显示正常。
- 副 agent 块：
  - Claude Code 可用 4 · 含 Claude。
  - Codex 可用 8 · 含 GPT，标「当前主 agent」。
  - Kimi Code 可用 6 · 含 Kimi。
  - pi agent 可用 8、未验证 26 · 含 GLM、Qwen、DeepSeek、GPT、Kimi、StepFun、Claude、Gemini、MiniMax。说明在名称右侧那一栏内换行。
  - Antigravity 可用 13 · 含 Gemini、Claude、GPT。
  - 五家可用数合计 39，与资源页「可用模型 39」一致。
- 配好块显示正常。看到时盲审模式三个位置未选，这是 Codex 主 agent 原有的配置状态，与本期无关。

## 没做 / 不确定

- 第一次打开、还没探测时的样子没有在窗口里看，只看了代码分支：副 agent 写「探测后列出」，配好写「探测后可配」。
- 系列名表是手写的，只有常见几家；表里没有的厂商照写厂商名。
