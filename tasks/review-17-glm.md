# TASK-17 审查报告

**总判：可交付但有未闭合项** —— 实现本体（六铁律、契约、界面、G1、回归口径）与任务书相符，测试真实可红；未闭合项集中在「审查期间工作树被并行 R1 修改，报告与复跑记录未跟上」及几处低危行为。

**审查基础说明（事实）**：`src/resources.rs` 在我审查过程中发生了变化——前一次读取时 `PiAuthReady` 只解析 `ready`，当前版本（resources.rs:1119-1137）解析 `status` + `ready`（注释标注「真机（pi 0.85，09-15 实测）」），并新增了宽松反序列化（resources.rs:59-67）与 `task17_dirty_login_value_reads_as_unknown_instead_of_failing` 测试（resources.rs:2998-3004）。前端各文件经行号锚定复核与初读一致。以下按当前工作树状态评审。

## 问题清单（按严重度）

1. **REPORT-17.md 与代码不同步，验证记录早于最后的 R1 改动** — `tasks/REPORT-17.md:13`（称「只反序列化顶层 `ready: bool`」，代码现为 `status`+`ready` 双字段，resources.rs:1119-1137）；`:16-19` 测试清单缺 `task17_dirty_login_value_reads_as_unknown_instead_of_failing`；`:40` 称「4 个 TASK-17 测试」，现 Rust 侧已有 5 个；`:95` 称「未运行真实 pi」，而 resources.rs:1117 注释引用了真机实测输出。报告中的 cargo test / vitest 数字（407/502）产生于新测试加入之前。**复现**：对照两文件即可。**建议**：更新报告第 2 节描述、测试清单与「未做与边界」（写明 pi 0.85 输出形态的来源），并复跑四条验证命令刷新数字。

2. **「全部探测 / 单 agent 探测」在持有 store_lock 期间串入每 provider 登录检查，锁窗口被拉长** — `src/resources.rs:2194`（`detect_all_resources_at` 顶部持锁直至 save_store）→ `detect_pi`:1032 → `check_pi_provider_logins`。最坏情形（所有 provider 检查都超时）额外阻塞 ⌈P/4⌉×10 秒，期间 `get_resources`（左栏资源 tab 首屏）等命令全部排队。对比：`check_pi_provider_login_at`:2218-2250 特意把检查放在锁外（注释写明「检查阶段不持有资源库写锁……避免阻塞其它资源操作」），探测路径没有同样处理。常规情形 `pi auth check` 亚秒级，实际影响小；**启动不受影响**（`detectAllResources` 唯一调用点是接入页按钮，src-ui/src/tauri.ts:388、ResourcesOverlay.tsx:98）。**复现**：让 `pi auth check` 睡满 10s，点「全部探测」，期间左栏停在「正在读取资源…」。**建议**：不阻塞交付；后续可将登录检查挪到锁外（先合并模型再补登录写回），或至少在报告记为已知限制。

3. **G1 中同时命中额度用尽与未登录的节点在两个分区各渲染一行** — `src-ui/src/components/center/graph/DispatchOverlay.tsx:594-627`。`quotaBlockedIds` 与 `loggedOutIds`（:142-143）独立计算，交集节点在「额度用尽」「未登录」两个 section 各出现一行，共享同一 `nodeIds` 勾选状态（两行勾选框联动）。**复现**：目标模型的 provider 为 `logged_out` 且 `quota_exhausted_until` 有效，打开确认派发屏。**影响**：重复展示，无功能错误；默认不勾与勾回收敛（`convergeQuotaSelection`:198）两分区口径一致。**建议**：分区去重（ loggedOut 区排除已进 quota 区的 id），或接受现状并在报告注明。

4. **pi 二进制整体缺席时，库中旧登录状态原样保留，界面仍按旧值显示** — `src/resources.rs` detect_into 中 pi 分支 `let Some(binary) = resolved else { return; }`：pi 未检测到时模型不重合并，上次存下的 `logged_in` 继续显示「已登录」列，卡片却标「未检测」。「命令缺失 → unknown」在「重新检查」路径已满足（resources.rs:2249-2251 → Unknown），探测路径则是不检查、不覆盖。**复现**：卸载/改名 pi 后点「全部探测」，展开 pi 卡，旧已登录 provider 仍正常列模型。**建议**：低危；pi 未检测时把该 agent 的 login 统一刷成 unknown，或报告注明。

5. **登录检查测试用进程级环境变量传参（`std::env::set_var`）** — `src/resources.rs:2904-2916`（`AICROSS_PI_ARGS_LOG`）。cargo test 默认同进程多线程并行，set_var/remove_var 是进程全局副作用（Rust 2024 已标 unsafe）；当前仅此一个测试读写该变量，无实际竞争。**复现**：无（潜在隐患非现行故障）。**建议**：后续改为固定临时文件名或 stdin 传参，不阻塞本期。

