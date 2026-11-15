# REPORT-82：Windows 核对后留给 skill 的三处

按 `tasks/TASK-82-after-windows-checks.md` 实施。只改了 skill；skill 版本 1.30.4 → 1.30.5。

## 改了什么

- `references/dispatch.py`：`review` 加 `--material-file`；`NO_TOOLS_FILE`，`call_agy` 写 `REQUEST.md` 时替换；`engine_host` 取代按表查名。
- `SKILL.md`、`references/channels.md`：对应说明。
- 测试：`test_engine_config_is_none_without_aicross_or_for_unknown_hosts` 原断言「不认识的宿主不调用引擎」改为「宿主名原样交给引擎」；agy 文件模式的测试加两条断言。

## 验证

macOS，Python 3.11.15：

```
$ cd skills/ai-cross && python3 -m unittest discover -s tests
Ran 109 tests in 1.942s

OK (skipped=1)
```

拉取 `6dc4a98` 后先跑了一遍，同样是 109 条通过。

## 没做什么

- agy 的改动没有真机跑：本机没有 agy。
- 引擎对不认识的宿主名怎么回应没有真机看：本机没有引擎。读 `src/console_cli.rs`，`config show` 经 `resolve_host_or_err` 返回错误码，`start` 会打印原因后改看 manifest。

## 不确定处

- `REPORT-81` 记的本机 `aicross --version` 是 0.3.7，源码是 0.3.10。那次对引擎行为的核对针对的是 0.3.7。
- `6dc4a98` 的 `_mode_has_seat` 把只有 `unfilledReason`、没有人选的位置也算作已配置。引擎在什么情况下给出 `unfilledReason` 没有核对。
- 聚合型宿主（Qoder、WorkBuddy 等）在装了引擎的机器上，`start` 每次会多打印一行引擎不认这个宿主的原因。
