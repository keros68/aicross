# TASK-57 新用户走查修正（引擎）

来源：`tasks/UX-newuser-20261007.md`（空数据目录走查，14 条）。本期做其中引擎侧的部分；界面侧为 TASK-58，并行进行，契约以本文为准。

## 要做

### 1. 未探测状态（走查 #1）
`resources` 输出（`agent_entry`）：资源库 `detectedAt` 为空（从未探测）时，agent 的 `state` 为「未探测」，`note` 为「尚未探测」，其下模型的 `state` 同为「未探测」、`note` 为空。探测过后保持现有两态（可用 / 未安装）。常量放在 `resources.rs` 与现有状态常量一起。补测试。

### 2. 探测并行与进度（走查 #3）
- `detect_all_resources_at`：各 agent 的探测（版本、模型清单、pi 的登录探测）在 store 锁**外**并行跑（每个 agent 一个线程，`std::thread::scope` 即可），全部完成后在锁内一次性合并、盖 `detectedAt`、写盘。合并结果与现在逐个顺序跑的结果一致（同一份输入下 store 相同）。把 `detect_into` 拆成「锁外取结果」与「锁内应用」两步，pi 已有的 `prepare_pi_detection` / `apply_prepared_pi_detection` 是范例。
- 新增带回调的版本：`detect_all_resources_with_progress(path, cc_switch_path, on_done: &(dyn Fn(&str, usize, usize) + Sync))`，每个 agent 探测完调一次 `(agent_id, done, total)`；原函数调用它并传空回调。
- 窗口命令（`server.rs` 的探测命令）接 `tauri::AppHandle`，每次回调 `emit("resources-detect-progress", { agent, done, total })`。命令行输出不变。
- 测试：并行版与顺序合并的结果一致（可用注入的假探测函数，不真跑本机 CLI）；回调次数等于 agent 数、`done` 递增到 `total`。

### 3. `config show` 提速（走查 #9）
窗口每次读配置都调 `acp_setup::adapter_status()`（会拉起 where.exe，约 1 秒）。在窗口进程里缓存其结果 60 秒（`OnceLock<Mutex<Option<(Instant, Vec<AdapterReport>)>>>` 之类，放在 server.rs 或 acp_setup.rs）；`config set acpEnabled true` 装完适配器后清缓存。命令行一次性调用不需要缓存。另查窗口其它读配置路径里有没有每次都拉起子进程的调用，有则同样处理，并在报告里列出。

### 4. 擂台推荐分散（走查 #6）
推荐执行位时，同一阶段内已选的执行者参与排序：同一个模型不重复（软条件：没有别的候选时才允许重复），其次优先尚未在本阶段出现过的厂商，再按现有档位与计费排序。即执行位排序键改为 `(本阶段已选同一模型, 本阶段已有该厂商, 档位, 计费)`。只有一个执行位的阶段结果不变。规划角色同理处理。补测试：擂台 3 个执行位在候选池含三家时选出三家不同模型；候选池只有一个模型时三个位置都填它。

### 5. 推荐输出给出路（走查 #4、#5）
- `config recommend` 输出增加 `unverified`：`[{ "agent": "pi", "count": 33 }]`，列出各 agent 下可用但未验证、因此没进候选池的第三方模型数（不含未发现的、原生的），没有则为空数组。窗口 `config_recommend` 同结构。
- `unfilled_reason` 在有未验证模型时补一句「另有 N 个未验证模型，测试连接后可参与推荐」（N 为总数）。
- 补测试。

### 6. 评审位置的视角（走查 #7）
`config show` 的每个位置增加 `focus` 字段：评审（`builtin-jury`）按扁平序依次为「安全」「性能」「需求覆盖」，超出的为空串；其余模式为空串。取值与 `engine::preset_node_texts` 的视角顺序共用一张表，不要抄两份。补测试（含数量调到 1、2 时）。

### 7. skill 状态（走查 #11）
- `skill status` 输出的所有路径（以及 `slash list` 里的 `path`）在 Windows 上统一用反斜杠：拼路径一律逐段 `join`，输出时不混用 `/`。
- 每个位置增加 `outdated`：种类为外部（他人管理）且版本可解析、低于内置版本时为 true；我们装的仍用现有 `updateAvailable`。版本比较按点分数字逐段比，解析不了的视为不比较（false）。补测试。

### 8. 已存在的短命令文件（走查 #14）
`slash list` 的 `modes` 每项增加 `existing`：该模式还没有登记的命令，但 `<前缀><短名>` 对应的命令文件已存在时，给出 `{ "name": "ax-blind", "state": "已写入" | "被占用", "path": "..." }`，否则为 null。`slash add` 对带标记的已有文件照旧覆盖（登记）；无标记的照旧拒绝。补测试。

## 不改
- 推荐仍只用可用 / 已验证的模型；未验证的第三方不进候选池。
- 批量测试连接用已有的 `resources smoke --agent <id>`（及单条 `--model`），引擎不新增命令。

## 范围与验证
只改 `src/**`、`docs/CONSOLE-CHANNEL.md`、`tasks/REPORT-57.md`。不改 `src-ui/**`。不加依赖，不 commit，不 push，不联网。`src/server.rs`、`src/main.rs` 是 CRLF，其它 Rust 文件 LF。测试一律用临时目录，绝不写真实家目录或真实数据目录，不真跑本机 agent CLI。

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

完成后写 `tasks/REPORT-57.md`：逐条改法、输出字段变化、新增测试名、验证结果摘要。
