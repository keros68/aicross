# TASK-50 完成报告 — 主 agent、skill 安装、一键推荐（引擎侧）

在 TASK-46 / 48 的未提交改动之上继续（同一工作树 t46）。未 commit、未 push。

## 改动与新增文件

新增：

- `src/hosts.rs` — 主 agent 检测与厂商推定、`--host` 解析、`set-host-vendor` 的实现体。
- `src/recommend.rs` — 一键推荐（候选池、档位表、选人规则）。
- `src/skill_install.rs` — skill 内置包（include_dir 嵌入）、位置判定（none / ours / external）、status / install / uninstall。
- `src/data/model-tiers.json` — 档位与计费小表（include_str! 嵌入）。
- `skill-bundle/ai-cross/` — skill 源仓去掉 `tests/`、`.git*`、`__pycache__`/`*.pyc` 的副本（21 个文件，308,831 字节 ≈ 302 KB；include_dir 不压缩，二进制体积增量约等于这个数加上宏元数据）。
- `scripts/sync-skill.py` — 同步脚本（整份替换式复制，源目录只读）。已运行一次，输出「已同步 21 个文件」。

修改：

- `Cargo.toml` — 新增 `include_dir = "0.7"`（任务书允许的那个 crate）与 `tauri-plugin-dialog = "2"`（允许的那个插件）。
- `src/orchestration.rs` — `modes.json` v2（`hosts.<id>` 分组：`vendorOverride` / `defaultMode` / `seats`）；`SeatChoice.origin`（`manual` / `recommended`，缺省视为 manual）；v1 宽松读当空配置、严格读报「modes.json 是旧格式，删除后重新设置」；`shape_seats_to_preset` 收敛到这里（set-seat 与推荐共用）。
- `src/engine/mod.rs` — `DispatchInput.host`、`EngineRun.host`（run 文件 `host` 字段，空不写出）；`PlanInput.host_modes`（按主 agent 的配置段补人）；`default_preset_id` 改按主 agent 配置段取 `defaultMode`；`resolve_engine_host`（单一配置推定）。
- `src/console_cli.rs` — 顶层 `hosts`、`skill` 子命令；config 子命令 `set-host-vendor`、`recommend`；config show / set-default-mode / set-seat / clear-seat 与 dispatch 的 `--host`（解析规则：给了必须检测到，没给时单一配置直接用、零或多于一个按规则退出 2）；写命令的选项与位置参数次序解耦（`unknown_option_error_allow_positionals` / `positional_values`）；窗口命令实现体全部加 host。
- `src/server.rs` — 窗口命令 `hosts_list`、`host_set_vendor`、`config_recommend`、`skill_status` / `skill_install` / `skill_uninstall`、`pick_folder`（全部 async + spawn_blocking）；原有 config 命令加 `host` 参数；注册 tauri-plugin-dialog（在单实例插件之后，单实例仍是第一个）。
- `capabilities/default.json` — 放行 `dialog:default`（插件按需放行；实际调用只在 Rust 侧）。
- `docs/CONSOLE-CHANNEL.md` — 主 agent 与 `--host` 规则、`aicross hosts`、`config set-host-vendor`、`config recommend`、`aicross skill status / install / uninstall`、modes.json v2 格式与 `origin`、run 文件 `host` 字段。
- `src/main.rs` — 三个新模块声明。`src/run_view.rs` — 测试夹具补 `host` 字段。

## 复用的现成函数（不另写逻辑）

- 厂商推定里查 pi 默认模型：`resources::find_target_model` + `resources::is_missing_from_detection`（未发现标记）。
- 推荐候选池：`resources::model_state`（只用「可用」「已验证」两态）。
- 补人与来源：`engine::fill_seats`；默认模式解析：`engine::default_preset_id`（改签名）。
- seats 整形：`orchestration::shape_seats_to_preset`；modes.json 原子写：`orchestration::save_modes_in`（内部 `write_json_atomic`）。
- 模式名与预设校验：`console_cli::resolve_preset_name`、`unknown_mode_message`、`modes_section`、`modes_write_lock`（推荐与写命令共锁）。
- 状态展示：`resources::agent_state` 等（未动）。

## 新增测试

