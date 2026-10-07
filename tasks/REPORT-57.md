# REPORT-57 — 新用户走查修正（引擎）

任务书：`tasks/TASK-57-newuser-engine.md`（来源 `tasks/UX-newuser-20261007.md` 的引擎侧条目）。
只改了 `src/**`、`docs/CONSOLE-CHANNEL.md`、本报告；未动 `src-ui/**`，未加依赖，未 commit / push / 联网。

## 逐条改法

### 1. 未探测状态（走查 #1）
- `src/resources.rs`：新增常量 `RESOURCE_STATE_NEVER_DETECTED`（「未探测」）与 `NEVER_DETECTED_NOTE`（「尚未探测」），放在现有状态常量旁。
- `src/console_cli.rs`：`resources_value_filtered` 按资源库 `detected_at.is_some()` 算 `ever_detected`，传给 `agent_entry`（签名加参）。从未探测时 agent 输出 state「未探测」、note「尚未探测」，其下模型 state「未探测」、note 空；探测过后维持现有 `agent_state` / `model_state` 两套判定，不动。
- 判定口径：`detectedAt` 为空即未探测，含旧版没写过该字段的资源库（首次重新探测后回到两态）。

### 2. 探测并行与进度（走查 #3）
- `src/resources.rs`：把 `detect_into` 拆成锁外准备与锁内应用两步：
  - 新增 `PreparedDetection` 枚举（`Pi(PreparedPiDetection)` / `Plain(PreparedAgentDetection)`）与 `prepare_agent_detection` / `apply_prepared_detection`（+ `apply_prepared_plain_detection`）；pi 原有的 `prepare_pi_detection` / `apply_prepared_pi_detection` 原样复用，`uses_pi_prepared_detection` 保留「pi 必须锁外探测」的判据供测试钉住。
  - `detect_all_resources_at` 改为调 `detect_all_resources_with_progress` 并传空回调；后者走 `detect_all_resources_inner`：锁内取 agent 清单（含 `ensure_agents` 补骨架）→ `std::thread::scope` 每 agent 一个线程锁外并行探测（`AtomicUsize` 计数，每个 agent 探测完回调 `(agent_id, done, total)`）→ `merge_prepared_detections` 在锁内按清单顺序一次合并、查 cc-switch 提示、盖 `detectedAt`、写盘。
  - 合并步骤抽成 `merge_prepared_detections`，顺序参照实现 `detect_all_resources_sequential_with`（`#[cfg(test)]`）与并行版共用它，保证「同输入同 store」。
  - 原 `detect_into` / `detect_into_supports_agent` 删除；单 agent 测试助手与 `live_detect_four_agents`（`#[ignore]`）改用新接口。
- `src/console_cli.rs`：探测尾部抽成 `detect_outcome`；新增 `resources_detect_command_with_progress(root, on_done)` 供窗口带回调调同一实现体（原 `resources_detect_command_at` 已无调用方，删除）。命令行 `resources detect` 走 `ResourcesOps` 注入路径，输出不变。
- `src/server.rs`：`resources_detect` 命令接 `tauri::AppHandle`，回调里 `emit("resources-detect-progress", { agent, done, total })`。
- 测试用假 `prepare` 注入（不真跑本机 CLI，全部临时目录）。

### 3. `config show` 提速（走查 #9）
- `src/channels/acp_setup.rs`：新增 `adapter_status_cached()`（`OnceLock<Mutex<Option<(Instant, Vec<AdapterReport>)>>>`，60 秒内复用）与 `clear_adapter_status_cache()`；`install_missing_blocking()`（即 `config set acpEnabled true` 的安装路径）装完后清缓存。命令行 `adapter_status()` 原样，不缓存。
- `src/server.rs`：`config_snapshot` 改用 `adapter_status_cached()`。
- 其它窗口读配置路径排查：`hosts_list`（hosts.rs，只读文件，无子进程）、`skill_status`（只读文件）、`slash_list`（只读文件）、`preset_save/remove`、`run_view`（只读文件 + 仅 `aicross open` 时另起窗口进程）、`config_recommend`（hosts::collect_hosts 只读文件与环境变量）——除 `adapter_status` 外没有每次拉起子进程的读路径，无需另改（详见「排查结论」）。

### 4. 擂台推荐分散（走查 #6）
- `src/recommend.rs`：`recommend_into` 新增同阶段已选清单 `stage_used_vendors` / `stage_used_models`（按阶段），来源与审查位同口径：`kept_effective` 的手选 / 预设写死、本轮新选、凑不出时角色默认补位，都只记非审查角色（执行与规划）。
- `choose_candidate` 加 `stage_used_vendors` / `stage_used_models` 参数；非审查位排序键改为 `[本阶段已选同一模型, 本阶段已有该厂商, 档位, 计费]`（前两项软条件，没有别的候选时自然允许重复）；审查位键值不变（补一位 0 对齐成 4 元数组）。只有一个执行位的阶段两个键恒为 0，结果不变。

