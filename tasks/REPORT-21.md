# REPORT-21 接入信息的统一

## 结果

TASK-21 已按任务书 1–4 完成。左栏与接入页现在共用版本、模型名和详情构造规则；旧来源字符串在显示时直接转换，不要求重新探测；pi 的历史真身不符记录只按当前静态路由表重判，没有启动 CLI、发送请求或调用模型。

## 1. 组头只显示版本号

- `cliVersionNumber` 从 `--version` 原样输出中提取版本号，覆盖 `2.1.272 (Claude Code)`、`codex-cli 0.154.0`、`0.42.0`、`0.85.1`，无法识别时保留原文。
- 左栏 agent 行与接入页卡片都只显示提取后的版本号，`title` 保留原样输出。

## 2. 模型名与悬停详情的形状固定

- `modelDisplayName` 只在模型前缀与 agent 显示名同义时去掉前缀，例如 `kimi-code/k3-256k` 显示为 `k3-256k`；存储和派发仍使用原 `model.id`。
- `modelDetailLines` 是左栏悬停与接入页详情的唯一构造函数。前四行固定为名字、模型 ID、状态、来源；即使 `resolved_id == id`，模型 ID 行也存在。
- 可选行按额度用尽至、provider、登录状态、最近登录检查、最近验证、失败原因的顺序出现。
- `unverifiedCount` 仍只统计 `unverified`；旧失败记录改为 `failed`、已知路由别名改为 `verified` 后，左栏和接入页计数自动按新档位更新。

## 3. 来源串统一

- Rust 新写入统一为 `来源：<说明> · <YYYY-MM-DD>`：文件读取、Claude 帮助别名、静态别名表和用户申报都走同一构造函数。
- 写入和显示都会把 Windows、macOS 或 Linux 用户主目录缩成 `~`。
- 显示层兼容三种旧值：`读:<路径> (RFC3339)`、`claude --help 别名 (日期)`、`申报(日期)`；旧库无需重新探测。
- 合并探测结果时同时识别旧、新两种用户申报来源，不会把手动申报误标为未在事实源中。

## 4. pi 的验证状态与旧库重判

- 加载资源库时，只解析已有 `smoke_failure` 并查询 `KNOWN_MODEL_ROUTES`，不运行冒烟。
- 命中 `deepseek/deepseek-v4-flash → deepseek/deepseek-flash` 时改为 `verified` / `已接入`，`resolved_id` 写为 `deepseek-flash`，真身来源标为 `identity`，清除失败原因，并在备注中追加 `按已知路由别名重判（日期）`。
- 未命中的第三方失败记录保留失败原因，并统一改为 `verification: failed` / `status: 冒烟失败`。
- 普通冒烟失败写回继续明确使用 `failed`。重新探测会保留已有 `verification`，避免重判结果再次退回 `unverified`。

## 测试先红

- 前端关键测试先加入、实现尚不存在时，定向 vitest 共 10 项失败：版本号四个真实值与回退、模型前缀与固定四行、三种旧来源分别因对应纯函数不存在而失败。
- 临时移除加载路径的旧库重判调用后，`task21_known_route_alias_rejudges_old_smoke_failure` 失败：实际为 `unverified`，期望为 `verified`。
- 在补上重判后先不保留重新探测前的 `verification`，同一测试继续失败：重新探测后的实际值为 `unverified`，期望为 `verified`；补上保留逻辑后通过。
- 临时把普通冒烟失败写回改成 `unverified` 后，`task21_smoke_failure_is_recorded_as_failed` 失败：实际为 `unverified`，期望为 `failed`。
- 临时把新来源构造改回括号形状后，`task21_source_writer_uses_uniform_shape_and_short_home` 失败：实际为 `读取 ~\.codex\models_cache.json (2026-09-16)`，期望为 `来源：读取 ~\.codex\models_cache.json · 2026-09-16`。
- 上述临时改动均已恢复，定向与完整测试重新通过。

## 最终验证

未启动窗口、未连接真实模型。以下命令均由本次实现直接运行。

```text
> cargo check --all-targets
Compiling aicross v0.1.0 (D:\work\aicross-ui)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.92s
```

零 warning。

```text
> cargo test
running 461 tests
test result: ok. 450 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 23.68s
```

11 项 ignored 均为仓库既有的真机、真实模型或本地事实源测试，本次未启用。

```text
> cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  38 passed (38)
Tests  574 passed (574)
Duration  1.92s
```

```text
> cd src-ui && npm run build
✓ 342 modules transformed.
✓ built in 2.08s
```

```text
> python scripts/copy-audit.py
共 6 处命中，5 个文件（含误报，需人工复核）
```

审计退出码为 0。本次相关两处是 `src/resources/types.ts` 中兼容旧库所需的既有状态值 `冒烟失败` 与旧来源前缀 `读:`；它们不是新显示文案。其余四处位于本次未修改的文件。

换行复核：本次修改的 Rust、TS、TSX 文件均保持 LF；`src/server.rs`、`src/main.rs` 保持纯 CRLF，`src-ui/src/tauri.ts` 保持原有混合换行且三者均未修改。未运行 `cargo fmt`。

## 没做与原因

- 未改变首次派发自动验证规则，未增加批量自动冒烟：任务书明确排除。
- 未改模型的存储 ID 或派发参数，只改显示名：任务书明确要求保留派发词。
- 未改行结构、字号或分组折叠：TASK-21 不包含这些范围。
- 未启动窗口、未发真机请求、未调用真实模型：用户硬性要求禁止。
- 未修改 `src/server.rs`、`src/main.rs`、`src-ui/src/tauri.ts`。
- 未运行 `cargo fmt`，未 commit，未 push。
