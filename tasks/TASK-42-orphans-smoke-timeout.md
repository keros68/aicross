# TASK-42 引擎被杀不留孤儿进程；冒烟超时不再判验证失败（10-01，积压 97、104）

**只改 `src/process.rs`、`src/resources.rs`、`src/engine/mod.rs`（仅派发前检查对冒烟状态的判定）、`src/console_cli.rs`（仅 `resources` 输出的状态映射调用处）、`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`。不 commit，不 push。**

## 1. 引擎被杀时连带结束它启动的 CLI 子进程（积压 97）

10-01 真机：杀掉后台引擎（`aicross __run`）后，它正在等待的 pi 进程继续运行成孤儿，额度照扣、结果无人收。

原因（编排者已核）：`src/process.rs` 里 Windows 的 kill-on-close Job Object 只在 ACP 长驻会话的启动处挂了（`assign_process_to_job(child.id())`，约 291 行）；一次性派发走的 `run_process_registered_with_activity`（约 425 行起，`.spawn()` 处）没有挂。

要求：一次性派发与冒烟的子进程在 `spawn()` 之后同样挂进 Job Object（子进程之后再起的孙进程随之进入同一个 Job）。非 Windows 平台保持现状，在文档注明。测试：Windows 下起一个会自己再拉起子进程的短命令，断言子进程已挂进 Job（例如查询进程的 Job 归属，或「结束持有 Job 句柄的测试进程后子进程随之结束」任选一种可在单测里稳定验证的做法）；做不到稳定单测时，至少把挂 Job 的判断抽成可测的函数并测调用路径，真机由编排者核。

## 2. 冒烟超时单独记，不拦截派发（积压 104）

10-01 真机：对 `zai-coding-cn/glm-5.3` 冒烟 60 秒超时，被记为「验证失败」，派发前检查随后直接拒掉该目标；而同一模型当天作实现者一直正常。

要求：
- 冒烟超时放宽到 180 秒（`src/resources.rs` 约 1674 行的 `Duration::from_secs(60)`）。
- 冒烟因超时结束时，`verification` 记新值 `timeout`（只追加取值，不改字段），`smoke_failure` 写「冒烟超时（180 秒）」。
- 派发前检查（`src/engine/mod.rs` 里「目标验证失败」那一条）只拦 `verification == "failed"`，`timeout` 不拦。
- `aicross resources` 的模型状态映射：`timeout` → 「冒烟超时」，note「可重试 aicross resources smoke <agent> --model <id>；不影响派发」。状态优先级插在「验证结果」一档内（与已验证 / 验证失败并列）。
- 资源库旧数据不受影响（没有 `timeout` 值）。

测试：超时结果映射为 `timeout` 而不是 `failed`；派发前检查对 `timeout` 放行、对 `failed` 照旧拒绝；状态映射与 note；超时时长常量为 180 秒。新测试在修复临时改回时必须变红。

## 3. 文档

`docs/CONSOLE-CHANNEL.md` 与 `docs/MANUAL.md`：模型状态取值补「冒烟超时」；说明引擎退出时会连带结束它启动的 CLI 子进程（Windows），其它平台不保证。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
```

每个改动文件保持原有换行；新文件 LF。写 `tasks/REPORT-42.md`：改动、测试名、回退验证、三条命令输出、未做与理由。

## 编排者真机核

真派一次，在 pi 运行中杀掉后台引擎进程，确认 pi 随之结束；对 `zai-coding-cn/glm-5.3` 冒烟一次（结束后把资源库恢复）。