### 5. 推荐输出给出路（走查 #4、#5）
- `src/recommend.rs`：新增 `unverified_counts(store)`——按 `model_state == 「未验证」` 计数（天然排除未发现、原生、额度用尽、未登录；agent 未探测的也算不进），返回 `[(agent, count)]`。`recommend_at_home` 输出加 `unverified: [{ "agent", "count" }]`，空时 `[]`；窗口 `config_recommend` 同结构（同一实现体）。
- `unfilled_reason` 加 `unverified_total` 参数：有未验证模型时在任何凑不出原因后追加「；另有 N 个未验证模型，测试连接后可参与推荐」（N 为全部这类模型总数）。
- 推荐候选池口径不变：仍只用 可用 / 已验证（任务书「不改」项）。

### 6. 评审位置的视角（走查 #7）
- `src/engine/mod.rs`：`preset_node_texts` 的评审文本表改成 `pub(crate) const JURY_PERSPECTIVES: [(&str, &str); 3]`（视角名 + 盖章文本一张表），新增 `jury_focus(index)`（扁平序取名，超出空串）。盖章文本内容不变。
- `src/console_cli.rs`：`mode_entry` 每个位置输出加 `focus`：`builtin-jury` 按 shaped 预设扁平序取 安全 / 性能 / 需求覆盖（数量调到 1、2 时相应变短），超出为空串；其余模式一律空串。与盖章共用一张表，没有抄两份。

### 7. skill 状态（走查 #11）
- `src/skill_install.rs`：`read_paths` 与 `install_path` 的路径一律逐段 `join`，Windows 上输出统一反斜杠、不混用 `/`。
- `SkillLocation` 新增 `outdated` 字段：`kind == external` 且版本可解析、低于内置版本时 true（链接位置同样适用）；我们装的仍用 host 级 `updateAvailable`。版本比较 `parse_version`（点分逐段转数字，任一段解析不了整条视为不比较 → false；前缀相等时短的小，如 1.21 < 1.21.1）。
- 测试里原来按字符串后缀断言路径的三处，改成按路径分量比较（`Path::ends_with`），Windows 与 Unix 都成立。

### 8. 已存在的短命令文件（走查 #14）
- `src/slash.rs`：`builtin_mode_items` 加 `home` 参数；`modes` 每项新增 `existing`：该模式 `command` 为 null 且 `<ax-><短名>` 命令文件在磁盘上时给 `{ "name", "state": 已写入 | 被占用, "path" }`，文件不在或已登记时 null。`slash add` 行为不变：带标记的照旧覆盖登记（测试覆盖），无标记的照旧退出 1 拒绝。

## 输出字段变化汇总

| 输出 | 变化 |
|---|---|
| `aicross resources`（及窗口 `resources` / `config_snapshot` 的 resources 段） | `detectedAt` 为 null 时：agent `state`=「未探测」、`note`=「尚未探测」；模型 `state`=「未探测」、`note`=`""` |
| 窗口事件 | 新增 `resources-detect-progress`，载荷 `{ "agent", "done", "total" }`，每个 agent 探测完一次 |
| `config show` 的 `modes[i].stages[j]` | 新增 `focus`（评审：安全/性能/需求覆盖，超出空串；其余模式空串） |
| `config recommend` / 窗口 `config_recommend` | 新增 `unverified`: `[{ "agent", "count" }]`（空时 `[]`）；`notes` 凑不出原因在有未验证模型时追加「另有 N 个未验证模型，测试连接后可参与推荐」 |
| `skill status` 的 `hosts[i].locations[j]` | 新增 `outdated`（external 且版本可解析、低于内置）；全部 `path` 在 Windows 统一反斜杠 |
| `slash list` 的 `modes[i]` | 新增 `existing`（`{ "name", "state", "path" }` 或 null） |

## 新增 / 改名测试

