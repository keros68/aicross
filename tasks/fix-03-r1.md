# TASK-03 修正轮 R1

先读 `AGENTS.md`、`tasks/TASK-03-dispatch.md`、`tasks/REPORT-03.md`，再读两份跨厂商盲审：`tasks/review-03-glm.md`（GLM-5.3-Flash）与 `tasks/review-03-qwen.md`（qwen3.8-max）。逐条处理，每条在 `tasks/REPORT-03.md` 末尾新增「修正轮 R1」一节里写「已修：怎么改的」或「不采纳：原因」。验证跑 `cargo check`、`cargo test`、`cd src-ui && npx vitest run`（沙箱起不来就用 `--configLoader native --pool threads` 并注明）、`cd src-ui && npm run build`，命令与输出原文贴进报告。不起窗口。不要 commit，不要 push。真机请求本轮不用你发，见「编排者真跑」一节。

## 先澄清审查里的一个误会

两位审查者都发现 `.dispatch/` 里有 19:37–19:41 的成功轮次而 REPORT 没写。那是**编排者在沙箱外补跑的**两条 ignored 验收（`live_blind_review`、`live_shared_continuation`），不是实现方隐瞒。结果与结论在下方「编排者真跑」一节，本轮把它写进 REPORT 的验收 3、4 项，把「未验证」改成对应事实。

## 编排者真跑（2026-09-13 19:37–19:41，普通桌面进程）

| 路 | 结果 | 证据 |
|---|---|---|
| pi/zai-coding-cn/glm-4.7 盲审 | ok | `.dispatch/20260913-193721-pi-glm-4-7-数据审查.md`：in 457 / out 675，session 有值，16.7s。**但 responseModel 是 glm-5.3-flash**（见必修 1） |
| codex/gpt-5.6-luna 盲审 | ok | `.dispatch/20260913-194022-codex-gpt-5-6-luna-风险审查.md`：in 389489（cache_read 354304）/ out 5928，197.8s |
| kimi/kimi-for-coding-highspeed 盲审 | error | `Model "kimi-for-coding-highspeed" is not configured in config.toml`：通道传了裸 ID，config.toml 的别名是 `kimi-code/kimi-for-coding-highspeed` |
| 替补 pi/qwen-token-plan-cn/deepseek-v4-flash | error | 事件里 `stopReason: "error"`，`errorMessage: 403 AccessDenied.Unpurchased`；通道只报「pi 的 text 块为空」 |
| 共享续聊 pi/glm-4.7 两轮 | ok | 第二轮 `marker_recalled=true`（答出 青松-731），in 115 / out 41 |

## 必修

1. **pi 真身核对看错字段**（GLM 2.1，有实证）：`.dispatch/20260913-194041-pi-glm-4-7-续接验证.md` 里请求 `glm-4.7`，`responseModel: glm-5.3-flash`，被判 ok。改为优先取响应级字段（`responseModel`，没有再退回 `message.model`），不符判 `identity_mismatch`；`identity` 写真身。用那份真实事件行做 fixture 补测试。这条同时说明 zai 端点会静默升级模型，正是契约里 `identity` 存在的理由。
2. **pi 错误事件要落进 error**（编排者真跑）：解析 `message_end` 的 `stopReason === "error"` 与 `errorMessage`，写入 `error` 字段并 `status: error`，不要只报「text 块为空」。用 `.dispatch/20260913-194024-pi-deepseek-v4-flash-替补审查.md` 的事件行做 fixture。
3. **pi 用量键名**（qwen 2.1）：真实键是 `cacheRead`（以及 `input`/`output`），现取 `cache_read` 恒 0。用真实事件 JSON 做 fixture。
4. **kimi 模型参数与续聊**（编排者真跑 + GLM 2.2 + qwen 1.1）：
   - 模型参数必须是 config.toml 的完整别名；resources.json 里 kimi 模型的 id 就是别名。fixture 改成 `kimi-code/kimi-for-coding-highspeed`；通道遇到不含 `/` 的 kimi id 直接报错「请使用 config.toml 里的别名」，不静默透传。
   - 续聊：本机 0.42.0 的 `--help` 只列 `-S/--session`，但提示行打的是 `To resume this session: kimi -r <id>`。用 `kimi -p "只回复OK" -m kimi-code/k3-256k` 起一轮，再分别用 `-S <id>` 与 `-r <id>` 续第二轮各一次（共 ≤ 3 次请求；kimi 若报 403 额度上限就如实记录、改用静态断言），确定哪个能续；发送侧与解析侧统一，解析锚定 `To resume this session` 行，不在全文里找 `-r`。补测试：正文含 `-r abc12345678` 时不得误取。
