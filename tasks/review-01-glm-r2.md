# 第二轮复审结论

## 一、「已修」逐条核实（对照 R1 处理声明）

**需求/数据正确性**

| 项 | 判定 | 依据 |
|---|---|---|
| 1.1 meta/note 分离 | 真修 | `models.rs:174-176` 新增 `meta`（serde default 兼容旧 JSON）；`resources.rs:666-684` 合并时刷新 meta、保留 note/tier/status/smoked_at/smoke_model/vendor；测试 `resources.rs:1552-1614` 覆盖二次探测刷新+旧数据兼容 |
| 1.2 auth 非硬门槛 | 真修 | `resources.rs:449-474` 逐 provider 循环，未就绪只标 meta「鉴权未就绪」+ 进 errors，整体仍返回 Ok |
| 1.3 去 `--no-refresh` | 真修 | `resources.rs:454-459` 参数仅 `auth check --provider … --json`，全仓 grep 无 no-refresh |
| 1.4 状态-动作成对 | 真修，**但引入新缺口** | `Resources.tsx:236-239` 已接入→移除+重新冒烟，其余→仅冒烟；但申报/冒烟失败条目从此**没有任何移除入口**（后端 `resources.rs:792-811` 支持任意删除，纯 UI 挡死；输错的手动模型只能改 resources.json） |
| 1.5 claude 别名标注 | 真修 | `resources.rs:539` meta="CLI 别名" |
| 1.6 codex fetched_at | 真修 | `resources.rs:29-33` 读取顶层字段，`:292` 写入「缓存抓取时间」 |
| 1.8 面板只列已检测 | 真修 | `Resources.tsx:342` `filter(agent => agent.detected && agent.id !== "cc-switch")` |

**逻辑正确性**

| 项 | 判定 | 依据 |
|---|---|---|
| 2.1 并发覆盖 | 真修 | `lib.rs:29` 全局 `resource_operations` Mutex；`lib.rs:412-433` 加锁→读最新→处理→存盘→回写，8 个变更命令全部走此路径（58/72/84/95/106/118-143/145/159），无绕过 |
| 2.2 provider-only | 真修 | `resources.rs:913-916` providers 非空时并入 cc-switch agent；`:873-876` 空选择报「所选资源没有可冒烟模型」；测试 `:1703` |
| 2.3 前缀误报 | 真修 | `resources.rs:496-505` 整词 token 或相邻列匹配；测试 `:1630-1636` glm-4 不因 glm-4.7 误判 |
| 2.4 broken pipe | 真修 | `process.rs:61-68` BrokenPipe 忽略、其余错误在收完输出后才返回；测试 `:186-200` |
| 2.5 超时文案 | 真修 | `process.rs:19` ProcessResult.timeout 记真实值；`resources.rs:1440-1446` 按实际时长生成文案 |
| 2.6 读线程泄漏 | 真修 | `process.rs:118-136` read_to_end+drain+recv+join，无超时丢弃 |
| 2.7 python 编码 | 真修 | `process.rs:32` 统一注入 `PYTHONUTF8=1` |
| 2.8 kimi TOML | 真修 | `resources.rs:333-336` 改用 toml crate（Cargo.toml 已加 `toml = "0.9"`）；fixture 含行内注释+多行数组，测试 `:1596-1613` |
| 2.9 codex 空清单 | 真修 | `resources.rs:273-275` 空 models 直接 Err，不进缺失标记合并；测试 `:1541` |
| 2.10 claude 未装 | 真修 | `resources.rs:198-205` 未检出即 return，不合并预设 |
| 2.11 手动 agent 原子性 | 真修 | `lib.rs:116-143` 同锁内先写 resources.json 再写 tools.json；`config.rs:121-129` 第二步失败恢复旧 resources.json；内存仅双写成功后更新（lib.rs:140-141）。残余：两写之间进程崩溃仍留磁盘不一致+重启后孤儿 agent（ensure_agents 只加不删，`resources.rs:71-105`） |

