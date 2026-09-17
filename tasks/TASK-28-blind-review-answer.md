# TASK-28 只读问题的盲审补上答案文件（09-17）

对应 backlog 77。**只做这一件事。**

**另有一期在飞（TASK-27，控制台任务详情页），这些文件一律不要碰**：`src-ui/src/console-*.ts`、`src-ui/src/components/**`、`src-ui/src/store/**`、`docs/MANUAL.md`。本期原则上只改 Rust；确需改前端契约时只准动 `src-ui/src/graph/types.ts` 与 `src-ui/src/tauri.ts`（字节级最小改动），并在报告里说明为什么非改不可。

## 问题（编排者 09-17 窗口实测）

控制台里用「经济模式」派「用一句话说明 beta.py 的用途，不要改任何文件」：执行两次都答对了（「`src/beta.py` 是一个字符串工具模块，提供 shout 和 whisper …」），但验收、验证两路**两轮都判 NEEDS_FIX**，理由是「待验材料中不包含任何说明文字」。白跑一轮修正，共 7 路 357.7 秒。

原因：盲目录物化（`src/dispatch.rs` 的 `prepare_outbound`，约 4484–4531 行）只铺 `REQUEST.md` / `diff.patch` / `changed/`（另可附 `VALIDATION.txt`）。执行者的最终回复只进了上游产物的 `stdout_summary`（`dispatch.rs` 约 4110 行，取 `channel_output.text` 前 2000 字），从没进盲目录。对改代码的任务无所谓；对只读问题，答案本身就是交付物，评审只能判不通过。

## 要做

1. 盲目录物化时，**上游产物的 diff 全部为空、且 `changed_files` 为空**时，把上游的执行者最终回复写成 `ANSWER.md` 放进盲目录，材料名单加上它，prompt 里补一句指路（参照 `VALIDATION.txt` 的做法，文案中性简短，例如「执行者未改文件，其最终回复见 ANSWER.md」）。
   - 多个上游时逐个写，带上游节点标识分节；只有一个上游时不加多余标题。
   - 内容来源用 `stdout_summary`，但**先剥掉 worktree 警告前缀**（那是引擎自己的提示，不是执行者的话）。若确认 `stdout_summary` 在某些 runner 下混有过程输出 / 工具轨迹而不只是最终回复，报告里写清证据，并改用最终回复文本来源；不要把推理过程送进盲目录。
   - 走现有脱敏（`redact_text`，受外发开关影响的口径与 `REQUEST.md` 一致）。
   - 2000 字截断时在文件末尾写明「（回复较长，已截断至前 2000 字）」。
   - 小于 30KB 时与 `diff.patch` 一样内联进 prompt。
2. **diff 非空时行为完全不变**：不写 `ANSWER.md`、盲目录条目仍是 `["REQUEST.md", "changed", "diff.patch"]`（现有测试 `dispatch.rs` 约 8950 行钉着）。
3. 这是对「验证者默认盲」的一次口径确认，不是放宽：答案是产物不是推理。在 `docs/PRODUCT.md` 讲盲审材料的位置补一句这个口径（CRLF 文件，保持换行）。

## 不做

- 不改修正轮逻辑、不改验收结论解析、不改 `stdout_summary` 的长度与既有用途、不改留痕 schema 字段名。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。

- **本仓连吃四次「只测纯函数漏接线」**：至少一格走 `dispatch_stage_core` 这条真实入口（参照 TASK-26 R1 的 `task26_r1_*` 测试用本地 stub runner 的写法），上游是一个不改文件、只回一句话的执行节点，下游盲审节点的盲目录里出现 `ANSWER.md` 且内容是那句话、prompt 含指路句。
- 反例各一格：diff 非空不出 `ANSWER.md`；worktree 警告不进 `ANSWER.md`；密钥样式文本被脱敏；超长截断带说明。
- 每条先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF，其它 Rust LF，`src-ui/src/tauri.ts` 混合只做字节级最小改动，`docs/PRODUCT.md` CRLF）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-28.md`。backlog 不用改（与 TASK-27 同文件相邻行，编排者合并时统一划）。