5. **脱敏**（GLM 3.1、qwen 3.1）：键允许可选引号、值允许可选引号（`"password": "x"`、`"api_key":"sk-…"`）；值含逗号或空格时整段吞掉到引号或行尾；测试各加 JSON 形态一例。失败路径（`prepare_outbound` 出错）留痕正文写「(未发送)」，不写未脱敏的原始 prompt（GLM 2.3、qwen 2.4）。stderr 与 error 文本落盘前也过一遍脱敏（qwen 3.3）。
6. **kimi / cc-switch 用量**（GLM 1.3/1.4、qwen 1.2/1.5）：kimi 的 stream-json 里有 usage 就记；cc-switch 的 `--usage` 打在 stderr，从 stderr 解析；cc-switch 标「单轮」。
7. **codex `--ephemeral`**（qwen 1.3）：`codex exec --help` 第 81 行确有该 flag。REPORT 补记它的出处与用意（隔离尝试），并说明 isolation 仍标「部分」的原因。
8. **codex thinking 映射**（qwen 1.4）：改为 关→low、低→low、中→medium、高→high；xhigh 不映射（留给用户显式指定的扩展）。
9. **同 agent 串行的键要先归一化**（qwen 2.3）：调度用 `normalize_agent` 之后的名字。
10. **claude `api_error_status` 为 null 误判失败**（qwen 2.2）：按非 null 判。
11. **E 节两项**（两家都提）：回复气泡加 in/out 角标；G1 或结果卡加「新开会话」，置空该目标父链的 session_id。
12. **共享节点附件给全路径**（GLM 1.5）：共享序列化用绝对路径，盲节点保留 basename。
13. **STATE.md**（GLM 2.4）：写实际并发上限；失败/超时单列，不进「进行中」。
14. **live 验收测试断言**（GLM 1.7）：非沙箱环境（能写用户目录时）断言 `status == "ok"` 且 tokens > 0；沙箱内保持现状但打印原因。
15. **argv 测试不得静默跳过**（qwen 5.1）：把二进制解析从 `build_invocation` 拆开，测试直接断言 args，不依赖本机装没装 CLI。
16. **硬编码路径**（qwen 3.4）：`D:\work\agentteam\dist\ai-cross\references` 改成设置项（默认空，空则报「未配置 cc_switch.py 路径」），不进源码。
17. **非 Windows 取消**（GLM 2.5）：非 Windows 分支用 `kill`，或如实返回 false。

## 次要（一并做）

18. 用量表状态词改中文并与卡片头一致，「阶段」列用真实 stage（qwen 4.4）。
19. G2 行带角色或节点摘要（qwen 4.5）；running 时禁用点背景关闭（GLM 4.5）；「采纳」标记写进 reply（GLM 4.6）。
20. `context.ts` 的 `replyMessage` 排除 `failed` 回复（GLM 4.8）；「交人裁决」系统消息不用 `[系统]` 前缀塞进用户文本，改成独立字段或不带角色标记（GLM 4.7）。
21. `DispatchOverlay.tsx` 里字面 `\n\n`、每次 render 读 localStorage、`isolationOf` 与 Rust 重复——按审查建议收拾（GLM 4.1–4.4）。
22. `parse_classification` 移到测试模块（qwen 4.2）；`OrchestrationPanel.tsx` 过时注释（qwen 4.3）；盲目录测试补断言 AGENTS.md 未拷入（qwen 3.2）。
23. 正则用 `OnceLock` 缓存（GLM 4.2）。

## 不采纳（报告里写明理由）

- G2 做成 overlay 内行而非中栏系统行：功能等价，记录偏差即可。
- D 节机器归类：任务书标可选，第 4 期再接。
- codex 盲审 389k 输入 token 的问题：是它在空目录里带工具反复读材料的代价，属路由策略（小体量纯文本材料应内联进 prompt 或走 pi），记入 REPORT「观察」，第 4 期处理。

## 与另一分支的注意

`ui/identity` 分支正在改壳子（CenterPanel、app-state、TitleBar、主题），本轮不要碰这些文件里与派发无关的部分，减少合并冲突。