**安全/边界/质量**

| 项 | 判定 | 依据 |
|---|---|---|
| 4.3 脱敏 | 基本真修，有漏 | `resources.rs:1448-1485` 四类正则+测试；但 `[?&](?:key\|token)=`（`:1468-1471`）不匹配 `api-key=`/`api_key=`；env 擦除仍仅自身进程（`:1477-1485`）——冒烟子进程不继承启动台 env，暴露面实际已收窄，可接受 |
| 4.4 就绪判定 | 真修（按建议的安全侧） | `resources.rs:1390-1402` 仅认每个 JSON 值顶层 `ready:true`；测试覆盖嵌套 true 不算。**风险**：pi 0.84.2 文档/CHANGELOG 只确认 `pi auth check` 存在（0.84.1 新增），未见输出 schema，顶层 `ready` 假设无本机验证——失配会全员标「鉴权未就绪」（噪但安全侧） |
| 进程工具去重 | 真修 | 新模块 `process.rs`；`tools.rs:62-68` 用 `process::version_for`；`pty.rs:288-293` terminate_child 委托 `process::terminate_process_id` |
| O(n²) 克隆 | 真修 | `resources.rs:838-862` smoke_one_in_place 原位回写，smoke_selected 单份 store |
| provider 刷新失败 | 真修 | `resources.rs:895-901` 失败返回 None+错误；`Resources.tsx:324,334` null 时保留旧值 |
| 请求类型合一 | 真修 | `models.rs:196-204`、`types.ts:135-142` 各只剩一个 ResourceModelRequest |
| 参数引号 | 真修 | `Resources.tsx:46-87` 支持单/双引号+转义，未闭合抛错（`:77`） |
| pi 空 provider | 真修 | `resources.rs:743`（添加时拒绝）、`:1009`（冒烟前拒绝）、前端 `Resources.tsx:256` |
| 「可留空」 | 真修 | `Resources.tsx:268` |

## 二、「不采纳/未做」理由判断

- **1.7 未补做：成立**。本轮明确禁发模型请求，glm-4.7 冒烟不做是政策约束；`--list-models` 属本机沙箱 EPERM（非模型请求），报告把两者归并表述略粗但事实无误。
- **5.4 agentNames 双份维护**：R1 既没修也没声明，属静默遗漏（轻微，显示名兜底链 `agentNames ?? toolNames ?? id` 仍正确）。

## 三、本轮改动回归扫描

1. **申报/失败条目无移除入口**（上表 1.4）——本期唯一实质性新问题，一行条件即可修。
2. `process.rs:61-66`：stdin `write_all` 在超时循环之前执行，子进程挂起不读 stdin 则永不超时；当前所有 `Some(input)` 调用只传 ≤11 字节提示词，实际不可触发，属潜在缺陷。
3. `resources.rs:676-680`：冒烟失败段按 `；` 切分携带，摘要本身含中文分号时携带被截断、留碎片（meta 观感级）。
4. `resources.rs:449-466`：pi 探测串行逐 provider auth check（各 20s），provider 多时「探测」可挂数分钟且无进度提示。
5. `Resources.tsx:284`：`String(error)` 会带 `Error: ` 前缀，引号未闭合时报错文案观感差。
6. `tools.rs:37-51`：resolve_binary_name 仍是裸 `Command::output()` 无超时（where.exe 本地快，轻微）。
7. 锁序核对无死锁：gate 先于 RwLock，无反向路径；探测/冒烟期间用户操作排队符合声明，前端仅缺排队反馈。

## 四、结论

**代码层面可验收（人工窗口 smoke 与 pi 本机两项除外）**；仍缺项：申报/失败条目的 UI 移除入口（1.4 修法引入，应补）、pi auth check 顶层 `ready` 输出形状无真机/fixture 验证、`api-key=` 类查询参数脱敏漏匹配、run_process stdin 写入不受超时约束（潜在）、agentNames 双份维护未处理。