# TASK-85 引擎被结束时带走子进程，并发记录不互相覆盖

## 问题

1. Unix 上引擎被结束时，它拉起的 agent 进程留下来继续运行、继续消耗额度。Windows 靠 Job Object 处理，Unix 没有对应的一层。
2. 两个并发节点的 agent、模型、角色相同且同一秒结束时，派发记录的文件名相同。`write_record` 先查文件是否存在再写，后写的一份盖掉先写的。`task75_kimi_blind_concurrent_…` 单独运行时 14 次失败 8 次，就是这个原因。两个系统都受影响。
3. 还没探测过资源时，`--host` 的报错是「未知 host：claude；检测到：（无）」，没有说下一步。ai-cross skill 把这句原样转给用户。
4. TASK-84 发现进程号从 `u32` 转 `i32` 没查范围。需要确认 `src/` 里没有第二处。

## 范围

只改 `src/`。

1. 命令行子命令启动时登记 SIGTERM、SIGHUP、SIGINT 的处理：收到后结束自己的全部子孙进程再退出。窗口进程不登记。SIGKILL 与崩溃不在范围内。
2. `write_record` 用 `create_new` 占住文件名。
3. 没有检测到任何主 agent 时，报错改为说明先探测。四处报错共用一个函数。
4. 搜一遍 `as i32`、`libc::kill` 的调用，逐处核对进程号来源。

## 不做

- 不用进程组结束子进程（终端里的 Ctrl-C 要能到 agent 的命令行）。
- 不改 Windows 的 Job Object 与 `taskkill` 分支。
- 不改版本号，不发 Release。

## 验证

`cargo check && cargo test`；并发记录那条测试单独连跑 25 次；真机做一次盲审派发，中途对引擎发 SIGTERM，核对子孙进程全部退出。

## 交付

`tasks/REPORT-85.md`。
