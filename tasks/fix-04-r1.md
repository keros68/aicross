# TASK-04 修正轮 R1

先读 `AGENTS.md`、`tasks/TASK-04-pipeline.md`、`tasks/REPORT-04.md`，再读 `tasks/review-04-glm.md`（GLM-5.3-Flash 盲审）。逐条处理，每条在 `tasks/REPORT-04.md` 末尾新增「修正轮 R1」一节里写「已修：怎么改的」或「不采纳：原因」。验证跑 `cargo check`、`cargo test`、`cd src-ui && npx vitest run`（沙箱起不来用 `--configLoader native --pool threads` 并注明）、`cd src-ui && npm run build`，命令与输出原文贴进报告。不起窗口。不要 commit，不要 push。真机请求本轮不用你发，见「编排者真跑」。

## 编排者真跑（2026-09-13 21:47–21:52，沙箱外）

- `cargo test dispatch::tests::live_economy_pipeline -- --ignored --nocapture`：**1 passed**，REQUESTS=3（出方案 pi/glm-5.3-flash → 执行 codex/gpt-5.6-luna → 验收 kimi/k3-256k），run `task04-pipeline-20260913-214756`，STATE.md 在临时项目目录。
- `AICROSS_CC_SWITCH_PATH=… AICROSS_CC_SWITCH_PROVIDER="Zhipu GLM" cargo test dispatch::tests::live_cc_switch_route -- --ignored --nocapture`：**1 passed**，`CC_SWITCH_REQUESTS=1`，留痕 `.dispatch/20260913-215132-cc-switch-cc-switch-验证.md`，in 7531 / out 63（usage 正则对真实 `--usage` 输出解析成功）。
- 把这两条写进 REPORT 的验收 3 与遗留 2，「未验证」改为对应事实。

## 必修

1. **G1 可勾选把方案发给盲验收**（GLM 1.1，A 节明文）：`contextPartsOf` 为盲验收额外产出「被排除的上游回复」候选，G1 渲染为默认不勾的复选项，勾选后进 `selection.messageKeys` 且 `serializeContext` 保留。补测试。
2. **`parseAcceptance` 子串误判**（GLM 2.1）：改为 `\bPASS\b` / `\bNEEDS_FIX\b` 取最后位置；补测试：`all tests passed` 不判 PASS；PASS 与 NEEDS_FIX 同现取最后；判定词落在截断线外。
3. **prompt 模板可在设置里看**（GLM 1.2，C 节）：角色页底部只读展示三类模板。
4. **用量表按阶段合计**（GLM 1.3，E 节）：按 stage 插小计行。
5. **单写者只对 codex 有意义**（GLM 2.2）：G1 在「单写者 && agent 不是 codex」时提示「该通道无写工具，产物为空」，角色填充时执行角色默认选可写通道。
6. **写锁获取失败不得无锁运行**（GLM 2.3）：`.ok()` 改为记 status=error。
7. **盲验收交付不看 kind**（GLM 2.4）：父回复有 artifacts → 走目录不进 prompt；无 artifacts → 回退进 prompt。补测试覆盖 turn 型单父验收。
8. **快照单文件上限**（GLM 2.5）：超过 8MB 只记标记，diff 按 Binary 处理。
9. **「继续」前清孤儿进程**（GLM 2.6）：按 STATE.md 的 run_id 逐节点 `cancel_dispatch_node`，再重派。
10. **G1 浮层视觉**（编排者合并后窗口实测）：浮层背景太透，底下的图透出来，决策行难读——用不透明面板底色（沿用 builder 弹窗样式）；「材料将发送至 codex、kimicode、pi、」末尾多顿号，按 vendor 去重再连接；未接入任何资源时「目标」下拉为空，应显示当前节点的 agent/model 文本并允许手填；未指定模型的节点决策行显示「→ /」，应显示「未指定模型」并阻止确认。
11. **changed/ 路径校验补盘符前缀**（GLM 3.1）。

## 次要（一并做）

12. `inspect_dispatch_state` 靠中文子串判未完成 → STATE 头部加机器可读行（GLM 2.7）；`validation_failed` 在 STATE 里单列。
13. `applyDispatchResults` 重建 reply 时保留 `adopted`（GLM 2.7）。
14. confirm 列表与实际派发集合不一致：父未完成的节点标「待上游完成」（GLM 4.5）。
15. 抽 `is_continuing(node)` 消除 preview/run_one 的重复判断（GLM 4.3）；归类模型 input 去掉死 onChange（GLM 4.4）；两套原子写在注释里互相指向（收敛留第 5 期）。
16. 补测试（GLM 5）：`start()` 推进循环抽成纯函数 reducer 并测封顶与停止条件；`artifact_diff` 的删除/二进制/>200KB 分支。
17. REPORT 措辞：「按任务书指定 fallback 复跑」改为「按修正单允许的 fallback」；live harness prompt 未走生产 `serializeContext` 如实标为过渡。

## 不采纳

- 无。
