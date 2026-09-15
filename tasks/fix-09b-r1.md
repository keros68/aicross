# TASK-09b 修正轮 R1

先读 `tasks/TASK-09b-resources-polish.md`、`tasks/REPORT-09b.md`，再读 `tasks/review-09b-ds.md`（与 `tasks/review-09b-glm.md`，若存在）。逐条处理，每条在 `tasks/REPORT-09b.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」，并更正报告里对比度表「深色」列的错误数据。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。Rust 只动 `src/resources.rs`；保持文件原有换行；不起窗口、不 commit、不 push。

## 必修（按严重度）

1. **浅色主题压暗色真正生效**（DS A1）：`AgentBadge` 内联 `--agent-tone` 覆盖了 `resources.css` 的浅色压暗规则；改成压暗值由 CSS 变量按主题选择（或内联时按主题传不同值），并把对应的假绿测试改成能红的断言（浅色下渲染出的颜色是压暗值）。
2. **对比度**（DS A2、A3）：未验证灰点在 cappuccino ≥ 3:1（非文本 UI 元素下限）并加 1px 描边或改用更深灰；kimi 徽标在 cappuccino / mint / dark 都 ≥ 3:1（按需分别压暗）；报告的对比度表重算（对各主题实际背景色），写清计算基准。
3. **单 provider 的 pi 显示 provider 名**（DS 3）：单 provider 不出组头但行悬停/接入页详情要带 provider；`types.ts` 不再把它抹成空串。
4. **键盘可达**（DS A5）：未开对话图时模型行不用 `disabled`，用 `aria-disabled` + 提示，仍可聚焦读到说明。
5. **`resolved_source`**（DS 7）：`ResourceModel` 追加 `resolved_source: "alias-table" | "identity"`（`#[serde(default)]`，空按 alias-table），别名表修正时只覆盖 `alias-table` 来源的值；补测试。
6. **许可与逃生门**（DS 8）：仓库 NOTICE（或 `src-ui/NOTICE`）的 aicross 段加 simple-icons 归属与 tag；OpenAI 标志加一条测试钉住「删掉 `BRAND_MARKS.codex` 时 AgentBadge 回退字母」。
7. **小项**（DS 9、10）：20px 越阶字号与 `.res-dialog gap:10px` 归位；`BrandMark.title` 死字段删或用；真身前缀剥离补大小写/不符边界测试；`hintId` 用节点唯一键防撞；报告「sm/md 同尺寸」改准。
## GLM 补充

GLM（`tasks/review-09b-glm.md`）判可交付；其 2、4、5 与上面第 7、2、7 条同题。额外的：

8. **原生 claude 的真身覆盖路径**（GLM 1）：原生模型不冒烟、首派也不验，别名表漂移后无法被纠正——文档口径改成「原生 claude 的真实 ID 来自别名表，派发留痕里的 `modelUsage` 键会在每次派发后覆盖 `resolved_id`（来源 identity）」，并在 `run_one` 派发成功后按留痕 identity 回写 `resolved_id`（Rust 只动 `src/resources.rs` 提供写回函数；若接线需要改 `src/dispatch.rs` 则在报告写明由下一轮做，本轮只改文档与函数）。补测试。
9. **发布前检查**（GLM 3）：`docs/MANUAL.md` 或 `tasks/RELEASE-CHECKLIST.md` 加一条：对外分发安装包前删 `BRAND_MARKS.codex`（或换成字母兜底），并有测试钉住回退。
