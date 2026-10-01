本代码审查针对 **TASK-39「资源与配置的命令行」**。审查全程只读，依据提供的任务书、实现者报告、产品说明、git diff 及代码全文，对照核实所有结论。

---

### 一、任务书第 1–4 节逐条核对表

| 章节与要求项 | 结论 | 证据（文件:行号） | 实现者报告核实与说明 |
|---|---|---|---|
| **1.1 命令通用规范**：JSON 写 stdout、一行说明写 stderr；退出码 0/1/2/3 规范一致 | **做到** | [`src/console_cli.rs:220-224`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L220-L224), [`L450-452`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L450-L452), [`L572-574`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L572-L574), [`L668-675`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L668-L675) | 核实属实：stdout 输出规范 JSON，stderr 输出说明；退出码统一遵循 0 成功 / 1 业务失败 / 2 参数错 / 3 引擎不可用。 |
| **1.2 `aicross resources`**：读库不探测、不发请求；输出 agent（`id`, `name`, `version`, `state`, `note`）与 model（`id`, `agent`, `provider`, `resolved_id`, `state`, `note`） | **做到** | [`src/console_cli.rs:214-224`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L214-L224), [`L695-719`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L695-L719), [`src/resources.rs:2372`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2372) | 核实属实：复用 [`get_resources_at`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2372) 经由数据迁移与重判，不探测，字段全量映射。 |
| **1.3 状态映射**：agent 可用/未安装；模型 原生可用/未验证/已验证/验证失败/未登录/额度用尽，note 下一步说明 | **部分** | [`src/resources.rs:2309-2354`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2309-L2354) | **存在优先级缺陷**：[`model_state`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2328) 先行判断了 [`is_native_model`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L417)，导致原生模型即便处于未过期的额度耗尽期，依然返回「可用」（见问题 2）。报告声称的优先级顺序虽符合其代码实现，但不符合业务逻辑。 |
| **1.4 `aicross resources detect`**：重新探测全部 agent（复用接入页函数，含 pi 登录检查），写回资源库并输出，ccSwitchPath 配置取自派发设置 | **做到** | [`src/console_cli.rs:225-248`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L225-L248), [`src/resources.rs:2421`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2421) | 核实属实：直接调用 [`detect_all_resources_at`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2421)，且透传了派发设置里的 `cc_switch_path`。 |
| **1.5 `aicross resources smoke <agent> [--model <id>] [--provider <p>]`**：第三方模型冒烟；指定 model 单冒烟；原生不冒烟并报「原生模型无需冒烟」退 0；输出该 agent 条目 | **部分** | [`src/console_cli.rs:257-413`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L257-L413) | **存在严重参数解析漏洞**：当用户传入尾部缺少参数值的 `--model` 时，解析器未报错退出 2，反而静默降级为对该 agent 的全量第三方模型批量冒烟（见问题 1）；此外 agent 无模型时误报「原生模型无需冒烟」（见问题 4）。正常传参逻辑已复用既有函数。 |
| **1.6 `aicross config show`**：输出 `{ roles, dispatch }`，roles 取自 roles.json，dispatch 取自 load_settings 口径（含默认值与夹取） | **做到** | [`src/console_cli.rs:445-453`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L445-L453) | 核实属实：[`roles_from`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/orchestration.rs#L609) 读 roles，[`load_settings`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L448) 读 dispatch 并完整涵盖夹取与默认值。 |
| **1.7 `aicross config set-role`**：角色校验；目标在资源库中匹配校验（大小写不敏感匹配、规范 id 回写、多 provider 歧义退出 2 并列出候选项）；省略 tier/thinking 保持原值；写 roles.json 输出新 roles | **部分** | [`src/console_cli.rs:466-575`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L466-L575) | 目标校验、规范回写、保持原值等均做到。但**存在损坏文件静默覆盖风险**：若 `roles.json` 内容损坏，读取返回默认空值，写盘将静默抹掉原有配置而非报错退出 1（见问题 3）。尾部缺值的 `--tier`/`--thinking` 会被吞掉（见问题 1）。 |
| **1.8 `aicross config set <key> <value>`**：13 个键白名单；数值/布尔 JSON 写入；越界按 load_settings 夹取并在 stderr 说明；未知键退出 2；保留其余键；输出新 dispatch | **做到** | [`src/console_cli.rs:426-436`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L426-L436), [`L596-676`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L596-L676) | 核实属实：白名单严格限制 13 键；复用 [`settings_from_value`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L586) 保证夹取区间与取整与引擎一致；保留未改动键；损坏文件直接报解析失败退出 1。 |
| **1.9 子命令接入**：加进 `src/console_cli.rs` 分发，不走窗口 HTTP | **做到** | [`src/console_cli.rs:19-22`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L19-L22), [`L77-81`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L77-L81) | 核实属实：分发直接接入并在前置判断中 return，直接操作本地文件，完全不触发 HTTP Client。 |
| **2.1 存储约束**：不改存储文件格式；窗口接入页与设置页照常可用 | **做到** | [`src/resources.rs:657`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L657), [`src/orchestration.rs:613`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/orchestration.rs#L613), [`src/console_cli.rs:662`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L662) | 核实属实：数据结构未作任何修改；沿用原版 schema。 |
| **2.2 代码复用约束**：探测与冒烟复用现有函数，不复制逻辑；Tauri 命令保持原行为 | **做到** | [`src/resources.rs:2421`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2421), [`L2522`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2522), [`L2532-2553`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2532-L2553) | 核实属实：Tauri 命令调用的底层函数暴露为 `pub(crate)`，[`smoke_agent_models_at`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2532) 通过抽取的 `filter` 参数委托给 [`smoke_agent_models_filtered_at`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2558)，未复制核心逻辑。 |
| **2.3 安全约束**：不读凭据、不输出 token 或密钥 | **做到** | [`src/resources.rs:2295`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2295), [`src/console_cli.rs:695-719`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L695-L719) | 核实属实：输出 JSON 只包含公开元数据与状态；错误输出经过脱敏；不涉及凭据文件读取。 |
| **3.1 状态映射测试**：纯函数测试（agent 2 态、模型 6 态包含未过期/过期） | **做到** | [`src/resources.rs:4570-4632`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L4570-L4632) | 核实属实：覆盖全部状态及时间边界，端点切换判断有单测钉住。 |
| **3.2 命令层测试**：参数解析、目标不存在列候选、set 越界夹取、写入读回一致，临时目录隔离，不发真请求 | **做到** | [`src/console_cli.rs:1189-1544`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L1189-L1544) | 核实属实：通过 [`ResourcesOps`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L184) 注入 mock 函数，单测无真实网络/进程探测。 |
| **3.3 退出码 3 测试**：各来源均有测试覆盖 | **做到** | [`src/console_cli.rs:1546-1605`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L1546-L1605) | 核实属实：分别测试了根目录不可用和 `resources.json` 损坏两种退出码 3 来源。 |
| **4.1 文档补充**：`docs/CONSOLE-CHANNEL.md` 补命令、字段、状态取值与退出码说明 | **做到** | [`docs/CONSOLE-CHANNEL.md:135`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/docs/CONSOLE-CHANNEL.md#L135), [`L153-195`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/docs/CONSOLE-CHANNEL.md#L153-L195), [`L233`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/docs/CONSOLE-CHANNEL.md#L233) | 核实属实：文档补充了「资源与配置」小节，详细说明命令、字段、状态及退出码 3 的来源扩充。 |

---

### 二、专项核对结论（第 2–6 项）

- **第 2 项（状态映射）**：
  - **原生判定**：正确。[`is_native_model`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L417) 判定 `is_native_agent && !third_party_endpoint`，当 claude 挂接 `ANTHROPIC_BASE_URL` 时会正确判定为第三方并触发冒烟，单测 [`task39_smoke_agent_third_party_respects_endpoint_kind`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L4636) 明确核验了此行为。
  - **额度过期**：正确。通过解析 RFC3339 时间与传入的 `now_millis` 比对，过期标记正确降级至常规验证状态。
  - **优先级冲突**：**不合理**。当前优先级为「原生 → 额度用尽 → 未登录 → 验证结果」，导致原生模型在配额耗尽时仍显示为「可用」（详见问题 2）。
  - **note 可执行性**：agent 未安装的 note 为 `安装后运行 aicross resources detect`，可直接执行；pi 未登录 note 为 `在 pi 里执行 /login <provider>`，属于自然语言指导，无法在外部 shell 中作为单行命令执行（详见问题 5）。
- **第 3 项（函数复用与 Tauri 命令影响）**：
  - 探测与冒烟均直接复用了 `src/resources.rs` 内既有的核心业务逻辑函数（[`detect_all_resources_at`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2421)、[`smoke_resource_model_at`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2522)）。
  - 批量冒烟抽取的 [`smoke_agent_models_filtered_at`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2558) 保留了原函数签名，Tauri 命令传入 `|_| true` 保持原行为完全不变，未复制逻辑。
- **第 4 项（`config set-role` 与 `config set` 校验与写盘）**：
  - `set-role`：agent 大小写不敏感并回写规范 id，同名模型跨 provider 未指定时退出 2，校验失败不写盘；但遇损坏文件时存在覆盖风险（详见问题 3）。
  - `set`：13 个键白名单完全一致；类型校验严格，有限浮点数校验完备；夹取口径直接调用 [`settings_from_value`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L586)，与引擎一致；保留了文件里的其余未知键；遇损坏文件时报解析失败退出 1，不写盘，经 [`write_json_atomic`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L662) 原子写入。
- **第 5 项（安全）**：
  - **未发现问题**。全流程未读取任何 token、未向 stdout/stderr 输出敏感凭据；冒烟失败信息经过 [`safe_summary`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2295) 脱敏；pi 探测仅检查退出状态，符合密钥安全规范。
- **第 6 项（退出码与参数解析干扰）**：
  - 退出码与任务书定义一致。
  - 新增子命令在 `console_cli::run_at_with` 的最前列通过 match 独立处理并 return，未修改原有 [`build_request`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L781) 等已有子命令的处理流程，互不干扰；面向用户的子命令正确配置了控制台挂接。

---

### 三、问题清单（按严重程度排列）

#### 【高】问题 1：选项缺值检查缺失，导致缺少 `--model` 值的单模型冒烟静默演变为全量批量冒烟

- **问题**：在命令行末尾传入缺值的带值选项（如 `aicross resources smoke pi --model`）时，系统不仅不报参数错误（退出码 2），反而直接视作未传 `--model`，触发全量第三方模型的批量冒烟。
- **证据**：
  1. [`src/console_cli.rs:751-762`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L751-L762)：
     ```rust
     fn unknown_option_error(args: &[String], from: usize, allowed: &[&str]) -> Option<String> {
         let mut index = from;
         while index < args.len() {
             let arg = args[index].as_str();
             if arg.starts_with('-') && allowed.contains(&arg) {
                 index += 2; // 若 arg 位于 args 末尾（如 args.len()=5, index=4），此处 index 直接变为 6
             } else {
                 return Some(format!("不认识的参数：{arg}"));
             }
         }
         None // 循环因 index >= args.len() 结束，直接返回 None，未检测选项缺值！
     }
     ```
  2. [`src/console_cli.rs:294-412`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L294-L412)：
     ```rust
     match option_value(args, "--model") {
         Some(model_id) => { /* 单模型冒烟 */ }
         None => {
             // option_value 找不到值返回 None，直接落入批量冒烟逻辑！
             let operation = match (ops.smoke_agent_third_party)(&store_file(root), &agent_id) { ... };
         }
     }
     ```
- **后果**：用户若在终端误输入 `aicross resources smoke pi --model`（漏写模型名），预期应为参数错误（退出码 2）。但实际代码会静默向该 agent 下**所有**第三方模型发起真实网络请求，消耗用户的 API 额度与费用，违背最小惊奇原则。同理，`config set-role` 末尾漏写 `--tier` 或 `--thinking` 的值时也会被静默吞没并保留旧值。
- **建议改法**：
  在 [`unknown_option_error`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L751) 中严格检查选项后继参数：
  ```rust
  if arg.starts_with('-') && allowed.contains(&arg) {
      if index + 1 >= args.len() || args[index + 1].starts_with('-') {
          return Some(format!("缺少 {arg} 的值"));
      }
      index += 2;
  }
  ```

---

#### 【中】问题 2：模型状态判定中「原生可用」优先级过高，导致额度耗尽的原生模型仍被显示为「可用」

- **问题**：[`model_state`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2328) 先行判断了 [`is_native_model`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L417)，短路了后续对 `quota_exhausted_until` 的有效性检查。
- **证据**：
  [`src/resources.rs:2328-2345`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2328-L2345)：
  ```rust
  pub(crate) fn model_state(model: &ResourceModel, now_millis: i64) -> (&'static str, String) {
      if is_native_model(&model.agent) {
          return (RESOURCE_STATE_AVAILABLE, String::new()); // 原生模型直接返回「可用」，不再走额度判断
      }
      let quota_active = model
          .quota_exhausted_until
          .as_deref()
          ...
  ```
- **后果**：当官方原生模型（如 Claude Code 或 Codex）在运行中触发 429 或 session limit、并在资源库中记录了未过期的 `quota_exhausted_until` 时，`aicross resources` 仍将其展示为「可用」且 `note` 为空。宿主 skill 或用户会误认为该原生模型当前可用并继续发起派发，导致不必要的重试或即时报错。
- **建议改法**：
  将未过期的额度检查提升至原生模型检查之前：
  ```rust
  pub(crate) fn model_state(model: &ResourceModel, now_millis: i64) -> (&'static str, String) {
      let quota_active = model
          .quota_exhausted_until
          .as_deref()
          .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
          .is_some_and(|moment| moment.timestamp_millis() > now_millis);
      if quota_active {
          return (
              RESOURCE_STATE_QUOTA,
              format!(
                  "额度恢复时间：{}",
                  model.quota_exhausted_until.as_deref().unwrap_or_default()
              ),
          );
      }
      if is_native_model(&model.agent) {
          return (RESOURCE_STATE_AVAILABLE, String::new());
      }
      ...
  ```

---

#### 【中】问题 3：`config set-role` 遇损坏的 `roles.json` 会静默以默认值覆盖，造成原有角色配置丢失

- **问题**：`config set-role` 依赖 [`orchestration::roles_from`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/orchestration.rs#L609) 读取配置，而该函数在反序列化失败时使用 `unwrap_or_default()` 返回默认配置；随后写入磁盘时，会把原本损坏但可能包含用户自定义角色的文件完全覆盖为默认配置。
- **证据**：
  1. [`src/console_cli.rs:552`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L552)：
     ```rust
     let mut roles = crate::orchestration::roles_from(root);
     ```
  2. [`src/orchestration.rs:609-611`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/orchestration.rs#L609-L611)：
     ```rust
     pub(crate) fn roles_from(root: &Path) -> RoleSettings {
         fs::read(root.join("roles.json"))
             .ok()
             .and_then(|bytes| serde_json::from_slice::<RoleSettings>(&bytes).ok())
             .unwrap_or_default()
     }
     ```
  3. [`src/console_cli.rs:568`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L568)：直接调用 [`save_roles_in`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/orchestration.rs#L613) 写盘覆盖。
- **后果**：相比 `config set` 在 `dispatch-settings.json` 损坏时主动报错退出 1（[`src/console_cli.rs:651`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L651)），`config set-role` 遇 `roles.json` 损坏时不仅不报错，反而把损坏文件整体覆写（丢失其他两个角色的既有配置），与 REPORT-39 所称「设置文件损坏退出 1」不符。
- **建议改法**：
  在 [`run_config_set_role`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L466) 中增加文件完整性校验：若 `roles.json` 存在且读取后反序列化失败，向 stderr 输出 `roles.json 解析失败` 并返回退出码 1，拒绝写盘。

---

#### 【低】问题 4：`smoke <agent>` 遇到模型列表为空的 agent 时误报「原生模型无需冒烟」

- **问题**：在未指定 `--model` 执行批量冒烟时，代码仅根据 `third_party == 0` 进行判定，未能区分「全部为原生模型」与「完全没有模型（例如未探测/未检测到模型的 agent）」。
- **证据**：
  [`src/console_cli.rs:390-406`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/console_cli.rs#L390-L406)：
  ```rust
  let third_party = fresh
      .agents
      .iter()
      .find(|agent| agent.id == agent_id)
      .map(|agent| {
          agent.models.iter().filter(|model| !resources::is_native_model(&model.agent)).count()
      })
      .unwrap_or(0);
  if third_party == 0 {
      let _ = writeln!(stderr, "原生模型无需冒烟");
      return 0;
  }
  ```
- **后果**：对于尚未安装、模型列表为空（`models: []`）的 agent，运行 `aicross resources smoke kimi` 会输出 `原生模型无需冒烟` 并退出 0，给出与实际状态（未安装/无模型）完全相悖的误导性提示。
- **建议改法**：
  检查该 agent 下总模型数是否为 0：
  ```rust
  let total_models = fresh.agents.iter().find(|a| a.id == agent_id).map(|a| a.models.len()).unwrap_or(0);
  if total_models == 0 {
      let _ = writeln!(stderr, "该 agent 下无模型");
      return 0;
  }
  if third_party == 0 {
      let _ = writeln!(stderr, "原生模型无需冒烟");
      return 0;
  }
  ```

---

#### 【低】问题 5：`pi` 未登录提示给出的内容是自然语言指引而非可执行命令行

- **问题**：`pi` 未登录时的 `note` 为 `在 pi 里执行 /login <provider>`，若外部宿主自动化脚本将其直接作为 shell 命令提取执行会失败。
- **证据**：
  [`src/resources.rs:2357-2364`](file:///C:/Users/keros68/AppData/Local/Temp/claude/D--work-agentteam/b59708ad-1bec-4b1e-b055-182e42f62367/scratchpad/agy39-empty/src/resources.rs#L2357-L2364)：
  ```rust
  fn pi_login_note(provider: &str) -> String {
      let provider = provider.trim();
      if provider.is_empty() {
          "在 pi 里执行 /login".to_string()
      } else {
          format!("在 pi 里执行 /login {provider}")
      }
  }
  ```
- **后果**：任务书第 1 节要求「`note` 给登录命令（`pi auth login --provider <p>`，按 `pi --help` 实际为准）」。实现者核实了 pi 0.86 没有 CLI 登录子命令而采用该文案，但该字符串在外部 shell 环境下不是合法命令。
- **建议改法**：若预期该字段供人类阅读，当前文案可保留；若供自动化调度，建议在文档中明确标注该说明属于人工操作指引。

---

### 四、审查结论总结

TASK-39 的总体架构设计清晰，子命令接入、跨平台终端挂接、Tauri 函数同步部分抽取复用、安全脱敏以及测试驱动覆盖均做得扎实。主要问题集中在 **`unknown_option_error` 边缘缺值未检导致 `--model` 穿透为批量全量冒烟（高）**，以及 **状态机原生检查过早短路额度判断（中）** 和 **`roles.json` 损坏未保护（中）**。建议修正上述问题后合入。