- `src/resources.rs`：`task57_parallel_detection_matches_sequential_merge`、`task57_progress_callback_fires_once_per_agent`、`task57_fake_prepare` / `task57_temp_root`（夹具）；`task26_detect_into_excludes_pi_prepared_outside_the_store_lock` 改名为 `task26_pi_is_detected_outside_the_store_lock`（判据函数改名，语义相同）。
- `src/console_cli.rs`：`task57_resources_reports_never_detected_before_first_probe`、`task57_config_show_focus_marks_jury_positions`；`write_fixture_store` 与 TASK-42 夹具补 `detectedAt`（否则按新口径会显示未探测），TASK-48「旧文件没有 detectedAt」子用例改为显式清掉该字段再断言，并顺带断言未探测状态。
- `src/recommend.rs`：`task57_arena_positions_spread_across_models`、`task57_arena_positions_repeat_when_pool_has_single_model`、`task57_unverified_models_counted_and_unfilled_reason_points_the_way`。
- `src/skill_install.rs`：`task57_external_location_below_bundled_is_outdated`、`task57_status_paths_use_native_separators`。
- `src/slash.rs`：`task57_mode_existing_reports_unregistered_files_on_disk`。

## 验证

工作树根目录（`D:\work\aicross-wt\t46`），`CARGO_TARGET_DIR=D:/work/aicross/target`：

```
cargo check --all-targets   →  Finished `dev` profile … in 4.22s，零 warning，exit 0
cargo check --release       →  Finished `release` profile … in 5.27s，零 warning，exit 0
cargo test                  →  624 passed; 0 failed; 10 ignored（均为标注真机的 live 测试）；无 warning
```

换行复查：`src/server.rs`、`src/main.rs` 全文件 CRLF，其余改动的 Rust 文件全文件 LF，无混合。测试全部用 `paths::test_temp_dir()` 临时目录，未写真实家目录或真实 aicross 数据目录，未真跑本机 agent CLI（探测用假 prepare 注入；ACP 安装路径测试沿用既有 fake 注入）。

## 窗口读配置路径排查结论（走查 #9 附带）

逐个核对窗口命令的实现体：`config_snapshot`（`adapter_status` → 已缓存）、`hosts_list`（hosts.rs 只读文件）、`resources_detect/smoke/prune`（探测与冒烟本就是长任务，非读路径）、`config_set_*` / `preset_*` / `slash_*` / `skill_*`（只读写配置文件与家目录文件）、`list_runs` / `load_run_view`（只读 run 文件）。除 `adapter_status` 外没有「每次读配置都拉起子进程」的调用；`aicross open` 另起窗口进程是功能本身，不在排查范围。

## 没做什么

- 推荐候选池口径、批量测试连接入口（`resources smoke`）未动（任务书「不改」项）。
- 走查 #1/#3 的界面呈现（#2 的「先探测」提示、#3 的进度条）、#4 的模式页提示、#8/#10/#12/#13 等 UI 侧条目属 TASK-58，未动 `src-ui`。
- `config show` 的 `effective.state` 未按「未探测」加态（走查未要求，保持现状）。

## 不确定处

- 旧版资源库（TASK-48 之前、无 `detectedAt` 字段）在下次探测前会整库显示「未探测」，与库里 `detected: true` 的旧标记不一致；按任务书「`detectedAt` 为空即未探测」的口径执行，重新探测一次即恢复。
- `existing` 对「文件已登记到别的模式」的情况也如实报磁盘状态（该文件带标记时显示「已写入」）；此时 `slash add` 会把定义改到新模式，与 add 的既有语义一致，但界面（TASK-58）如需区分要再议。
- 并行探测后，两个进程同时探测会各自跑一遍子进程、在资源库锁内先后合并，后写的覆盖先写的（与原顺序版在锁内的行为等价，只是子进程会重复跑一次）。

## 修正轮 r1

来源：`tasks/fix-57-r1.md`（Claude 子代理盲审，9 条均低）。只动了引擎行为涉及
的文件；`src/server.rs`、`src/main.rs` 本轮未改。另有一处超出原定文件清单：
金标准夹具 `fixtures/engine-golden/*.json` 按第 8 条「golden 测试相应更新」
做了数据更新（仅追加提示词文本，见下）。

### 逐条改法

1. **探测线程 panic**（`src/resources.rs`）：`PreparedDetection` 新增
   `Failed(String)` 占位档；并行探测的 join 失败不再 `expect` 作废整次，
   转成该 agent 的 `Failed`，锁内合并时记一条「探测 <agent> 失败：线程异常」，
   其余 agent 照常合并、盖 `detectedAt`、写盘。
2. **进度事件顺序**（`src/resources.rs`）：计数器从 `AtomicUsize` 换成
   `Mutex<usize>`，加一与 `on_done` 回调在同一把锁内（锁中毒时取回数据继续），
   事件里的 `done` 严格递增。`task57_progress_callback_fires_once_per_agent`
   去掉排序，直接断言回调序列就是 `1..=total`。
3. **kimi 的 `existing`**（`src/slash.rs`）：抽出共用函数
   `command_occupancy(home, host, name) -> (状态, 占用方路径)`，`slash add`
   与 list 的 `existing` 都走它；kimi 的 `~/.agents/skills/<名>` 已存在、或
   `~/.kimi-code/skills/<名>/` 目录已在而没有带标记的 SKILL.md，与文件无标记
   一样判「被占用」（路径给占用方路径）。