6. **G1「登录目标 → 默认不勾」的组件级接线只有源码字符串钉住** — `src-ui/src/resources/__tests__/resources-login-17.test.tsx:129-131` 断言的是 DispatchOverlay.tsx 源文本（分区标题与文案），而 `blockedIds = [...quotaBlockedIds, ...loggedOutIds]`（DispatchOverlay.tsx:144）若被删掉 `loggedOutIds`，现有测试不会红（`loggedOutNodeIds` 与 `initialBlockedSelection` 的单测各自仍绿）。这与仓内既有惯例一致（review-defaults-12.test.ts:13 同样用 `?raw` 源码断言，额度接线的覆盖方式相同）。**复现**：从 :144 移除 loggedOutIds 后跑专项测试。**建议**：接受（与惯例一致）或补一个行为级接线断言。

## 核过没问题的要点

- **铁律 1**：argv 逐字 `auth check --provider <p> --json --no-refresh`（resources.rs:1084-1097）；全仓 grep `--credentials` 仅出现在测试假脚本与断言（resources.rs:2935）；`PiAuthReady` 只声明 `status`/`ready`，serde 丢弃其余字段含 `credentials`（:1119-1125）；`credentials`/`reason` 从不被读、不落盘、不进日志（`run_process` 输出仅驻内存，此路径的失败/超时静默映射 Unknown，:1103-1113）；resources.json 只新增 `login`/`login_checked_at` 两字段（:122-127）；测试断言落盘 JSON 不含 `do-not-read`（:2939-2946）。
- **铁律 2（契约）**：缺字段 serde 默认 unknown（resources.rs:51-55 + 测试 :3007）；认不得的值宽松读为 unknown 不炸整库（:59-67 + 测试 :2998）；并发分批 chunks(4)、峰值恰为 4 有实测断言（:1063、:2952-2974）；单次 10s（:43）；缺命令/超时 → unknown 有真实子进程测试（:2976-2995）；只对 pi 分支生效（detect_into 其余 agent 路径未动），前端也只在 `agentId === 'pi'` 时分组（ResourcesTab.tsx:259、ResourcesOverlay.tsx:453）。
- **铁律 3（界面）**：已登录 provider 正常列且组内未验证仍折叠（splitUnverified 按组执行）；未登录/未检查分别合成「未登录 N 个 provider · 展开」「未检查 N 个 provider · 展开」，展开后仅 provider 名 +「重新检查」，模型不铺开（渲染测试断言 `not.toContain('hidden-model')` 等）；四列等高 `repeat(4, minmax(0,1fr))` + `grid-auto-rows: minmax(0,1fr)`、页面 `height/max-height:100vh; overflow:hidden`、卡片内 `.agent-card-models{overflow:auto}`（resources.css:469-479、543-551、602-608）；左栏同口径（ResourcesTab 与 Overlay 共用 `splitPiProviderGroups`）；G1 对 logged_out 提示且默认不勾（DispatchOverlay.tsx:143-145、611-627；dispatch.ts:1048-1061）；「全部探测」经 `detect_all_resources` 刷新登录状态（resources.rs:1032）。
- **回归**：`list_dispatchable_models` 仍不过滤（orchestration.rs:712-730 → `detected()`），`login` 随 `ResourceModel` 全量透传前端（graph/types.ts:141-143）；TASK-15 的 `agentDisplayName`/`AgentBadge`/启动器相关文件本期未触碰，接入页与左栏继续走同一套显示名与图标；启动路径只读 `get_resources`，不触发探测，登录子进程不拖慢启动。
- **测试能红、非同义反复**：假 pi 是真子进程（.cmd/.sh 经 `run_process` 真拉起，argv 日志断言去重后恰 4 次调用、逐字 argv、无 `--credentials`）；超时例真睡 1s 配 30ms 预算；并发例用真实线程 + 峰值计数；前端测试跑真实现非 mock。报告称做过注错复红（改并发上限、去 `--no-refresh`、翻默认值等均失败后恢复），本次审查无法重跑命令核实，仅静态认定各测试对相应改动敏感。
- **文案**：已登录 / 未登录 / 未检查 / 重新检查全仓一致（types.ts:242-244、292-298；两处组件；G1 分区），术语表已加行（docs/MANUAL.md:226），全角标点，无内部词。
- 小事实现（不算问题）：`.agent-card-logged-out` 容器无专属 CSS 规则（纯块级包裹，无碍）；`check_pi_provider_logins_with_runner` 用 `join().ok()` 吞掉 runner panic（生产 runner 不会 panic）；provider 去重区分大小写（`providerGroups` 本就区分，属既有口径）。