# TASK-01 资源接入 · 代码审查

## 1. 需求完整性

**1.1** `resources.rs:668-706`（merge_detected_models）— 合并规则第一条“同 `(agent, provider, id)` 更新元数据”只做了一半：`model.note = remove_note_marker(&existing.note, MISSING_MARKER)` 用旧 note 整体覆盖新探测到的 note，事实源侧的 priority / effort / 上下文长度 / 描述在首次探测后被永久冻结，后续变化不再刷新。任务书把 note 列为用户保留字段、其余为“元数据”，实现把两者混在一个字段里且偏向旧值。建议：探测元数据与用户备注分段存储（如 note 里约定前缀，或加独立字段），合并时刷新元数据段、保留用户段。

**1.2** `resources.rs:465-488`（detect_pi）— 任务书把 `pi auth check` 定位为“就绪状态”信息，实现做成硬门槛：任一 provider 未就绪就 `return Err`，整个 pi 探测失败，其他已就绪 provider 的模型全部丢失。建议：按 provider 记录未就绪（进 errors 或 note），不整体失败。

**1.3** `resources.rs:459-464` — `auth check` 私自加了 `--no-refresh`，任务书命令模板没有此 flag。若 pi 版本不认它，auth check 全部失败，叠加 1.2 直接瘫痪 pi 探测。建议：确认 flag 存在或去掉。

**1.4** `Resources.tsx:198-201` — 任务 D 明确“状态标签与动作成对：待冒烟 ↔ 冒烟、已接入 ↔ 移除”，实现无论什么状态恒显「冒烟」「移除」两个按钮。建议按 status 条件渲染。

**1.5** `resources.rs:542-556` — claude 预设表用 CLI 别名 `opus/sonnet/haiku` 当模型 ID。任务书写“模型 ID”，别名与真实 ID（claude-…）不同，`smoke_model` 回填时会出现别名/真名不一致的观感。至少应在 note 标明“CLI 别名”。

**1.6** `resources.rs:74-95, 263-310` — codex 事实源的 `fetched_at` 完全未读取。source 里记的是探测时刻，用户会误以为缓存是刚抓的。建议把 fetched_at 写进 note 或 source。

**1.7** 验收 3 两项未达成（pi `--list-models` 本机验证、`zai-coding-cn/glm-4.7` 冒烟成功并记 `smoke_model`）。REPORT-01 已如实声明，属缺项不算隐瞒。

**1.8** `Resources.tsx:302` — 面板上半标题“检测到的 agent”实际列出 store 里全部 agent（含未检测、手动、cc-switch）。小偏差。

其余核对：resources.json 结构与 12 个模型字段名与任务书逐字一致（`models.rs:138-175`）；合并规则其余三条（新条目待冒烟、消失不删标“未在事实源中”、每次记 source+时间）实现正确；冒烟判据表五通道逐条对上（pi 真身核对、thinking 不算答案均有测试）；kimi 真身“无”、claude/codex 官方端点不核对，均符合。

## 2. 逻辑正确性

**2.1** `lib.rs:55-64, 73-82, 143-152, 161-170, 413-420` — 所有资源命令都是“read_lock 克隆 → 阻塞操作 → 存盘 → write_lock 覆盖”，无版本 CAS、无串行化。「冒烟所选」可阻塞数分钟，期间用户做的手动输入/移除/单条冒烟会在它结束时被旧克隆整体覆盖，内存与 `resources.json` 同时丢失。这是本期最重的问题。建议：store 加单调版本号做 compare-and-swap 重放，或用 Mutex 把“读-改-写”整体串行。

**2.2** `resources.rs:874-893`（smoke_selected）— keys 先按 `agent_ids` 过滤，而 cc-switch 模型挂在 `"cc-switch"` agent 下；用户只勾 provider 不勾 cc-switch agent 时 keys 为空，静默什么都不做、不报错。`Resources.tsx:304` 又允许这种点击。建议 `cc_switch_providers` 非空时把 `"cc-switch"` 并入 agent 过滤集合。

**2.3** `resources.rs:516-522`（parse_pi_sources）— 可用性判定用子串 `contains`：model id 是另一 id 前缀时（`glm-4` vs 行里的 `glm-4.7`、`deepseek-chat` vs `deepseek-chat-v3`）会把 models-store 里未鉴权的模型误判为 `--list-models` 可用。建议按空白切列后整词比较，或匹配 `provider/id` token。

