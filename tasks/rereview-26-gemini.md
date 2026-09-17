# TASK-26 R1 复审 · Antigravity / gemini-3.8-flash-high（盲审，材料内联）

### 总判

可交付。

---

### 问题清单

无。清单中的三项必修项均已正确闭合，测试真实接线且具备反向变异红测能力，未触碰禁区文件，未引入新缺陷。

---

### 核过没问题的要点

1. **必修 1 · 归类运行目录定向清理已彻底闭合**
   - [`src/dispatch.rs`](file:///src/dispatch.rs#L1889-L1900) 在 `dispatch_stage_core` 中通过 `ClassificationRunCleanup` RAII guard 守护运行目录生命周期。仅对以 `proposal-classify-` 开头且不包含路径分隔符与冒号的合法单目录在作用域结束时执行 `fs::remove_dir_all`。
   - 无论正常完成还是提前错误退出均会触发清理；若目录不存在则静默放行，其余删除错误仅输出至 `eprintln!`，不污染派发结果。
   - 补充的 Rust 测试真实经由 `dispatch_stage_core` 入口，分别覆盖正常成功、普通文件冲突导致的目录创建失败两条路径，并在多线程并发用例中验证各删各的、同前缀哨兵目录不被误删。

2. **必修 2 · 恢复原生 `isContentEditable` 优先识别**
   - [`src-ui/src/components/center/ProposalBar.tsx`](file:///src-ui/src/components/center/ProposalBar.tsx#L104-L115) 的 `proposalTargetFlags` 优先采信 `typeof element?.isContentEditable === 'boolean'` 的浏览器计算结果，使外层 `contenteditable="true"` 内部嵌套的 `false` 不可编辑区能被正确识别为非输入控件，同时能原生支持 `plaintext-only`。
   - 在测试替身或无原生属性环境回退选择器时，补充了 `plaintext-only`，保留了空值与 `true`，排除了 `false`。
   - [`src-ui/src/__tests__/proposal-20.test.ts`](file:///src-ui/src/__tests__/proposal-20.test.ts#L62-L96) 新增的两组用例均通过 `proposalShortcutFromTarget` 真实快捷键派发链路执行，完整覆盖原生与回退分支，在回退选择器漏项或忽略嵌套 false 时均能稳定红测。

3. **必修 3 · 备选已真跑后回写主记录分支补齐测试并验证能红**
   - [`src/dispatch.rs`](file:///src/dispatch.rs#L7885-L7930) 新增测试 `task26_r1_logged_out_after_failed_fallback_updates_main_record`，构造主尝试失败后、备选 1 真跑失败且备选 2 因未登录跳过的场景。
   - 测试通过本地 Python stub 留痕精确断言实际调用仅有 `["p0", "p1"]`，且真实检查了 `log_dir` 下带有 `attempt: 主\n` 的主尝试留痕文件，确认其成功写入「跳过 cc-switch/logged-out（未登录）」。注释掉回写逻辑后该测试确定红测。

4. **工程与规范约束达标**
   - `src/server.rs`、`src/main.rs`、`src-ui/src/tauri.ts` 等禁区文件均保持未修改。
   - 保持文件原有换行符，未运行 `cargo fmt`；全量 Rust 编译与测试通过（`cargo check --all-targets` 0 warning，464 passed），UI 单测全部通过（609 passed），前端打包通过。
