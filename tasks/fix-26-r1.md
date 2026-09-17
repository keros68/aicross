# fix-26-r1 待办册建议条与资源页批 · 复审修正轮

依据 GLM-5.3-Flash 与 Antigravity/Gemini 3.8 Flash 的盲审（`tasks/review-26-glm.md`、`tasks/review-26-gemini.md`）。两家都判六条已闭合，下列三条是修正本身带出的问题。契约只追加。禁区文件与 TASK-26 任务书相同，仍然不要碰。

## 必修

1. **68 的选型让 `runs/` 目录重新无限增殖（GLM 1；Gemini 3 提示过清理问题，GLM 核实全仓没有清理逻辑）**
   `classifyId = proposal-classify-${proposal_id}` 之后，每条建议归类都新建一个 `runs/proposal-classify-<uuid>/dispatch/`，永不清理。这正是 `tasks/fix-20-r1.md` 第 11 条当年判为缺陷、TASK-20 用固定 runId 修掉的问题——本期为了解并发互写又退回了原点。
   修法：归类调用结束后（成功或失败都要）删除**这一个** `runs/<classifyId>` 目录。只删精确匹配本次 classifyId 的目录，不做前缀批量删除，删除失败只记日志不影响归类结果。补测试：归类完成后该目录不存在；并发两条时各删各的，互不误删。

2. **67 改过头了：丢了原生 `isContentEditable`（Gemini 2 / GLM 3）**
   现在只靠 `closest('input,textarea,[contenteditable="true"],[contenteditable=""]')`。两处误判：外层 `contenteditable="true"` 里嵌一个 `contenteditable="false"` 的不可编辑区，`closest` 仍会命中外层，被当成输入控件；`contenteditable="plaintext-only"` 这种合法编辑模式反而漏判。
   修法：**优先用 `element.isContentEditable`**（浏览器会正确计算继承与 `false` 遮蔽，也认 `plaintext-only`）；只有该属性不可用时（测试替身、非 HTMLElement）才退回属性选择器，且选择器也补上 `plaintext-only`。补测试：嵌套 false 不算输入、`plaintext-only` 算输入、普通 true / 空值算输入——至少一格走 `proposalShortcutFromTarget`。

3. **57 新增的「备选已真跑后回写主尝试记录」分支没有测试覆盖（GLM 2）**
   现有测试里未登录候选是唯一候选、从没真跑过备选，于是 `result.dispatch_file == main_record.0`，走不进 `dispatch.rs:3475-3481` 这条新分支；报告里的红测命中的是另一条路径。补一例：`fallback_order` 两条，备 1 真跑失败、备 2 未登录，断言**主尝试**的留痕文件里含「跳过 …（未登录）」。

## 保持，不改（记录理由）

- **57「一次派发只读一次库」的过期窗口**：两家都提到主尝试耗时期间登录状态可能变化。这是任务书明确要求的取舍（减少每候选重复读库），窗口是主尝试的时长（秒级到几十秒），代价可接受。不改，在代码注释里写明这个冻结语义即可。
- **66 的连带效果**：下拉打开期间终端焦点守卫也不抢焦点——合理（抢焦点会让下拉失焦收起）。可以顺手在 `focus-enforcer.test.ts` 补一格，但不是必修。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。三条每条都要有能红的测试，先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-26-R1.md`，逐条对应。
