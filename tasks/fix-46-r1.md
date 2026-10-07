# fix-46-r1 修正清单

来源：Claude 子代理与 Antigravity（Gemini 3.8 Flash）两份盲审。编排者已核对代码的条目注明「已核实」。

## 要改

1. **同步窗口命令会卡住窗口主线程（Claude 独有，高，已核实）**：`src/server.rs` 里 `config_snapshot`、`config_set_default_mode`、`config_set_seat`、`config_clear_seat` 是同步命令，在主线程上跑；它们经 `get_resources_at` 取 `store_lock()`，而冒烟（`smoke_resource_model_at`）与探测在整个过程中持有同一把锁，最长 180 秒。`config_snapshot` 还会经 `acp_setup::adapter_status()` 拉起 `where.exe`。改法：六个窗口命令全部改成 `async` + `tauri::async_runtime::spawn_blocking`。
2. **严格读取把任何读取错误当成文件缺失（Claude 独有，中，已核实）**：`modes_from_strict` 用 `let Ok(text) = fs::read_to_string(..) else { return Ok(default) }`，文件存在但不是 UTF-8 或无权限时返回空配置，随后写命令会覆盖掉用户已有的人选，违反「文件损坏时退出 1、不覆盖」。改法：只有 `ErrorKind::NotFound` 返回默认值，其余读取错误返回 Err。`roles_from_strict` 同样的写法一起改。各加一条「非 UTF-8 文件 → 退出 1、文件内容不变」的测试。
3. **窗口探测另写了一份逻辑并丢掉说明（Claude 独有，中）**：`resources_detect_at` 与 `run_resources` 的 `detect` 分支各自实现，窗口版丢掉 `operation.errors` 与 cc-switch 发现提示。改法：照 `resources_smoke_at` 的做法抽出共用函数返回 `{ value, messages }`，命令行与窗口都调它；窗口命令把 `messages` 作为返回 JSON 的追加字段 `messages`（字符串数组，空时也给 `[]`）。
4. **窗口冒烟的参数校验与失败处理（两家共同，中）**：`--provider 需要 --model` 的校验只在 `run_resources_smoke`，窗口传 provider 不传 model 会冒烟该 agent 全部第三方模型；冒烟失败（命令行退出 1）在窗口里返回 `Ok` 且丢掉说明。改法：校验挪进共用的 `resources_smoke_at`，返回参数错误；窗口冒烟失败时返回 `Err`，内容为命令行 stderr 的那几行说明。
5. **空人选被当成「位置」（两家共同，低）**：`fill_seats` 对 `agent` 或 `model` 为空（trim 后）的人选也写入并标「位置」，阻断了规则 3。改法：这类人选按 `null` 处理，继续走规则 3；`thinking` 不在 `关 / 低 / 中 / 高 / 空串` 内时忽略（不覆盖预设值）。补测试。
6. **modes.json 读改写没有进程内锁（Claude 独有，低）**：窗口与命令行在同一进程内并发写会互相覆盖。改法：三条写命令共用一把 modes 专用的进程内锁，覆盖「严格读 → 修改 → 写」全程。跨进程不加锁，在 `docs/CONSOLE-CHANNEL.md` 注明「同时从窗口和命令行改人选，后写的为准」。
7. **写入时 seats 未按预设形状补齐（Gemini 独有，低）**：`ensure_seat_slot` 只扩到给定的阶段与位置，前面的阶段落盘为 `[]`。改法：写入时按该预设的阶段数与每阶段位置数补齐 `null`。
8. **文档与实现不一致（Claude 独有，低）**：`docs/CONSOLE-CHANNEL.md` 写 `set-seat` 的错误「均退出 2 并列出可选」，实际只有「模式不存在」「不在资源库」会列出可选；资源库读不了退出 3 没写。改法：按错误来源分别写，补退出码 3。
9. **测试缺口（两家，低）**：
   - 窗口命令与命令行对同一临时目录、同一参数的输出做完整 JSON 比对（至少 `config_snapshot` 对 `config show`、`config_set_seat` 对 `set-seat`），不只抽查几个字段。
   - 盲审模式在 `roles.json` 设了角色时，没有位置人选的来源仍是「未指定」（规则 3 的整份跳过）。
   - 同一阶段位置数多于预设的 seats。
   - `set-seat` 退出 2 的来源补：位置为 0、位置非整数、备选 agent 不在资源库。

## 不改

- Gemini「缺 `tasks/REPORT-46.md`」：报告已写，只是没有放进审查材料。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同任务书。改完在 `tasks/REPORT-46.md` 末尾追加「修正轮 r1」一节：逐条改法、新增测试名、三条验证命令结果。不 commit，不 push。