hosts.rs：`resolve_host_follows_given_single_configured_and_need_host_rules`、`detected_host_ids_keeps_fixed_order_and_skips_undetected`、`claude_vendor_infers_from_settings_env_and_process_env`、`codex_and_kimi_vendors_follow_provider_names`、`pi_vendor_reads_resource_store_and_reports_missing`、`vendor_override_wins_and_auto_clears`、`collect_hosts_lists_only_detected_with_names`。

skill_install.rs：`bundled_version_reads_frontmatter_metadata`、`location_kinds_none_ours_and_external`、`junction_counts_as_external_with_target`（Windows junction）、`status_reports_duplicate_and_update_available`、`install_creates_updates_and_refuses_external`、`install_writes_agents_dir_for_codex_kimi_pi_and_project_scope`、`uninstall_removes_only_ours`、`status_output_structure_matches_cli_shape`。

recommend.rs：`task50_recommend_fills_empty_and_recommended_but_not_manual`、`task50_recommend_all_refills_manual`、`task50_recommend_unknown_host_vendor_notes_and_still_varies_stage`、`task50_recommend_empty_pool_leaves_empty_with_note`、`task50_recommend_subscription_before_per_call_and_deterministic`、`task50_recommend_single_mode_leaves_other_modes_untouched`。

console_cli.rs：`task50_config_commands_follow_host_rules`（--host 三种缺省情形与非法 host）、`task50_hosts_lists_detected_with_inferred_vendor`、`task50_set_host_vendor_writes_override_and_auto_clears`、`task50_recommend_window_output_matches_cli`、`task50_skill_commands_use_injected_home_and_match_impl`、`task50_dispatch_rejects_unknown_host_before_spawn`。

orchestration.rs：`task50_modes_v1_lenient_read_is_empty_and_strict_read_rejects`。原有 TASK-46/48 测试随 v2 格式与 `--host` 更新（modes.json 夹具包进 `hosts.claude`、命令加 `--host claude`、断言路径改 `hosts.claude.seats`）。

所有涉及家目录的测试（厂商推定、skill、set-host-vendor、recommend）都用临时目录注入，不写、也不依赖真实家目录的内容（推荐夹具的 verification/vendor 全部显式，不依赖原生判定——那会读真实 claude 配置）。

## 新增依赖与体积影响

- `include_dir = "0.7"`（任务书允许）：把 `skill-bundle/ai-cross/`（302 KB）嵌进可执行文件，release 二进制约增同量级体积（未单独链接对比，`cargo check --release` 只做元数据检查）。
- `tauri-plugin-dialog = "2"`（任务书允许）：`pick_folder` 窗口命令用，只在 Rust 侧调用，前端不装 npm 包。

