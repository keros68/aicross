# TASK-07 代码审查

## 1. 需求完整性

**GraphTab.tsx 属禁区文件，被改了**。`src-ui/src/components/center/graph/GraphTab.tsx:52-62`：任务书「不做」明确不动 `components/center/graph/`，B1 给的机制是「现有的 `aicross:` 自定义事件或 store 动作」。核实：仓库里现有事件只有 `aicross:dispatch-settings`、`aicross:open-orchestration`，没有 insert-mention；GraphTab 的 @ 弹层（`GraphTab.tsx:474`）是内部 state，无现成事件可用。实现者选择新造事件并改禁区文件——技术上是最小方案且只追加，但这是任务层面的范围决定，应先回报再动，而不是在 REPORT 里事后披露。这是本期与 phase/4 分支唯一真实的代码冲突面。

其余对照结果：
- kimi 完整别名：✓ `parse_kimi_config` 用 TOML 表键做 id（`src/resources.rs:527`），测试 `kimi_alias_keeps_full_prefix` 断言 `contains('/')`；channels 端 kimi 要求 `/`（`channels/mod.rs:178`）对得上。
- pi 三 provider 解析 + auth check：✓ `parse_pi_sources` 逐 token 精确匹配（`glm-4` 不命中 `glm-4.7` 有测试），auth check 按 provider 逐一跑、未就绪标进 meta。注意 provider 集合来自合并结果而非固定三家——行为正确（多于三家也覆盖）。
- cc-switch 降级：✓ 三层到位——`ensure_agents` 迁移删除历史 agent、`connected()` 排除 `cc-switch`（`src/resources.rs:278-284`，双保险）、探测只产 `CcSwitchHint`；路径取设置项，空则跳过（`src/resources.rs:753-763`）。
- 冒烟复用：✓ `run_smoke`（`src/resources.rs:1015`）只用 `channels::build_invocation` + `process::run_process` + `channels::parse_output`，无复制进程代码。
- 左栏：✓ 只列已接入、按 agent 折叠、组头名+版本、点行发 `aicross:insert-mention`、栏底汇总与「管理 →」。
- 接入页：✓ 卡片网格齐全，状态动作成对（待冒烟↔冒烟、已接入↔移除、已检测↔探测）。入口按钮在齿轮左侧 ✓。

偏差/缺口：
- `detectAllResources` 命令注册 + tauri wrapper（`src-ui/src/tauri.ts:346-349`）但 UI 无任何入口——死接口面，第 1 期有「全部探测」按钮没搬，要么补按钮要么删。
- 左栏点模型时 GraphTab 未挂载则事件静默丢失（REPORT 已披露）。「去接入」入口存在，但点模型无反馈仍是体验缺口，建议 EventsTab 端无图时禁用模型行。
- 无移除手动 agent 的途径，manual-* 在 tools.json + resources.json 里只增不减（不在本期需求内，记 Later）。
- i18n：新界面除 2 个 key 外全部硬编码中文（`ResourcesOverlay.tsx`、`ResourcesTab.tsx`、`resources/types.ts:resourceSummaryText`）。app 有 11 个语言文件且 useT 有 en 回退，英/日/韩等用户看到整页中文。任务书未点名要求，但与现有体系不一致，至少应在 REPORT「没做什么」里写明——目前没写。

## 2. 逻辑正确性