**2.4** `resources.rs:1440-1448`（run_process）— 子进程秒退（鉴权失败等）时 `stdin.write_all` 遇 broken pipe 直接返回 Err“写入子进程 stdin 失败”，stderr 里的真实报错被丢弃，冒烟失败摘要指向错误方向。建议 broken pipe 视为非致命，继续收输出再判定。

**2.5** `resources.rs:1361-1368, 1538-1544` — “60 秒超时”硬编码；20 秒的 `list`/`auth check` 超时也报 60 秒。建议把 timeout 值放进 ProcessResult。

**2.6** `resources.rs:1495-1504`（finish_bounded_reader）— 2 秒 recv 超时即返回空并丢弃 JoinHandle：每次超时泄漏一个读线程，且可能丢输出。轻微。

**2.7** `resources.rs:560-585, 1205-1262, 1518-1523` — python 子进程未设 `PYTHONUTF8=1`/`PYTHONIOENCODING`。中文 Windows 上管道输出默认 GBK，`cc_verification_passed` 匹配的“结论：全部一致…”经 `from_utf8_lossy` 后必然失配 → verify 通过也会判冒烟失败。list 的 JSON 是 ASCII 无碍，但 verify 的中文结论行是真实风险。

**2.8** `resources.rs:349-436`（parse_kimi_config）— 手写 TOML 行解析不认行内注释：`model = "k3" # x` 使 `strip_suffix('"')` 失败返回 None，真实 ID 静默丢失；多行数组解析不到 effort；`capabilities` 前缀匹配会误中 `capabilities_x` 或注释里含 "thinking" 的行。建议值解析先剥 `#` 注释，或直接用 toml crate。

**2.9** `resources.rs:263-310` — kimi/pi 空结果都 Err，codex `{"models":[]}` 却返回 Ok(空)，merge 会把该 agent 全部条目批量标“未在事实源中”；缓存瞬时为空即误报。建议空清单同样报错或跳过 missing 标记。

**2.10** `resources.rs:196-200` — claude CLI 未检测到时预设表照样合并，未装 claude 也会出现三条待冒烟条目。小问题。

**2.11** `lib.rs:113-134`（add_manual_agent）— 非原子：`add_tool_override` 成功后 `save_resources` 失败，则 tools.json 已落盘、两处内存未更新，不一致持续到重启。建议统一提交或失败回滚。

Windows 路径/编码其余方面：argv 数组全程无 shell、`from_utf8_lossy`、`find_ai_cross_reference` 的 cfg(windows) 守卫，均正确。

## 3. 边界

- 事实源缺失/为空：codex/kimi/pi 缺文件与空输出都有带路径的中文错误（`resources.rs:239-245, 300-306, 438-457`）✓；codex 空数组除外（见 2.9）。
- `--list-models` 格式漂移：失配走向 Err（安全侧失败）✓；反向风险是 2.3 的前缀误报。
- `cc_switch.py` 找不到：`find_ai_cross_reference` 返回 None → “未配置 ai-cross 目录” ✓（`resources.rs:1265-1289`）；python 缺失 → “未检测到 Python” ✓。
- 冒烟超时：60s + `taskkill /T /F` + kill/wait，无僵尸 ✓；残余问题是 2.5 文案与 2.6 线程。
- 模型 ID 特殊字符：全程单 argv 元素，安全 ✓。但 `Resources.tsx:256-258` 手动 agent 启动参数按空白切分，含空格的参数（Windows 路径）无法输入，建议支持引号或逐行。
- `resources.rs:1008-1012` — 手动添加的 pi 模型 provider 为空时会直接传 `--provider ""`，pi 报错信息不友好；建议校验 pi 模型必须带 provider。

## 4. 安全