## 验证结果（工作树根目录）

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # Finished dev，0 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release       # Finished release，0 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo test                  # 530 passed; 0 failed; 10 ignored
```

`python scripts/sync-skill.py` 运行一次：`已同步 21 个文件：D:\work\skill\xiaoyu-skill\skills\ai-cross -> skill-bundle\ai-cross`（源仓只读）。

换行核对（脚本统计）：`src/server.rs`、`src/main.rs` 全 CRLF；其余 Rust 文件与新文件全 LF。

## 未做与理由

- 窗口页面（选主 agent、skill 页、推荐按钮）是 TASK-51；本期只提供窗口命令与 `pick_folder`。原前端未改，旧调用少传 host 时按「缺省推定」规则照常工作（host 参数为 Option）。
- `aicross hosts` 的真机核对（编排者真机核）不做：那是实现者之外的一步。
- codex / kimi 的第三方端点推定只按 provider 名称匹配，不读各家 base_url 事实源——沿用 `resources.rs` `third_party_endpoint` 同样的现状，手册未宣称更多。
- 未做真实家目录的 `skill install`（任务书明令禁止）；skillmanager junction 的识别由单元测试里的临时 junction 覆盖。
- 档位表（`model-tiers.json`）里的具体条目是占位基线：真实模型 ID 会漂移，这张表按版本更新，本期只保证机制（前缀 / 关键字匹配、未知排后、先命中先用）与测试覆盖。
- Cargo.lock 随新依赖更新（已在工作树中，未提交）。

## 修正轮 r1（按 tasks/fix-50-r1.md，未 commit 未 push）

逐条改法：

1. **`config show --host` 跑不通**：`show` 分支删掉 `extra_arg_error(args, 3)`，只留 `unknown_option_error`（只认 `--host` 带值，位置参数仍拒）。
2. **`recommend --all` 跑不通**：新增 `unknown_option_error_with_flags`（`unknown_option_error` 改为它的空 flags 包装），`recommend` 把 `--all` 声明为不带值的开关；真正的缺值（如末尾 `--host` 无值）仍报错。
3. **厂商写法统一**：`hosts.rs` 新增全仓共用的 `normalize_vendor` / `is_known_vendor` / `KNOWN_VENDORS`，统一用资源库英文名（Anthropic、OpenAI、Google、Moonshot、Zhipu、DeepSeek、Alibaba、MiniMax、StepFun、OpenRouter）；推定规则表、`vendorOverride`（写入与读取都规范化）、推荐里的厂商比较（占位集合、过滤、排序）全部先规范化，不区分大小写，中文旧写法（智谱 / 月之暗面 / 阿里）也认。`set-host-vendor` 只认这些英文名或 `auto`，其它退出 2。主 agent 自己的模型（如 claude 原生）在推荐比较时按端点厂商算（`comparison_vendor_of_target` / `candidate_comparison_vendor`），不会被当成另一家选为审查者。URL 匹配改为先解析主机名再按标签比对（`url_hostname` / `host_matches_rule`，`z.ai` 不再误中 `buzz.ai`），补 `anthropic.com`、`z.ai`、`kimi.com`、`minimax`、`stepfun` 等端点。测试夹具改用英文厂商名（`infer_vendor` 的真实输出）。
4. **`writeTarget`**：`SkillLocation` 加 `write_target` 字段（camelCase `writeTarget`），`host_status` 里与 `install_path` 同路径的位置标 true。
5. **空位不再预先占位**：`kept_effective` 对无席位的位置一律 false；推荐凑不出人、角色默认会补位时才把默认人选的厂商计入占位集合，并在 notes 写「该位置将由角色默认 X 执行」（角色默认模板用 `fill_seats(preset, roles, None)` 预算）。
6. **推荐在锁内严格读 → 计算 → 写盘**：`recommend_at_home` 整个流程在 `modes_write_lock` 内，先 `modes_from_strict`（损坏或旧格式退出 1，不再按空配置算），写盘用同一份快照改出，只动本轮算过的模式。
7. **档位表 provider 前缀匹配**：`TierRule::matches` 的 provider 改前缀匹配；表补 `opencode`（中 / 按次）条目与 claude 别名 `fable`（高），并把 `flash` 规则提到 `glm-4` 之前（否则 `glm-4.7-flash` 永远先命中「中」）。
8. **`config_snapshot` 走主 agent 缺省规则**：实现体先过 `resolve_host_or_err`；host 为 null 且一份配置都没有时返回资源与空配置不报错；多于一份配置且未给 host 时与命令行一致报错。
9. **`ANTHROPIC_BASE_URL` 环境变量可注入**：`infer_vendor` / `infer_claude` 加参数（生产传 `std::env::var` 结果，测试传固定值），settings.json 仍优先；不再直接读测试进程的环境。
10. **`build.rs`** 加 `cargo:rerun-if-changed=skill-bundle`。
11. **安装先写临时目录再替换**：`write_bundle` 先写到同级 `.aicross-install-tmp-<pid>`（含标记文件），全部成功后才删旧目录、换名到位；失败时清理临时目录，旧安装原样保留。`install_at` 不再预先删 ours 目录。
12. **测试缺口**：「按次排后」夹具改为按次模型在资源库排前、同档（订阅仍胜）；补单一配置不给 `--host` 的写命令与 show 用例；可更新判定正向用例先放好标记文件（负向也改成真实的 ours 同版本）；补出方案优先高档用例；codex 新旧两处 duplicate 用例保留并补 `writeTarget` 断言。
13. **没给 `--host` 不为校验读资源库**：`resolve_host_or_err` 分两路——给了才读资源库（读不了退出 3）；没给只看 modes.json 结构，资源库读不了时传空 detected（不退出 3）。
14. **大小写与推定校验统一**：`set_vendor_override_at_home` 的 host 先转小写再比对；`resolve_host` 对推定出的主 agent 在 detected 非空时校验是否仍被检测到（已停用的退出 2）。
15. **capabilities** 去掉 `dialog:default`（对话框只在 Rust 侧调用）。
16. **路径存在但是普通文件**按 `external` 处理，不当 `none`。
17. **`--project` 等于家目录**：`host_status` 按规范化路径去重（Windows 下统一小写），不误报 duplicate。
18. **`skill_uninstall` 失败文案**改「卸载任务失败」。

文档同步（`docs/CONSOLE-CHANNEL.md`）：厂商取值（英文名、常见端点、覆盖只认这些值）、`--host` 缺省不读资源库与推定校验、`config_snapshot` 的 null 行为、推荐的角色默认 notes 与锁内写盘、档位表 provider 前缀匹配、`writeTarget`、同一物理目录去重、安装先写临时目录再替换。

新增测试（名字）：

- hosts.rs：`normalize_vendor_unifies_writings`；`claude_vendor_infers_from_settings_env_and_injected_env`（替换原 `…_and_process_env`，补注入环境变量路径与 settings.json 优先）；`resolve_host_follows_…` 补推定 host 停用与空 detected 两例；厂商中文名 / 大小写规范化断言并入 `vendor_override_wins_and_auto_clears`。
- recommend.rs：`task50_recommend_host_native_models_count_as_endpoint_vendor`、`task50_recommend_kimi_host_excludes_moonshot_reviewers`、`task50_recommend_role_default_not_placeholder_until_final`、`task50_recommend_planning_prefers_high_tier`、`task50_tier_table_matches_real_provider_prefixes`。
- skill_install.rs：`project_same_as_home_dedupes_locations`；`location_kinds_none_ours_and_external` 补普通文件与 `writeTarget` 断言；`status_reports_duplicate_and_update_available` 重写（标记文件先放好、双向都有意义）；`install_creates_updates_and_refuses_external` 补无临时目录残留断言。
- console_cli.rs：`task50_config_show_accepts_host_option`、`task50_recommend_accepts_all_flag_anywhere`、`task50_single_config_infers_host_without_flag`；`task50_set_host_vendor_writes_override_and_auto_clears` 补 host 大写与未知厂商两例。

验证结果（工作树根目录，本轮三条命令实跑）：

```text
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # Finished dev，warning 0 条
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release       # Finished release，warning 0 条
CARGO_TARGET_DIR=D:/work/aicross/target cargo test                  # 540 passed; 0 failed; 10 ignored
```

换行核对（脚本统计，本轮改后）：`src/server.rs`、`src/main.rs` 全 CRLF，其余 Rust 文件与 JSON / 文档全 LF，无混合。skill 相关测试全部用临时目录（`paths::test_temp_dir()`），未写真实家目录下的 `.claude` / `.agents` / `.codex` / `.pi` / `.kimi-code`。

## 修正轮 r2（fix-50-r2）

按 `tasks/fix-50-r2.md` 逐条修改；「不改」清单为空，未动其它内容。不改 `src-ui/**`，未 commit / push，未联网。

逐条改法：

1. **pi 主 agent 模型厂商**（`src/recommend.rs`）：新增 `is_host_native`——只有主 agent 是 claude / codex / kimi 且模型 provider 为空（原生模型）时才把比较厂商改写为主 agent 厂商；pi 下的模型（含默认模型）一律用资源库厂商。`comparison_vendor_of_target` / `candidate_comparison_vendor` 都走这个判断。
2. **`--project` 时的 `writeTarget`**（`src/skill_install.rs`）：`host_status` 按 scope 分别比较——global 位置与 `install_path(host, home, None)` 比、project 位置与 `install_path(host, home, Some(project))` 比，给了 `--project` 时全局写入位置不再标成 false。
3. **`ANTHROPIC_BASE_URL` 注入**（`src/hosts.rs`、`src/recommend.rs`、`src/console_cli.rs`、`src/server.rs`）：`collect_hosts`、`list_hosts_at`、`set_vendor_override_at_home`、`recommend_at_home` 加 `claude_base_url_env` 参数；生产入口（`run_hosts` / `run_config` / `set_vendor_override_at` / `recommend_at` / 窗口 `hosts_list`）传 `std::env::var("ANTHROPIC_BASE_URL")` 的结果，测试一律传 None，库内不再读真实环境。
4. **skill 更新替换**（`src/skill_install.rs`）：`write_bundle` 改为先建 skills 目录，旧目录 rename 成备份、装好的临时目录 rename 到位，成功后删备份，换名失败把备份 rename 回去；临时与备份目录名带 uuid，放在 skills 目录之外的同卷位置（skills 目录的上一级），不再落在 skills 目录里被 agent 当成第二份 skill。`host_status` 扫描时按前缀清理 skills 目录与其上一级残留的临时 / 备份目录（旧版留在 skills 目录里的也一并清）。新增 `skill_write_lock()` 进程内锁，`status_at` / `install_at` / `uninstall_at` 全程持有（codex、kimi、pi 共写 `.agents/skills`）。
5. **推荐读回不占锁**（`src/recommend.rs`）：`recommend_at_home` 的「严格读 → 计算 → 写盘」收进一个块，出锁后再调 `modes_section` 读回输出（`modes_section` 自己要拿同一把锁）。
6. **厂商规范化缺口**（`src/resources.rs`、`src/hosts.rs`、`src/recommend.rs`）：`infer_vendor` 补 gemini / google → Google；`normalize_vendor` 补「Moonshot AI」「Zhipu AI」「阿里云」；URL 规则补 `minimaxi.com`；新增 `participates_in_vendor_rules`——「其他」与空厂商不参与同阶段去重与主 agent 避让（占位集合、候选过滤、执行厂商排序都跳过），同时审查位按 agent/provider/model 记同人集合，同阶段不重复选同一个模型。
7. **单一配置未被检测到的文案**（`src/hosts.rs`、`src/console_cli.rs`、`src/recommend.rs`）：`HostResolve` 新增 `ConfiguredNotDetected(String, Vec<String>)`，没给 `--host` 且 modes.json 唯一配置已停用时报「modes.json 里唯一的配置 <host> 未被检测到，请用 --host 指定，可选：…」；给了非法 `--host` 仍是「未知 host：…」。
8. **同阶段冲突终检**（`src/recommend.rs`）：新增 `append_stage_conflict_notes`，写盘前对每个阶段按最终人选核对审查位（含角色默认补位的空位），同阶段厂商重复或撞上主 agent 厂商时追加 notes（「其他」与空厂商不参与）。

文档同步（`docs/CONSOLE-CHANNEL.md`）：`--host` 缺省校验的新文案、推荐规则的 pi 厂商口径与「其他」厂商、角色默认冲突 notes、skill 安装的备份换名与临时目录位置、status 清理残留、进程内写锁。

新增测试（名字）：

- recommend.rs：`task50_recommend_pi_host_models_keep_resource_vendors`、`task50_recommend_unrecognized_vendors_do_not_block_reviewers`、`task50_recommend_stage_conflict_note_when_role_default_kept`。
- skill_install.rs：`write_target_marks_global_and_project_separately`、`status_sweeps_leftover_temp_and_backup_dirs`；`install_creates_updates_and_refuses_external` 的残留断言扩到 skills 目录上一级与备份前缀。
- resources.rs：`infer_vendor_maps_model_id_keywords`。
- hosts.rs：`normalize_vendor_unifies_writings` 补「Moonshot AI」「Zhipu AI」「阿里云」断言；`resolve_host_follows_…` 的停用情形改断言 `ConfiguredNotDetected`。
- console_cli.rs：`task50_single_config_infers_host_without_flag` 的文案断言改为新消息；`run_hosts_at` / `run_config_at_args` / 窗口输出对比用例补 None 环境变量参数。

验证结果（工作树根目录，本轮三条命令实跑）：

```text
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # Finished dev，warning 0 条
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release       # Finished release，warning 0 条
CARGO_TARGET_DIR=D:/work/aicross/target cargo test                  # 546 passed; 0 failed; 10 ignored
```

换行核对（脚本统计，本轮改后）：`src/server.rs`（504 行）、`src/main.rs`（276 行）全 CRLF；`src/hosts.rs`、`src/recommend.rs`、`src/skill_install.rs`、`src/console_cli.rs`、`src/resources.rs`、`build.rs` 全 LF，无混合。skill 相关测试全部用临时目录（`paths::test_temp_dir()`），未写真实家目录下的 `.claude` / `.agents` / `.codex` / `.pi` / `.kimi-code`。

没做：备份还原失败的直接用例（rename 在同目录内几乎不会失败，注入困难）；`HostResolve::ConfiguredNotDetected` 在「给了 --host」分支不可达，与 `Unknown` 合并匹配。
