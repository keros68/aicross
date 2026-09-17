# TASK-28 首版审查 · Antigravity / gemini-3.8-flash-high（盲审，材料内联）

### 总判
**可交付**。任务书各项要求均已严格闭合，盲审边界收束严密，diff 非空保持原有严格不变，真实接线与反例红测覆盖完整，未触碰 TASK-27 禁改文件。

---

### 问题清单（按严重度）

本次审查未发现阻断交付的严重（Critical/High）缺陷，以下为 2 项低严重度（Low/Observation）的边界与鲁棒性观察项：

#### 1. 【低 / 鲁棒性】工作树降级与 2000 字截断同时发生时的截断标记漏判隐患
- **文件与位置**：`src/dispatch.rs:4497`
- **问题**：`truncated |= answer.chars().count() >= 2000;` 是在剥去工作树警告后对 `answer` 的字数进行判定。若底层工作树降级逻辑是在拼接警告前缀后对整个 `stdout_summary` 做了 2000 字截断（不确定底层是否对整体做过截断），则剥离掉约 30~50 字的警告后，`answer` 的实际字数将为 1950~1970 字，导致 `answer.chars().count() >= 2000` 判定为 `false`，从而漏在 `ANSWER.md` 末尾追加 `（回复较长，已截断至前 2000 字）`。
- **复现**：构造 `stdout_summary` 为 `worktree 不可用，已退回单写者：not git\n` + 1960 个字符（总长 2000 字代表已被引擎截断），调用 `prepare_outbound` 物化，产出的 `ANSWER.md` 不会带截断提示。
- **改法**：建议可同时放宽或结合原摘要长度判断，例如判定 `artifact.stdout_summary.chars().count() >= 2000 || answer.chars().count() >= 2000`（或者判定剥离警告后若原摘要触顶 2000 则置 `truncated = true`）。

#### 2. 【低 / 边缘防御】CRLF 下 `split_once('\n')` 的 `strip_prefix('\r')` 属无效防御
- **文件与位置**：`src/dispatch.rs:4475-4478`
- **问题**：在 `answer_summary` 中，`split_once('\n')` 在遇到 Windows 的 `\r\n` 换行时，左侧切片包含末尾的 `\r`，右侧切片（即 `answer`）是以正文开头的，其首字符根本不是 `\r`。代码中的 `answer.strip_prefix('\r').unwrap_or(answer)` 实际上永远命中 `unwrap_or(answer)`，属无害代码，但未能如作者意图清洗掉如果出现在行尾的其它潜在控制字符。
- **复现**：静态分析调用链路即可确认：对 `"prefix\r\nanswer"` 执行 `split_once('\n')`，切得 `(_, answer) = ("prefix\r", "answer")`，`answer.starts_with('\r')` 为 `false`。
- **改法**：若仅剥离首行警告，直接 `split_once('\n').map(|(_, a)| a).unwrap_or_default()` 即可；若需防御多余空行或换行符，可直接使用 `lines().skip(1).collect::<Vec<_>>().join("\n")` 或 `a.strip_prefix('\n')` 等标准方法。

---

### 核过没问题的要点

1. **只读问题盲审材料生成逻辑**：
   - 空改动判定严格：`artifacts.iter().any(|artifact| !artifact.diff.trim().is_empty() || !artifact.changed_files.is_empty())`，只有当所有上游产物的 `diff`（含 trim）与 `changed_files` 均为空时才触发生成 `ANSWER.md`。
   - 文件写入与名单：成功写入盲目录的 `ANSWER.md`，并在返回的材料名单 `names` 中追加该文件名。
   - Prompt 指路与内联：Prompt 中正确注入中性指路语 `执行者未改文件，其最终回复见 ANSWER.md`；且当 `< 30 * 1024` 字节时，以 `[ANSWER.md 全文]` 完整内联入 Prompt。
2. **多上游与单上游分节格式**：
   - 单上游产物直接输出正文，无额外多余标题。
   - 多上游产物按 `## 上游节点 <source_node_id>` 逐节排版；无来源标识时降级为顺序号 `1`, `2`，完全符合要求。
3. **盲审边界安全（不泄露推理、工具轨迹、引擎提示与密钥）**：
   - 引擎提示隔离：`WORKTREE_WARNING_PREFIX`（`worktree 不可用，已退回单写者：`）首行被安全剥离，不进入盲目录。
   - 推理过程与工具轨迹：核实各通道 runner（Claude 取 `result`、Codex 取 `agent_message`、Kimi 过滤 tool 等）在 `channel_output.text` 中仅存最终回复文本，不含思考链或工具中间轨迹。
   - 密钥与脱敏：统一走 `redact_text(&answer, redact)`，受外发脱敏开关控制，脱敏命中数累加进 `count`，密钥样式文本均被替换为 `[REDACTED]`。
4. **diff 非空时行为零变动**：
   - 遇到任一 diff 或改动文件非空时，`readonly_answer_material` 立即返回 `None`。
   - 盲目录中坚决不生成 `ANSWER.md`，Prompt 中无任何相关文字，条目依旧严格保持 `["REQUEST.md", "changed", "diff.patch"]`。
5. **`source_node_id` 新字段兼容性**：
   - Rust 结构体 `DispatchArtifacts` 增加 `#[serde(default, skip_serializing_if = "String::is_empty")] pub source_node_id: String`，反序列化旧数据自动填空串，空串序列化时自动省略，完全向下兼容。
   - 前端契约只在获准的 `src-ui/src/graph/types.ts` 中声明 `source_node_id?: string;`，未污染其它模块与共享上下文。
6. **文件换行与范围红线**：
   - 未触碰 TASK-27 的 `src-ui/src/console-*.ts`、`src-ui/src/components/**`、`src-ui/src/store/**`、`docs/MANUAL.md`。
   - `docs/PRODUCT.md` 保持 CRLF 换行且无 diff whitespace 告警，补充的口径定位精准；未运行 `cargo fmt`，未碰 backlog。
7. **测试覆盖与真实接线**：
   - 真实入口测试 `task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core` 完整贯穿 stage 1（执行 stub）到 stage 2（盲验收），断言盲目录落地文件与 prompt 任务单内容，切实杜绝了以往“只测纯函数漏接线”的回归隐患。
   - 覆盖非空 diff、剥离警告、脱敏、2000 字截断、多上游分节等全部反例，且报告中清晰留存了旧实现与变异测试下的变红证据（红测成立）。
