# TASK-09 修正轮 R1

先读 `tasks/TASK-09-resources-ui-and-smoke.md`、`tasks/REPORT-09.md`，再读 `tasks/review-09-ds.md`（判不可交付，12 条）与 `tasks/review-09-glm.md`（判可交付，7 条）。逐条处理，每条在 `tasks/REPORT-09.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。Rust 只动 `src/resources.rs`、`src/dispatch.rs` 冒烟接线段；保持文件原有换行；不起窗口、不发真机请求、不 commit、不 push。

## 必修（按严重度）

1. **原生判定 = 原生 agent 且非第三方端点**（DS 1、GLM 3）：复用已有 `claude_third_party_endpoint()`（及 codex/kimi 若有同类判定）：走第三方端点的 claude 模型算第三方 → `unverified`，首次派发自动验证；接入页对它显示「验证」按钮。口径写进 `docs/PRODUCT.md`「资源冒烟口径」一句。补测试。
2. **provider 为空不得静默跳过**（DS 2）：`verify_on_first_use` 对 `unverified` 一律验证（provider 空就按 agent 自身端点验），验不了的返回失败并写明原因；G1 承诺与行为一致。补测试。
3. **验证窗口可取消且不占写锁**（DS 3、4、GLM 4）：`verify_on_first_use` 移到取项目写锁与 worktree/快照/材料之前；冒烟进程走 `run_process_registered` 以便 `cancel_registered` 能叫停；取消时该节点按「已取消」返回而不是吞掉。补测试。
4. **并发首派与写回**（DS 5、6、GLM 7）：同一模型两路同时首派只冒烟一次（模型级互斥或「验证中」状态），写回失败不吞——记 note 并让该次派发失败。补测试。
5. **UI 与可访问性**（DS 7–11、GLM 2、5、6）：unverified 提示不用红（`DispatchOverlay.tsx:449`）；「详情」按钮补样式并删死规则；左栏多 provider 同名模型加 provider 后缀且点击目标与所选行一致；徽标与 verified 绿点对比度 ≥4.5:1（浅色主题）；焦点环 2px 不透明；悬停提示改成可键盘触达（`aria-describedby` 或点击展开）；9px 字号与 6/10px 间距改回纪律值；厂商色统一从 `graph/vendor.ts` 取。补断言。
6. **文档与清理**（DS 12、GLM 1）：`MANUAL.md` 删掉不存在的「一个月重验」句；`STATUS_PENDING` 死导出删除；注释/测试名与实现对齐。
