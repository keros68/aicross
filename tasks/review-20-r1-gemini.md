# TASK-20 R1 复审 · Antigravity / Gemini 3.1 Pro (High)

2026-09-16。材料全部内联（修正清单 + 实现者报告 + src/src-ui/docs 完整 diff，约 10 万字符），prompt 里明写不使用任何工具——该通道无头模式下读文件会被权限自动拒绝，只能这样用。
用量：56,941 token / 70.2 秒 / 订阅额度窗口，无按次费用。

---

**总判**：可交付

（本次复审未发现任何未闭合项或新引入的问题，12 条修正已全部按要求落实。）

**核过没问题的要点**：
- **终端与输入框焦点阻断（第 1、2、3 条）**：`proposalTargetFlags` 纯函数精确分离了 `ptyFocused` 与 `targetInput` 的识别。在 `proposalShortcutAction` 中，PTY 分支判定置于顶层，仅放行 `Ctrl+Enter`，成功交还 Esc/Tab/Enter；单行 `INPUT` 与 `TEXTAREA` 均被有效拦截，彻底杜绝回车导致的静默派发；复用 `hasOpenModal` 判断，完美避让了设置页等模态弹窗的 Esc 监听。
- **幽灵建议与超时清理（第 4、7 条）**：`relay_proposal` 采用了 `mpsc::sync_channel` 挂起 HTTP 线程，前端通过 `runConsoleProposal` 收到推送后先 `await io.consoleRunUpdate`，成功后才在 UI 渲染。找不到 run 或超时直接异常退出并不上屏。初始化时 `dismissStaleConsoleProposals` 能有效洗掉残留的 `proposed` 状态；`cancel` 动作也已正确映射到 `dismissed` 终态。
- **并发控制与本地注册（第 5、6 条）**：新增 `console_register_proposal` 补齐了本地建议的入表逻辑。不论通道还是本地建议，均走统一的 `acceptConsoleProposal` 进行真实 run 查询和前端上限拦截。遇阻时 catch 逻辑会将其从可视列表中移除，并转化为明确的红色 notice（“在飞已满，稍后再采纳”）。
- **分类判据与执行（第 9、11 条）**：`proposalClassificationVerdict` 正则只提取首个命中词；`classificationTarget` 遇指定模型缺失时直接抛错而不降级，异常信息区隔清晰。二次分类调用锁定了 `proposal-classify` 为 runId 与 nodeId，超时限制收窄至 20 秒，避免了长轮询及目录无限增殖。
- **其它细节（第 8、10、12 条）**：新增的 Vitest 测试涵盖了 4 种元素的模拟目标构造、上限 409 的断言；无 `preset.name` 时的显示去重逻辑生效；文案、产品操作入口命令（`aicross propose`）及文档修订均准确无误。整个补丁干净，无任何绕过限制的 mock 或破坏契约的改动。
