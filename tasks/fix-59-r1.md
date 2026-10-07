# fix-59-r1 修正清单（TASK-59，引擎）

来源：Claude 子代理盲审（读快照代码），结论「可以合并」，5 条均为低。

## 要改

1. **skills-manager 链接判定**（skill_install.rs `external_manager`）：先 `std::fs::canonicalize(path)` 再与 `~/.skills-manager`（同样 canonicalize）比对，覆盖带 `..` 的相对目标与链式链接；canonicalize 失败时退回现有逻辑。补测试（能建 symlink 的平台上造相对目标；建不了时跳过并注明）。
2. **视角文字校验**（console_cli.rs `validate_focus_text`）：先在原串上拒绝 `\n`、`\r`、`\t`，再 trim；另拒绝 Unicode Cf 类格式字符（零宽、双向覆盖等，如 U+200B–U+200F、U+202A–U+202E、U+2066–U+2069、U+FEFF）。补测试。
3. **读出口校验**（engine/mod.rs `jury_focus_effective` 及派发读取 focus 处）：读到的值用同一套校验，不合格的回落默认表，不报错。补测试（手写 modes.json 放入带换行 / 超长的 focus）。
4. **stale 比对**（slash.rs）：比对前去掉 BOM、把 CRLF 统一成 LF。补测试（CRLF 与 BOM 版本不算 stale）。

## 不改

- `--focus -性能` 的提示语：沿用既有 `option_value` 行为。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前。测试一律用临时目录。不 commit，不 push。改完在 `tasks/REPORT-59.md` 末尾追加「修正轮 r1」。