4. **项目路径斜杠**（`src/skill_install.rs`）：新增 `native_separators`，
   Windows 上把正斜杠统一成反斜杠；`status_at` 与 `validate_target` 对传入的
   项目根路径先统一再拼接，`inspect_location_full` 对 `read_link` 读出的
   `linkTarget` 输出前同样统一。其它平台原样返回。
5. **`unverified` 输出测试**（`src/console_cli.rs`
   `task50_recommend_window_output_matches_cli`）：断言全库已验证时输出
   `"unverified": []`；往资源库加一条未验证模型后重跑，断言
   `[{"agent":"pi","count":1}]`。
6. 删掉 `src/console_cli.rs` 里两处无作用的 `.map_err(|message| message)`
   （`resources_detect_command_with_progress` 与 `config_snapshot_at`）。
7. **同阶段已选清单的注释**（`src/recommend.rs`）：在 `stage_used_vendors` /
   `stage_used_models` 旁注明执行与规划合用一份清单是有意的，影响只在自定义
   模式同一阶段混排两类位置时出现（内置模式同阶段只有执行位，无区别）。
8. **如实汇报（新增）**（`src/engine/mod.rs`）：新增常量 `HONEST_REPORTING`
   （内容与出处按修正清单，注释注明 Language Models Are "Insecure" Reporters
   2026-09 的数据与中文句未单独实测），在 `build_request` 里于角色模板、
   `NO_EXTERNAL_ACTIONS` 之后统一追加 `

` + 常量——这是所有角色、所有
   模式、ACP 与一次性 CLI 两条通道共同的请求构造处，用户原文原样保留，
   不做开关。`docs/CONSOLE-CHANNEL.md` 派发一节补了一段说明。
   **golden 更新**：18 个有派发请求的夹具共 56 条 `stages[].request.nodes[].prompt`
   逐条追加 `

` + 该段文字，其余字段未动；13 个 precheck 夹具无请求、未动。

### 「不改」项

- ACP 缓存上限照旧：命令行装完适配器后，窗口的 `config show` 最多 60 秒
  才反映新状态（`adapter_status_cached` 的 60 秒复用窗口）。无测试。
- 缓存过期不补测试。

### 新增测试

- `src/resources.rs`：`task57_panicking_probe_becomes_one_agent_error`
  （注入对 codex 会 panic 的假探测；断言错误清单、其余 agent 的合并结果、
  `detectedAt` 与重新读盘）。
- `src/slash.rs`：`task57_kimi_existing_follows_add_occupancy_rule`
  （agents 目录占用与无标记命令目录占用都报被占用且给出占用方路径，add 对
  同一路径同样拒绝；带标记的 SKILL.md 仍是已写入、add 可直接登记）。
- `src/skill_install.rs`：`task57_r1_native_separators_only_replace_slashes`、
  `task57_r1_forward_slash_project_roots_are_normalized`（`D:/x` 形式项目根
  安装、状态输出不混斜杠、project 位置落在项目根下）；既有
  `junction_counts_as_external_with_target` 补 linkTarget 不含 `/` 断言。
- `src/engine/tests.rs`：`task57_r1_dispatch_prompts_end_with_honest_reporting`
  （盖章后 `build_request` 的每个节点提示词以该段收尾、恰出现一次，用户原文
  原样保留）。

### 验证

工作树根目录（`D:\work\aicross-wt\t46`），`CARGO_TARGET_DIR=D:/work/aicross/target`：

```
cargo check --all-targets   →  Finished `dev` profile … in 8.32s，零 warning，exit 0
cargo check --release       →  Finished `release` profile … in 8.52s，零 warning，exit 0
cargo test                  →  638 passed; 0 failed; 10 ignored（均为标注真机的 live 测试）；无 warning
```

换行复查：`src/server.rs`、`src/main.rs` 全文件 CRLF（本轮未改），本轮改过的
Rust 文件（resources / slash / skill_install / recommend / console_cli /
engine/mod / engine/tests）全文件 LF，无混合。测试全部用
`paths::test_temp_dir()` 临时目录，未写真实家目录或真实 aicross 数据目录。

### 没做什么

- 修正清单之外的行为变化没有；`src-ui/**` 未动；未 commit、未 push、未联网、
  未加依赖。
- `command_occupancy` 对「文件带标记但 kimi 的 agents 目录也在」判被占用，
  与 `slash add` 原判定顺序一致（此前 list 不看 agents 目录，现在两边同一套）。