- **合并规则**：✓ `merge_detected_models`（`src/resources.rs:858`）保留 tier/status/smoked_at/smoke_model/smoke_failure/vendor(非空)/note，新条目待冒烟，消失标「未在事实源中」，`source.starts_with("申报(")` 跳过。与任务书逐条对得上。
- **load/load_or_default**：✓ 损坏 → `load` 报错 → 命令在 `save_store` 之前 `?` 返回，不覆盖。未发现问题，但没有测试钉住（见第 5 节）。
- **原子写**：✓ 临时文件 + sync + rename。`remove_file` 后重试的兜底窗口是从 `orchestration.rs:254` 原样继承的（Windows 上 `fs::rename` 实际走 MOVEFILE_REPLACE_EXISTING，兜底极少触发），不算本期新问题。
- **并发读改写会丢更新，且无任何 CAS/锁**。`src/resources.rs:273`（`save_store`）无条件覆盖；写方共 7 个命令，其中 async 命令在 spawn_blocking 线程、sync 命令（add/update/remove）在主线程，后端层面完全可并发。目前唯一串行化手段是 ResourcesOverlay 的 `busy` 全按钮禁用——UI 纪律，不是保证。最坏窗口在 `smoke_agent_models`（`src/resources.rs:1396`）：加载 store → 循环冒烟（每条最长 60s）→ 结束才整盘写回，期间任何其他写方的更新（比如将来 dispatch 侧标记）会被整体抹掉。第 1 期抓过 CAS 问题的话，这里的答案是：**退化了，当前没有任何 CAS**。建议：一个进程级 `static resources_lock: Mutex<()>` 包住所有 load→save 序列，成本一行；或保存前重读合并。第 4 期 dispatch 若也开始写 resources.json，这个洞就是真的。
- **冒烟真身判定**：pi ✓——`parse_pi`（`channels/mod.rs:535`）严格比对 provider+model，403 经 `stopReason=="error"` 出错，channels 自带测试覆盖。claude 第三方：`src/resources.rs:1046` 用**控制台进程自己的 `ANTHROPIC_BASE_URL` 环境变量**判断是否第三方端点——若用户在 `~/.claude/settings.json` 里配的 base URL（不经过继承环境），比对被静默跳过。任务书字面要求的是「第三方端点比对」，实现是它的启发式近似；建议至少把「未能核对」记进 meta。codex/kimi 无真身比对（任务书不要求，pi/claude 已覆盖主风险）。
- **resolve_cli 与 channels resolve_binary 重复**：`src/resources.rs:322-369` 与 `channels/mod.rs:354-395` 逐行近似（含 AI_CROSS_PEER、CREATE_NO_WINDOW），差异是 kimi fallback channels 走 `spec.fallback`。已见一处行为漂移：探测路径 kimi 解析到 `.cmd` 会成功（`version_for` 经 cmd.exe 包装能跑），冒烟/派发路径 `build_invocation` 会拒（「需要原生 kimi.exe」）——同一个 agent「探测通过、冒烟必败」。channels 不许改是约束，但应在 channels 解禁后把 `resolve_binary` 提为 `pub(crate)` 收敛；本期至少在 resolve_cli 注释里记下这个已知漂移。
- 小问题：claude 未检测到时提前 return（`src/resources.rs:414`），既有预设条目不刷 missing 标记也不合并；codex/kimi 未检测到却继续读事实源。同一函数两种策略，至少注释说明是故意的。
- `add_manual_agent` 吞掉探测错误（`src/resources.rs:1375` `let _ = errors`），用户登记失败 agent 时零反馈，只能看到「未检测」。返回 errors 或并入 store meta 都行。

## 3. 安全

未发现实质问题。核对项：
- 密钥：全程无凭据读取；`models-store.json`/`models_cache.json`/`config.toml`/`cc_switch.py list` 输出都不是凭据文件；冒烟子进程靠自身配置认证，控制台不碰 key。✓
- `smoke_failure` 落盘前过 `safe_summary`（sk-/URL 凭据/query secret/Bearer/环境变量值 → [已隐藏]，截 200 字，`src/resources.rs:1214`），有测试。✓
- hints 只含 name/has_token/模型 id，无 token 值。✓
- fixtures 全部假数据，无 key/token/带凭据 URL。✓
- 无 shell 拼接：所有子进程直 argv；`cmd.exe /d /s /c` 分支是第 3 期 process.rs 既有代码，参数仍按 argv 传。✓
- cc_switch.py 路径来自前端 `loadDispatchSettings().ccSwitchPath`（`ResourcesOverlay.tsx:62`），非硬编码。✓
- 一个理论性 nit：`safe_summary` 扫全部环境变量做值替换，任何恰好等于某敏感值的普通文本都会被替换——误伤方向是安全的，不处理。

## 4. 代码质量

- `write_json_atomic`（`src/resources.rs:234-252`）与 `orchestration.rs:254-292` 整函数重复。建议提到共享 util 模块；现在两份已经在注释上开始分叉（resources 版丢了「MoveFileExW 可覆盖」的解释）。
- GraphTab 监听本身：纯追加、空依赖数组、函数式 setState，无既有逻辑改动。✓ 唯一隐患是多 GraphTab 同时挂载时每个实例都会收到事件（当前单实例，记下即可）。
- `t('explorer.tab.resources' as any)`（`TitleBar.tsx`、`Explorer.tsx`）：key 已加进 en.ts/zh-CN.ts，就是合法 `I18nKey`，`as any` 完全多余还放弃了拼写检查。删掉。
- 样式：全部用主题 token，四套主题变量核对过都存在。✓ 仅 `.resource-status.connected/.failed` 的底色是硬编码 rgba（`resources.css`），与 `--green/--red` 色值耦合，主题改色时不同步——小疵。
- `toml = "0.9"` 必要：kimi 事实源是带内联注释的 TOML，手写正则必然脆（有 `kimi_config_inline_comments.toml` fixture 为证）。✓
- `ResourcesOverlay.tsx:509` 冒烟单条成功后 `setError('')` 会顺手清掉无关的全局错误条；`smokeAgentModels` 返回后 `finishOperation` 把 hints 覆盖成空 Vec（`smoke_agent_models` 恒返回空 hints，`src/resources.rs:1434`）——**点一次「冒烟」，cc-switch 提示条就从页面上消失了**，下次探测才回来。要么 smoke 命令带回 hints，要么前端在 cc_switch_hints 为空时保留旧值。
- 手写中文状态字符串（`STATUS_CONNECTED` 等）在 Rust 常量、TS 常量、i18n 三处并存——第 1 期遗产，契约不改就接受。

