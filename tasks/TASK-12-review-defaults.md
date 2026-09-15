# TASK-12 修正轮上限、验收与盲审并行、审查材料附带验证输出（09-15 定案）

依据 `docs/PRODUCT.md`「第 4 期草案：把编排经验做成默认值」第 1、2、5 条（用户裁决合成一期）。改动横跨引擎与界面，契约只追加。

## 契约（只追加）

- `DispatchSettings` 追加 `fix_round_limit: usize`（serde default 1；前端 `UiDispatchSettings.fixRoundLimit`，设置页「派发」栏一个数字框，文案「修正轮上限」）。现有 `reducePipelineStep` 里写死的 `count >= 3` 改读这个值。
- `Node` 追加可选 `leftover?: { source_node_id: string; items: string[] }`（Rust `Node.leftover` 镜像，serde default），表示「遗留」节点：只读清单，不可派发，`dispatchableNodeIds` 排除。
- 派发请求 `DispatchNodeRequest` 追加可选 `review_material_validation?: boolean`（Rust `review_material_validation`，default false）：为 true 时引擎在派该节点前先在其材料来源目录跑一遍验证命令，把输出（截断 8 KB、脱敏后）写进盲目录 `VALIDATION.txt`，并在节点 prompt 末尾追加一段「验证命令输出见 VALIDATION.txt」。
- `fixtures/prompts/acceptance.txt` 与 `blind-prefix.txt` 末尾追加两条固定关注点（原文）：「契约（字段、命令签名、序列化大小写）有没有变」「新增的测试能不能红：把修复回退后该测试是否会失败」。Rust 读文件填入，前端不复制文本。

## 实现

1. **修正轮上限**：`reducePipelineStep` 到上限后不再派修正节点，而是生成一个「遗留」节点挂在最后一个验收节点下（`leftover.items` = 验收结果里未闭合的条目：按行取「- 」或编号开头的行，取不到就整段截 20 行），文案「遗留：N 条未闭合，来源 <验收节点>」。图上节点卡显示「遗留」标签，点开面板只读展示清单，可复制。结果屏「流水线结果」在遗留节点处显示同一清单。
2. **验收与盲审同阶段并行**：内置预设 经济模式 / 盲审模式 的验收阶段改成两路同阶段：一路角色「验收」（现有验证命令 + 验收提示词）、一路角色「验证」（盲、只读、`review_material_validation: true`）。两路结果并排；验证门（validation_failed → 修正轮）只看「验收」那一路。`PRESET_NODE_TEXTS` / 讲解页相应补一句。
3. **审查材料附带验证输出**：`run_one` 在盲目录物化后、派发前，若 `review_material_validation` 为真且验证命令非空，跑一次验证命令（超时用节点超时的一半），输出落 `VALIDATION.txt`；命令为空则写一行「未配置验证命令」。派发记录（`.dispatch/`）frontmatter 追加 `validation_attached: true|false`。
4. **测试**：`reducePipelineStep` 上限 1 / 上限 3 各一条 + 遗留节点生成与 `dispatchableNodeIds` 排除；Rust `VALIDATION.txt` 物化（真跑一个 `exit 0` / `exit 9` 命令）、截断、脱敏；预设两路同阶段的 stamp 测试；提示词文件末尾两条关注点的存在性测试。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src/server.rs` CRLF，其它 Rust LF，`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-12.md`。
