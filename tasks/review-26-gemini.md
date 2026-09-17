# TASK-26 盲审 · Antigravity / Gemini 3.8 Flash (High)

2026-09-17，材料全部内联。

---

**总判**：六条任务已全部真实闭合，未触碰任何禁区文件，测试均具备红测验证能力且关键前端项均走真实接线，整体实现质量符合任务书要求；仅在快照读取时机与 contenteditable 嵌套边界上存在两处低危设计副作用，需关注存量清理脚本的前缀兼容性。

---

### 严重度编号清单

#### 1. [低] 派发快照读取过早导致主尝试耗时期间的登录/配额变化无法感知
- **文件:行**：`src/dispatch.rs:3351`
- **问题**：`resource_models_snapshot()` 在主尝试 `run_one_attempt` 之前立即获取。主尝试为模型远程调用，通常耗时数秒到几十秒；若在此期间用户在 UI 刷新登录或备选模型配额冷却到期，由于引擎沿用主尝试开始前抓取的旧快照，后续备选遍历时仍会将其识别为 `LoggedOut` 或未解冻，导致误跳过已就绪的可用备选。
- **复现**：配置主尝试模型超时/失败，备选模型初始为未登录；触发派发后，在主尝试运行的 5 秒窗口期内外部更新资源库将备选置为已登录；主尝试失败后，派发引擎依然基于开始时的旧快照直接跳过该备选并记为「跳过（未登录）」。
- **改法**：将 `resource_models_snapshot()` 的获取时机推迟至主尝试失败、确实需要进行备选分支挑选时再执行一次快照读取；或者维持单次派发只读一次，但需接受派发窗口期内的状态冻结语义。

#### 2. [低] 移除 `isContentEditable` 后在嵌套不可编辑区内仍会被误判为输入控件
- **文件:行**：`src-ui/src/components/center/ProposalBar.tsx:108`
- **问题**：直接删除了 `element?.isContentEditable === true` 检查，仅依靠 `closest('input,textarea,[contenteditable="true"],[contenteditable=""]')`。在富文本嵌套场景中（例如外层为 `<div contenteditable="true">`，内层包含局部显式不可编辑的 `<div contenteditable="false">` 标签），内层元素虽然本身不可编辑，但 `closest` 仍会向上寻址命中外层的 true 容器，从而被误当做输入控件；此外，`contenteditable="plaintext-only"` 等合法编辑模式也会因选择器未覆盖而漏判。
- **复现**：构造 DOM 目标为 `<div contenteditable="true"><div contenteditable="false"><span id="target">不可编辑</span></div></div>` 的 target，按 Enter，`target.closest('[contenteditable="true"]')` 返回外层容器，建议条将该区域当成输入控件而放行按键，快捷键失效。
- **改法**：判定优先使用原生 DOM 的 `element.isContentEditable === true`（浏览器会正确计算继承与 false 遮蔽），属性选择器仅作为不支持该属性环境或特定祖先选择的兜底。

#### 3. [极低/提示] runId 动态加后缀若遇全字匹配清理逻辑会导致目录遗留
- **文件:行**：`src-ui/src/console-channel.ts:254`
- **问题**：`runId` 与 `nodeId` 由固定的 `'proposal-classify'` 改为 `'proposal-classify-' + event.proposal_id`。如果工程现有存量维护脚本、启动清理逻辑或其它检索工具依赖严格的全路径名 `runs/proposal-classify` 进行清理或查找，将无法匹配并跳过这些动态生成的子目录，导致日志留痕堆积。
- **复现**：存量脚本若以 `dir.name === 'proposal-classify'` 查找并删除归类目录，并发执行归类后生成了 `proposal-classify-p1` 与 `proposal-classify-p2`，清理脚本无法扫描到目标目录。
- **改法**：核查工程内所有涉及归类目录清理的逻辑，确保均统一为 `starts_with("proposal-classify")` 或 glob 通配符匹配。

---

### 六个问题逐项核对结论