## 5. 测试

存在且断言真实的：Rust 11 个非 ignore 测试覆盖四事实源解析、三 provider、别名前缀、hints-only、迁移、合并保留、精确 token、auth 状态、pi provider 必填、脱敏；前端 9 个 `it` 断言计数/分组/拼接规则。质量合格。

问题：
- **「第 1 期既有测试全部并回」不成立**。`fixtures/resources/pi_smoke_success.jsonl`、`pi_smoke_mismatch.jsonl`、`pi_smoke_thinking_only.jsonl` 三个 fixture 搬来了但**没有任何测试引用**（全仓 grep 无 `include_str` 指向）。等价行为（真身不符、403 error event）已由第 3 期 channels 测试用自己的 fixture 覆盖，所以是死重量而非覆盖漏洞——要么删这三个 fixture，要么在 REPORT 里如实改口「等价覆盖在 channels，第 1 期 smoke 解析测试未并回」。
- 「申报条目不标未在事实源中」这条合并规则**没有测试**：`merge_preserves_user_fields_adds_pending_and_marks_missing` 里消失条目的 source 是 `读:old`。给 source 为 `申报(...)` 的条目补一个不标标记的断言。
- `load_or_default` 损坏不覆盖、`smoke_agent_models` 部分失败不中断整批，均无测试。
- REPORT 说前端「18 个用例」，实际是 9 个测试、18 个断言——数字对不上口径，测试维度外的报告准确性问题。
- 最该补的 3 个：① 申报 source 跳过 missing 标记；② 损坏 resources.json 时 add/update 命令报错且原文件字节不变（钉住「不覆盖」保证）；③ `smoke_agent_models` 两条模型一成一败 → store 里一已接入一冒烟失败、errors 恰含一条。

## 6. 真跑证据

可信，与代码核对一致：
- 「不写 resources.json」说法与代码相符：`live_detect_four_agents`/`live_smoke_pi_models` 都在内存 store 上直接调 `detect_into`/`smoke_one_in_place`，不经命令层的 `save_store`；session 落 `~/.aicross/sessions/smoke` 是 `build_invocation` 的 `--session-dir` 所致，已披露。✓
- 冒烟输出形状与代码路径吻合：成功条 `smoke_model=zai-coding-cn/glm-5.3-flash` 正是 `parse_pi` 的 identity 拼法；失败条 403 文本经 `safe_summary` 200 字截断，`AccessDenied.Unpurchased` 与 channels 测试 fixture 的错误串一致。✓
- 误跑第 3 期 live 测试多花 5 次请求、MSIX pwsh 测试失败，都如实记了，没有粉饰。✓
- 唯一不实：「18 个用例」（见上）。

---

## 结论

**对照 TASK-07 原始需求：基本完整实现，A/B 各条与验收 1–4 均有着落，真机验收达标；但有一处越界改动和若干缺项。**

缺项列表：
1. GraphTab.tsx 被改动（禁区文件，纯追加，未事前确认）——唯一的真违反。
2. 并发写 resources.json 无锁无 CAS，仅靠前端 busy 串行化；smoke_agent_models 长持旧 store 放大丢更新窗口。
3. 冒烟 claude 第三方端点判定依赖控制台 `ANTHROPIC_BASE_URL` 环境变量，settings 文件配置的场景静默跳过比对。
4. 接入页点「冒烟」后 cc-switch 提示条被空 hints 覆盖消失。
5. 「申报条目不标」无测试；损坏不覆盖、批量冒烟部分失败无测试；三个 pi_smoke fixture 成死文件，「第 1 期测试全部并回」的说法不实。
6. detectAllResources 命令 + wrapper 无 UI 入口（死接口）；无移除手动 agent 途径。
7. 新界面硬编码中文未进 i18n，REPORT「没做什么」未披露。
8. REPORT「18 个用例」实为 9 个测试。