- 未读 `~/.pi/auth.json`（全仓 grep 无引用）✓；pi 只反序列化 id/provider/reasoning/contextWindow ✓；8 个 fixture 无 key/token/baseUrl ✓。
- 无 shell 拼接：`Command::new` + args 数组，包括 taskkill/where ✓。
- **`resources.rs:1557-1570`（safe_summary）覆盖不足**：只擦 aicross 自身进程环境变量中名字含 KEY/TOKEN/SECRET/PASSWORD/AUTH 的值。启动台传给终端子进程的环境变量不在 `std::env::vars()` 里，擦不到；CLI 打印的带凭据 URL（`?key=…`、`user:pass@host`）与 `sk-` 类 token 也不会被擦，这些会以 200 字摘要落盘进 `resources.json` 的 note。建议补 URL 凭据与 token 前缀的正则擦除。
- `resources.rs:1386-1407`（value_reports_ready）过宽：任意嵌套出现 `ok/valid/configured=true` 即判就绪，可能把未就绪误判就绪（探测面，非凭据面）。建议只认顶层 `ready`。
- source 字段落盘含主目录绝对路径——本地文件可接受，注意别复制进日志/报告。

## 5. 代码质量

- **重复**：`resources.rs:1410-1504` 的 run_process/bounded_reader 与 `tools.rs:78-148` 的 version_for/pipe_reader/finish_reader 是两套几乎相同的“子进程+超时+截断读取”；`resources.rs:1525` terminate_process_tree 与 `pty.rs` terminate_child 重复。建议抽一个子进程工具模块，顺带统一 2.5 的超时文案。
- `resources.rs:886-894` — smoke_selected 对每个 model `smoke_one(store.clone(), …)` 整库克隆，O(n²) 拷贝。改为借用或只回写单条结果。
- `resources.rs:908-912` — 冒烟结束后重跑一次 `detect_cc_switch` 只为刷新 provider 列表；失败时 providers 被清空，面板误显“未检测到 provider”。建议复用结果或失败时保留旧值。
- `Resources.tsx:12-18` agentNames 与 tools.json 名称双份维护；cc-switch 的显示名可挪到后端。
- `models.rs:201-224` — AddResourceModelRequest 与 UpdateResourceModelRequest 字段完全相同，可合一（前端 types.ts 已合一）。
- 文案：整体符合 AGENTS.md（按钮 2–6 字动词短语、无 emoji、无内部词、错误中文前缀+原文）。小点：`provider（可空）` 中英混排括号不一致。

## 6. 测试

已覆盖：四事实源 fixture 解析、合并三规则、pi 冒烟三态（成功 / 身份不符 / thinking-only）、claude/codex/kimi 判据、cc 结论行、配置原子替换、cancelled TTL、live codex/kimi（ignored）。

最该补的 3 个：
1. **run_process 行为测试**：超时杀树不留残留进程、stdin broken pipe 时仍返回子进程输出（会抓到 2.4）、1MB 截断生效。用 sleep/cat 即可跨平台。
2. **parse_pi_sources 可用性匹配**：`glm-4` 不得因 `glm-4.7` 行误判可用（抓 2.3）；`pi_auth_ready` 对 `{"ready":false}` 与无关嵌套 true 的判定（抓 4.5）。
3. **合并元数据刷新**：二次探测 note 变化时用户备注保留、元数据更新（会暴露 1.1 的设计缺口）；附带 kimi 带行内注释的 TOML fixture（抓 2.8）。

---

## 对照 TASK-01 原始需求是否完整实现？

**一句话：主体（A 探测、B 手动、C 冒烟、D 界面、E 遗留、验收 1/2）已实现且报告诚实，但验收 3 的本机 pi 两项未完成，合并规则“更新元数据”与 auth-check 语义有实质偏差，并发写覆盖是必须修的数据丢失缺陷。**

缺项列表：
- pi `--list-models` 本机验证（未验证，沙箱 EPERM）
- `zai-coding-cn/glm-4.7` 冒烟成功并记录 `smoke_model`（未做）
- 合并规则：同键条目的事实源 note 元数据不刷新（1.1）
- pi auth check 硬门槛取代“就绪状态”语义（1.2），及私加 `--no-refresh`（1.3）
- 界面“状态标签与动作成对”（1.4）
- Tauri 窗口内人工点检资源 tab / 面板 / 手动输入 / 移除（未做，已声明）
- 修复项：并发探测/冒烟对 resources.json 的丢失更新（2.1）、provider-only 冒烟所选无效（2.2）、pi 可用性前缀误报（2.3）、python 中文输出编码（2.7）