1. **六条是否闭合；56 确无生产调用方的证据与误伤评估**
   - **闭合情况**：六条需求均已实现，`tasks/fix-04-r5-backlog.md` 对应项均已按规范划线并注明 `~~…~~ 已修（09-17，TASK-26）`。
   - **56 证据链**：报告列出了 `src/resources.rs` 中全部 5 处 `detect_into` 调用，经逐行核对：2356 处 pi 已在进入前被 `prepared_pi` 分支提前拦截并短路；2388 处明确位于 `agent_id != "pi"` 的 else 分支内；2620 处固定针对 `manual-*`；4475 处为 `#[ignore]` 本地测试且已在本次同步重构为标准接入方式。生产调用链确实没有任何将 `pi` 传入 `detect_into` 的可能。
   - **误伤防范**：`detect_into` 开头补充了 `if !detect_into_supports_agent(agent_id)` 显式报错拦截，未来若有代码误接回旧路径会立即抛错，不会发生静默降级或语义回归。

2. **66 判定入口复用与下拉关闭后吃键状态**
   - **入口复用**：已严格复用 `src-ui/src/lib/focus-enforcer.ts` 的既有入口 `hasOpenModal`，在其内部扩展了 `root.querySelector('.preset-select[open]')`，未在 `ProposalBar.tsx` 内重复另写选择器。
   - **吃键恢复**：预设下拉依赖原生 `<details class="preset-select">` 的 `open` 属性。下拉展开时带 `open`，建议条放行 Esc；下拉关闭后 `open` 属性消失，`hasOpenModal` 返回 `false`，建议条在捕获阶段立即恢复正常拦截并吃键，未做成永久放行。

3. **67 改完后正常的 contenteditable（空值、true）识别情况**
   - 识别完全正常。选择器 `[contenteditable="true"],[contenteditable=""]` 能同时匹配显式声明 `contenteditable="true"`、显式声明 `contenteditable=""` 以及 HTML 属性简写 `<div contenteditable>`（在浏览器 DOM 解析后属性值均为空字符串 `""`），这些正常输入目标均会被判定为输入控件并放行按键。

4. **68 动态 runId 对旧清理/查找逻辑的影响**
   - 任务书明确指引「仍是可预期的固定前缀，便于清理」。只要现有或后续清理逻辑采用前缀匹配（`starts_with("proposal-classify")`），均能正常索引并清理；但若存量清理逻辑硬编码了精确名称 `runs/proposal-classify`，则会导致带有后缀的目录漏被清理。

5. **57 一次派发只读一次库的时序与登录状态过期问题**
   - **确实存在时序过期窗口**：由于快照是在主尝试执行前抓取的，主尝试执行耗时较长期间如果外部发生登录状态变动（如后台探测成功或用户重新登录），引擎在 fallback 时依然使用旧快照，会拿到陈旧的登录状态。但这严格符合任务书「一次派发读一次、候选间复用」的性能与降锁要求，属于派发周期内的受控折中。

6. **禁区文件与测试质量核对**
   - **禁区文件**：未修改任务书指定的 7 个禁区文件（`console-chat.ts`、`console-session.ts`、`ConsoleChat.tsx`、`ConsoleChat.css`、`app-state.tsx`、`CenterPanel.tsx`、`docs/MANUAL.md`），亦未改动 `src/server.rs`、`src/main.rs`、`src-ui/src/tauri.ts`。
   - **红测与真实接线**：
     - 56、57、58 的 Rust 定向测试均通过断言或变异验证证明能红；57 走真实 `run_one` 派发与留痕落盘断言。
     - 66、67 的前端测试均通过 `proposalShortcutFromTarget` 真实快捷键分发接线函数施测，不是仅测纯底层纯函数。

---

### 核过没问题的要点
1. **57 留痕落盘完整**：跳过未登录备选的说明不仅追加到结果返回体，且在发生备选重试时，通过 `sync_record_runner_note` 正确反向更新了磁盘上的主尝试留痕文件 `main_record.0` 的 frontmatter。
2. **58 口径固定不偏航**：`pi_login_from_check` 保持现有行为不变，注释准确阐明了「解析到 ready 但非 0 退出码判为 LoggedOut」的技术口径与理由，并补齐专项测试钉住该行为。
3. **格式与工程约束达标**：未执行全局 `cargo fmt`，新增与修改的 Rust/TS 文件保持了原有 LF 换行，编译零 warning，测试套件完整跑通。
