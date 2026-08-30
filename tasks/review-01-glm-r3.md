现在对照本机 pi 0.84.2 源码验证 `auth check --json` 的真实输出形状：
# 定向复核结论

## 一、review-01 三、四节缺项 vs 修正轮 R2 声明 vs 当前代码

| 缺项 | 判定 | 依据（一句） |
|---|---|---|
| 申报/失败条目无移除入口（三-1 / 四-1） | 已核实修复 | `Resources.tsx:204-210` ModelRow 对所有状态无条件渲染“移除”按钮并调 `remove`，后端 `resources.rs:780-801` 任意条目可删。 |
| run_process stdin 写入不受超时约束（三-2 / 四） | 已核实修复 | `process.rs:63-72` stdin 写入移入独立 `thread::spawn`，主等待循环 `process.rs:75-90` 独立计时杀进程，测试 `process.rs:186-205`（8 MiB 阻塞写 + 100 ms 超时，断言 <3 s 返回）通过。 |
| 冒烟失败摘要按中文分号切分截断（三-3） | 已核实修复 | 新增 `smoke_failure` 字段 `models.rs:174-176`（serde default 兼容旧 JSON），失败整段写入/成功清空 `resources.rs:838-840`、合并完整保留 `resources.rs:665`，前端独立展示 `Resources.tsx:215`，测试 `resources.rs:1607-1609` 断言含“；”摘要原样保留。 |
| pi 串行逐 provider auth check 无进度（三-4） | 未修 | `resources.rs:449-466` 仍逐 provider 串行各 20 s，前端只有“探测中…”（非阻塞观感项，R2 也未声明修）。 |
| `String(error)` 带 `Error: ` 前缀（三-5） | 已核实修复 | `Resources.tsx:39-41` `errorText` 仅取 `Error.message`，全部 catch 点已替换。 |
| tools.rs resolve_binary_name 无超时（三-6） | 未修 | `tools.rs:38-51` 仍是裸 `Command::output()`（轻微，R2 未声明修）。 |
| pi auth check 顶层输出形状无验证（四-2） | 已核实修复，**但 fixture 半真** | 本机 `D:\pi-0.84.2\pi.exe` 内嵌源码 445757-445771 行确认 auth check 返回顶层 `{status:"ready"/"not_ready"/"invalid", provider, ...}`；`resources.rs:1411-1424` 只取顶层 `status`、`resources.rs:474` 只认 `"ready"`；`pi_auth_check_ready.json` 与 pi.exe:445769 逐字一致，测试 `resources.rs:1669-1681` 两个 fixture 均被使用且断言旧 `{"ready":true}` → None。 |
| `api-key=` 类查询参数脱敏漏匹配（四-3） | 已核实修复 | `resources.rs:1472-1476` 正则 `api[-_]?key|access_token|secret` 覆盖 api-key/api_key/apikey，测试 `resources.rs:1833-1843` 断言 one..six 全隐藏。 |
| agentNames 双份维护（二-5.4 / 四） | 已核实修复 | `models.rs:157-159` `ResourceAgent.name` 后端单源，`ensure_agents` `resources.rs:64-115` 从 tools.json 同步并强制 cc-switch 名；src-ui 全仓 grep 无 `agentNames`，`Resources.tsx:178/250/288` 均用 `agent.name || agent.id`。 |

## 二、特别核对

- **就绪判定只认顶层 status=="ready"**：成立——`pi_auth_status`（resources.rs:1411-1424）仅对每个顶层 JSON 对象取直接子键 `status`（非字符串也按原值转写 meta），唯一就绪判定点 `resources.rs:474` `== Some("ready")`，与 pi.exe 实际输出形状吻合。
- **fixture 与测试对应**：`pi_auth_check_ready.json` / `pi_auth_check_not_ready.json` 均被 `pi_source_matching_uses_exact_tokens_and_auth_uses_top_level_status`（resources.rs:1669-1681）真实读入并断言，非摆设。

## 三、本轮新引入/残留问题

1. `pi_auth_check_not_ready.json` 的 `"missing"` 不是 pi 真实取值（pi 实际发 `"not_ready"`/`"invalid"`，见 pi.exe:445757-445771），R2 声称两个 fixture 均为“真机原文”不实；判定逻辑不受影响（非 ready 一律未就绪），属 fixture 保真度瑕疵。
2. `process.rs:97-100` 超时后 `join()` stdin 写线程：若被杀子进程留下脱离进程组的孙进程持有 stdin 读端，join 理论上仍可无限阻塞；`taskkill /T`（process.rs:149-157）覆盖常规场景，当前调用方不可触发，属残余风险非缺陷。
3. `ensure_agents`（resources.rs:99-104）每次探测都用 tools.json 定义名覆写 `agent.name`——无重命名功能时无实际影响。

## 四、结论

代码层面可验收；三-4、三-6 两项观感级未修和上述第 1、2 条瑕疵不构成阻塞。