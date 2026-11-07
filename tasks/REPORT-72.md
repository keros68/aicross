# REPORT-72 kimi 一次性派发固定目录

## 改了什么

- `skills/ai-cross/references/dispatch.py`：新增 `kimi_oneshot_dir`，独占并清空 `<AICROSS_HOME>/kimi-oneshot/`，退出时再清空；锁用 `fcntl.flock`（macOS/Linux）或 `msvcrt.locking`（Windows），非阻塞重试到超时。`call_kimi` 不再使用调用方给的 cwd，改在该目录里运行。
- `skills/ai-cross/tests/test_dispatch.py`：新增 `test_kimi_runs_in_one_fixed_emptied_dir`（两次调用同一目录、进入时为空、上一路残留文件被清掉、结束后为空）与 `test_kimi_waits_for_lock_then_gives_up`（锁被占时不调用 CLI、返回错误）。
- `skills/ai-cross/references/channels.md`：kimi 命令行一行与「一次性派发不留会话」一段同步。
- `skill-bundle/` 由 `python scripts/sync-skill.py` 重新生成（21 个文件）。

## 怎么验证的

- macOS，`cd skills/ai-cross && python3 -m unittest discover -s tests -q`：`Ran 75 tests ... OK`（本机无 pytest，用 unittest 跑同一组测试）。
- 真机（macOS，kimi 2.1.1）：`python3 references/dispatch.py smoke --entry kimi.moonshot` 连跑两次，低档、高档共 4 次调用全部「通过」。运行前 `~/.kimi-code/workspaces.json` 有 3 个工作区，运行后 4 个，新增的只有 `~/.aicross/kimi-oneshot`；该目录运行后为空。

## 没做

- 引擎 `src/` 里 kimi 盲审的工作目录未改（见任务书「不做」）。本机没有 cargo，`cargo test` 未跑；本期未改 Rust 代码，`skill-bundle` 变化只影响 `include_dir!` 打包的内容。
- Windows 上的 `msvcrt.locking` 分支未在真机运行。
- skill 版本号未改，留给发版提交。
