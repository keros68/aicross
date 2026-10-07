# REPORT-61 更多 agent：支持清单 + Antigravity（引擎）

## 没做完、没验证的
1. 未实测四家（cursor / codebuddy / copilot / qoder）派发一律直接失败（「未实测通道：…暂不派发」，附命令模板）：本期不联网，官方文档未打开核对，stdin / 读文件用法拿不准，按任务第 4 节处理。命令模板、文档链接、copilot / codebuddy 的内置模型表（src/data/agent-models.json）、cursor-agent models 的输出格式均凭既有知识填写，未核对，只用假输出测过。
2. agy 的「测试」路径（resources smoke agy --model）未在真机跑（两次调用用在派发上），只有单元测试。
3. agy 作盲验收且有上游产物时，只能看到内联进提示词的内容（≤30 KB diff、验证输出）；REQUEST.md、changed/、项目只读副本是文件，agy 无头下读不到。本期未处理。
4. 改了现有断言（新增 agent 的直接后果）：两个 task39 测试 agent 数 4→9；两个 task57 测试的探测列表多出五家未安装；task15 名称测试改对支持清单断言（NATIVE_AGENTS / PI_AGENT 常量已删）。golden 未改。
5. src/engine/tests.rs 的新测试用 shell heredoc 追加（违反约定），事后核对内容完整、LF。

## 改了什么
- src/agent_catalog.rs（新）：九家支持清单（命令、兜底路径、版本参数、模型来源、homepage、untested、native、blind_only、core）。
- src/resources.rs：ResourceAgent 加 untested / homepage；ensure_agents、原生判定、resolve_cli 兜底、探测都按清单；agy 跑 `agy models`（30 秒），cursor 跑 `cursor-agent models`，copilot / codebuddy 读内置表，qoder 固定五档；厂商按模型名推，认不出记「未知」；非固定四家没装不报错；agy 冒烟真发最小派发。
- src/channels/cli_agents.rs（新）+ mod.rs：build_invocation 加 timeout 参数；agy 走 stdin 一行 NDJSON 的 stream-json 模板，不加跳过权限的参数，模型 id 自带档位后缀时不传 --effort；解析最后一行 result 事件；未实测四家派发直接失败。
- 「不要使用任何工具，直接用文字回答。」：引擎 build_request 在如实汇报后缀之后拼接（仅 agy 节点），通道里缺了才补，只出现一次。
- 只接盲位置：run_one_attempt 在首次验证前拦；config set-seat（窗口同一实现）对人选或备选拦住，退出 2。
- src/recommend.rs：untested 不进推荐；「未知」厂商不进验收 / 验证 / 仲裁位、不参与厂商去重；非盲位置去掉只接盲的 agent。
- src/console_cli.rs：资源输出每个 agent 加 untested、homepage。
- src/process.rs：版本参数取清单。
- docs/CONSOLE-CHANNEL.md：相应说明。

## 新增测试
agent_catalog::task61_catalog_lists_core_four_then_new_agents；channels::cli_agents 七个（agy 命令拼装 / effort / 非盲拒绝 / 未实测模板 / agy 成功解析 / agy 失败解析 / 未实测输出解析）；resources 五个（厂商推断 / agy 清单解析 / cursor 清单解析 / 内置表与 qoder 五档 / ensure_agents 补字段）；recommend::task61_recommend_skips_untested_unknown_vendor_reviewers_and_blind_only_shared；dispatch::task61_blind_only_agent_on_shared_seat_fails_before_dispatch；console_cli::task61_set_seat_rejects_blind_only_agent_on_shared_seat；engine::task61_agy_prompt_appends_no_tools_after_honest_reporting。

## 验证
cargo check --all-targets：0 warning；cargo test：660 passed，0 failed，10 ignored。main.rs 保持 CRLF，server.rs 未改，其余 LF。

## 真机
- resources detect：退出 0；agy 可用 1.3.1、18 个模型（Google 11、Anthropic 6、OpenAI 1，全 native、官方订阅）；四家未安装、untested、带官网。
- set-seat 把 agy 放共享位：退出 2，说明「纯文本盲审」。
- 盲派发 gemini-3.8-flash-low 问 17×23：第 1 次 status ERROR（--model 自带 -low 与 --effort medium 冲突），已修；第 2 次回答 391，tokens_in 11953 / tokens_out 747 / cache_read 0，节点 21.6 秒（agy 自报 10.2 秒、1 轮）。共调用 agy 2 次，临时目录已删。

## 没做的与理由
未实测四家的真机与文档核实（未装、不联网）；agy 的 isolation 仍记「部分」（未核实是否加载全局规则）；HOST_IDS 不扩；src-ui 留给 TASK-62；未 commit / push。

## 修正轮 r1

按 `tasks/fix-61-r1.md`「要改」逐条处理，「不改」（未实测四家派发即失败）未动。本轮没再调真机 agy。

### 没做完、没验证的
- 内联材料、冒烟专属目录、程序路径统一都只有单元测试，没在真机复跑 agy（清单要求不调）。
- 第 2 条「角色默认」：一键推荐凑不出合规人选时不再退回违法的角色默认，但它只能把位置留空并写 notes；roles.json 里已有的违法角色默认在派发时仍会补位，由派发前的位置检查拦下（报错，不发出），`config show` 的 `invalid` 会提前标出。
- 第 1 条的「推荐只填纯审查位」：按角色实现——agy 不进推荐的「验收」位，「验证 / 仲裁」位照常可进。经济模式的「验证」位在执行者没改文件时也会附项目副本，此时派发前会失败（不发出），推荐层不预判这种情况。

