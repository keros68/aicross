# fix-28-r1 只读问题盲审答案文件 · 复审修正轮

依据 GLM-5.3-Flash 与 Antigravity/Gemini 3.8 Flash 的审查（两家都判可交付，下列是共同或经编排者核实的未闭合项）。禁区与 TASK-28 任务书相同。

## 必修

1. **worktree 警告 + 长回复时漏标截断（两家共同）**
   `stdout_summary` 是「警告行 + 正文前 2000 字」拼出来的；`readonly_answer_material` 剥掉警告后按答案字数 `>= 2000` 判断，于是答案在约 1970 字处被切断却不标「已截断」。
   修法：截断事实在产物构建处（`run_one_attempt` 组装 `stdout_summary` 的位置）就已知——`channel_output.text` 经 `safe_trace` 后字数超过 2000 即为截断。把这个事实可靠地传到物化处（例如判定改为：原始 `stdout_summary` 剥警告前的正文部分字数达到 2000，或在剥警告时比较原文长度），**不要**为此新增序列化字段，除非没有别的可靠办法（若新增，沿用 `skip_serializing_if`、在报告里写理由）。补测试：带警告前缀 + 正文恰好 2000 字 → 有截断说明；带警告前缀 + 正文 1500 字 → 无截断说明。

2. **全部答案为空时不要写 ANSWER.md（GLM 2）**
   旧图或恢复图里产物的 `stdout_summary` 可能为空（serde default），此时会写出空文件并在 prompt 里指路，误导盲审。修法：剥警告后所有答案 trim 都为空时返回 `None`，不写文件、不指路、不内联。补测试。

3. **生成文件与附件重名（GLM 3，顺手收口）**
   盲节点附件里若有同名 `ANSWER.md` 会覆盖生成文件、名单里出现两次。修法：把本次生成的 `REQUEST.md` / `diff.patch` / `changed` / `ANSWER.md`（以及已有的 `VALIDATION.txt` 若在同一处可顺手）并入附件重名检查，冲突时报错「附件文件名与盲审材料重名：…」。补一格 `ANSWER.md` 的测试即可。

## 保持，不改

- `answer_summary` 里 `strip_prefix('\r')`（Gemini 2）：对 `split_once('\n')` 的右半边确实不会命中，属无害冗余；可删可留，不是必修。
- codex / kimi 的 `text` 是多条 agent 可见消息拼接而非仅最后一条（GLM 4）：仍是执行者可见回复、不含推理与工具输出，边界不破，不改。
- `types.ts` 的可选字段声明（GLM 5）：保持，契约与 Rust 同步。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。每条先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-28-R1.md`，逐条对应。