### 逐条改法
1. **agy 拿不到文件材料**（`src/dispatch.rs`）：盲节点派给只看得到文字的 agent（`agent_catalog::is_text_only`，即 agy）时，在备料与附验证输出之后调用 `inline_materials_for_text_only`：盲目录里的全部文件（REQUEST.md、diff.patch、changed/**、ANSWER.md、ACCEPT.md、VALIDATION.txt、附件）以「[材料全文 相对路径]」内联进提示词，提示词里已原样出现的内容不重复。总长上限 `TEXT_ONLY_INLINE_LIMIT = 200 KB`（常量旁注明依据与升级条件）。需要项目只读副本（盲目录里出现 `project*/`）、有二进制、超上限时返回错误，节点在进通道前失败。推荐：`seat_rule_violation` 让 agy 不进「验收」位。
2. **角色默认绕过限制**：`config set-role planning|execution` 设成只接盲的 agent 时退出 2（不写盘），acceptance 照常；`config show` 每个位置的 `effective` 新增 `invalid`（人选或备选违反位置硬规则时为原因，否则 null）；一键推荐（含 `--all`）凑不出人时，若角色默认违反硬规则（`seat_rule_violation`：只接盲 / 未实测 / agy 验收位 / 未知厂商非执行位），不占位、不写「使用角色默认」，改写「角色默认 X 不合规（原因），未采用；该位置需手动指定」。set-seat、set-role、show、派发共用 `agent_catalog::seat_violation`。
3. **agy 冒烟目录**（`src/resources.rs`）：`run_smoke_registered` 对 agy 建 `$AICROSS_HOME/scratch/smoke-agy-<uuid>` 空目录作工作目录，由 `SmokeScratch` 的 Drop 在成功、失败、超时（含 `?` 提前返回）后删除；其余四家仍用系统临时目录。
4. **agy 程序路径一致**：新增 `agent_catalog::resolve_program`（清单兜底路径存在就用，否则 PATH）。探测的 `resolve_cli` 对清单内命令、新 agent 通道的 `build_invocation`（派发与冒烟）都走它。kimi 的探测也经此函数，优先级与原来相同（兜底优先，再 PATH）。
5. **模型自带档位与思考设置**：新增 `channels::effective_thinking`（agy 且模型 id 以 -low/-medium/-high/-xhigh/-max 结尾 → 「由模型决定：low」，其余原样）。派发入口把 `node.thinking` 换成生效值，留痕 frontmatter 的 `thinking` 随之记实际值；run 摘要 `RunNode` 新增 `thinking`（只在与位置设置不同时写出，其余节点序列化不变，golden 不受影响）；`config show` 的 `effective.thinking` 记生效值。
6. **未知厂商只当执行位**：`seat_rule_violation` 对厂商「未知」且角色不是「执行」的位置一律排除（出方案 / 拆分 / 顾问 / 验收 / 验证 / 仲裁）；`choose_candidate` 里原来只挡审查位的判断移到这里。上轮断言「出方案可选未知厂商」的测试已改为断言排除，另补自定义模式含规划位的测试。
7. **cursor 解析失败**：`cursor_models_or_mark_missing`：`cursor-agent models` 解析失败时合并空清单（已有模型由 `merge_detected_models` 标「未在事实源中」、不再可选），并写「探测 cursor 失败：…」。固定四家的失败处理不变。
8. **官网**：codebuddy 的 homepage 改为 `https://www.workbuddy.ai`；通道注释与内置模型表的文档链接改为 `www.workbuddy.ai/docs/cli`。

`docs/CONSOLE-CHANNEL.md` 同步：agy 内联材料与上限、不发出的情况、思考强度生效值、冒烟专属目录、cursor 解析失败、set-role 限制、`effective.invalid`、推荐不退回违法角色默认。

### 新增 / 改动的测试
- 新增：`dispatch::tests::task61_r1_text_only_agent_gets_all_materials_inlined`、`dispatch::tests::task61_r1_text_only_agent_refuses_project_copy_binary_and_oversize`、`dispatch::tests::task61_r1_text_only_agent_on_acceptance_with_project_copy_fails_before_channel`（用不存在的模型名，万一拦截失效也不消耗额度）、`console_cli::tests::task61_r1_set_role_rejects_blind_only_agent_and_show_marks_invalid`、`recommend::tests::task61_r1_unknown_vendor_skips_planning_seat_in_custom_mode`、`recommend::tests::task61_r1_recommend_does_not_fall_back_to_illegal_role_default`、`resources::tests::task61_r1_agy_smoke_scratch_is_private_and_removed`、`resources::tests::task61_r1_cursor_parse_failure_marks_existing_models_missing`、`resources::tests::task61_r1_agy_program_resolution_is_shared`、`channels::cli_agents::tests::task61_r1_effective_thinking_follows_model_suffix_for_agy`、`engine::tests::task61_r1_run_summary_records_model_decided_thinking`。
- 改动：`recommend::tests::task61_recommend_skips_untested_unknown_vendor_reviewers_and_blind_only_shared`（agy 不再进验收位、改进验证位；未知厂商在规划位也排除）；`dispatch::tests::task61_blind_only_agent_on_shared_seat_fails_before_dispatch`（加留痕思考强度断言）；`console_cli::tests::task61_set_seat_rejects_blind_only_agent_on_shared_seat`（加 effective.thinking / invalid 断言）；`agent_catalog::tests::task61_catalog_lists_core_four_then_new_agents`（加 homepage、`is_text_only`、`seat_violation` 断言）。

### 验证结果
```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # Finished，0 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo test                  # 671 passed; 0 failed; 10 ignored
```
本轮源码改动都用编辑工具完成；`src/main.rs`、`src/server.rs` 本轮未改。
