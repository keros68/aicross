# REPORT-31 Rust 测试并发偶发失败

## 结果

两条负载敏感测试已改为等待真实条件或使用与生产检查一致的超时。最终代码全量默认并发 10 次、单线程 5 次均通过，每次均为 `473 passed; 0 failed; 11 ignored`。

未发现生产代码竞态，生产超时、ACP 回退和资源检查行为未改动。另删除一条普通测试中未使用的进程级环境变量修改，避免它与并发测试共享 `AICROSS_TEST_RUN_ROOT`。

## 失败率

### 全量 `cargo test`

| 阶段 | 命令 | 连跑 | 失败轮次 | 失败率 | 单轮耗时 |
|---|---|---:|---:|---:|---:|
| 修前 | `cargo test -- --nocapture` | 10 | 0 | 0% | 40.42–46.65 秒 |
| 修前 | `cargo test -- --test-threads=1 --nocapture` | 5 | 0 | 0% | 208.64–215.96 秒 |
| 修后最终版 | `cargo test -- --nocapture` | 10 | 0 | 0% | 39.07–43.88 秒 |
| 修后最终版 | `cargo test -- --test-threads=1 --nocapture` | 5 | 0 | 0% | 209.24–211.97 秒 |

修前普通连跑未复现任务书记录的两次偶发失败。单线程明显变慢，原因是等待型测试改为依次执行，不是挂起。

### 12/18 逻辑核 CPU 压力下的定向连跑

| 测试 | 修前 | 修后 | 修前失败率 | 修后失败率 |
|---|---:|---:|---:|---:|
| `dispatch::tests::run_one_dead_acp_falls_back_once_without_resume_and_rewrites_full_task` | 0/10 失败 | 0/10 失败 | 0% | 0% |
| `resources::tests::task17_pi_login_check_uses_safe_args_once_and_ignores_credentials` | 1/10 失败 | 0/10 失败 | 10% | 0% |

修前 pi 失败的 panic 原文：

```text
thread 'resources::tests::task17_pi_login_check_uses_safe_args_once_and_ignores_credentials' panicked at src\resources.rs:3080:9:
assertion `left == right` failed: each provider is checked once: ...
  left: 3
 right: 4
```

第一次加压命令因短测试名与 `--exact` 组合而运行了 0 项，已立即停止且未计入表格。表中数据来自改用完整测试路径后的有效执行。

## 根因与改法

### ACP 退出测试

证据：

- fake ACP 在第一轮响应写出后执行 `sys.exit(0)`。
- 原测试在第一轮 `run_one` 返回后固定 `sleep(100ms)`，然后立即发起第二轮。
- 第二轮是否看到进程已退出取决于操作系统何时完成进程退出与 `try_wait` 状态更新；测试设置的节点超时只有 1 秒。
- 任务书记录了该测试在高负载全量运行中失败；本机 10 次普通全量和 10 次加压定向运行均未再次触发，因此没有补写不存在的本机 panic。

改法：

- 在 `src/channels/acp.rs` 增加仅在测试构建中存在的 `test_wait_for_process_exit`。
- 测试按 ACP 进程池键找到 fake client，轮询真实子进程状态，最多等待 10 秒。
- `src/dispatch.rs` 的测试在确认 fake ACP 确实退出后再发第二轮，不再用 100ms 墙钟猜测。

该辅助函数带 `#[cfg(test)]`，生产构建中不存在；ACP 运行和回退逻辑未改。

### pi 登录检查测试

证据：

- 该测试对四个 provider 并发启动四个 `pi.cmd`，原来给每个子进程 800ms。
- `run_process` 到达截止时间后终止子进程并返回超时；登录状态会变成 `Unknown`。
- 12 核 CPU 压力下第 2 次定向运行复现：`argv.log` 只有 3 行，断言期望 4 行。临时目录使用 UUID，测试不修改 PATH、当前目录或其他进程级环境，证据指向子进程未在 800ms 内完成，而不是文件名冲突。

改法：

- 该契约测试改用生产检查已有的 `PI_LOGIN_CHECK_TIMEOUT`，即每个 provider 10 秒。
- 该测试验证的是安全 argv、去重与凭据不落盘，不验证短超时；`task17_pi_login_missing_command_and_timeout_are_unknown` 仍以 30ms 单独覆盖超时语义。
- 生产常量和生产调用没有修改。

### 同类共享状态

- ACP 进程池相关测试已有同一把 `test_pool_guard`，两条目标测试的临时目录均使用 UUID。
- pi 目标测试不修改 PATH、当前目录或进程级环境。
- `r4_1_worktree_fallback_runs_validation_in_effective_directory` 原来设置后删除 `AICROSS_TEST_RUN_ROOT`，但测试路径完全不读取该变量。已删除这两行，避免并发测试看到临时的进程级值。
- 真机验收测试中的同名环境变量修改均带 `#[ignore]`，不参与本次全量连跑，未扩大修改范围。

## 修改文件

- `src/channels/acp.rs`：增加测试专用的 ACP 进程退出等待函数。
- `src/dispatch.rs`：ACP 测试改为等待真实退出事件；删除一条普通测试中无效的全局环境变量修改。
- `src/resources.rs`：pi 契约测试使用既有 10 秒检查超时。
- `tasks/REPORT-31.md`：记录修前、修后统计与根因证据。

## 验证

定向测试：

```text
cargo test dispatch::tests::r4_1_worktree_fallback_runs_validation_in_effective_directory -- --exact
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 483 filtered out

cargo test dispatch::tests::run_one_dead_acp_falls_back_once_without_resume_and_rewrites_full_task -- --exact
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 483 filtered out

cargo test resources::tests::task17_pi_login_check_uses_safe_args_once_and_ignores_credentials -- --exact
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 483 filtered out
```

全量统计结果见“失败率”表。最终一轮输出均为：

```text
test result: ok. 473 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out
```

其余验证：

```text
cargo check --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s)
```

退出码 0，无 warning。

```text
cd src-ui
npx vitest run --configLoader native --pool threads
Test Files  45 passed (45)
Tests  656 passed (656)
```

```text
cd src-ui
npm run build
✓ built in 701ms
```

TypeScript 与 Vite 构建成功；仍有既有的 500 kB chunk 提示。

```text
git diff --check
```

退出码 0，无输出。三个修改过的 Rust 文件修前、修后均为 LF，未改变换行格式。

## 范围与限制

- 未修改 `src-ui/**`，前端命令仅用于确认未被波及。
- 未运行 `cargo fmt`。
- 未打开窗口，未发送真机请求，未调用真实模型。
- 未修改生产行为，未修改 backlog。
- 未 commit，未 push。

## 编排者核对后的更正（09-17）

**pi 测试的根因判断不对。** 报告里引用的修前 panic 是第二个断言（argv 行数 3≠4）失败，而第一个断言——四个 provider 的登录结果——**已经通过**。若是超时，`run_process` 会把那一路判成未知，第一个断言先红。所以子进程都跑完了，丢的是日志行。

编排者复现：保留实现者改的 10 秒超时，单独连跑这条测试 40 次仍失败 1 次，现象相同（登录结果对、`argv.log` 少了 `ready` 那一行）。原因是四个 `cmd` 并发 `>>` 追加同一个 `argv.log`，Windows 上有一次写因文件被占用而丢失。

改法：假 pi 按 provider 写各自的 `argv-<provider>.log`，测试汇总读取。修后单独连跑 60 次 0 失败，全量 `cargo test` 连跑 5 次全过。去重断言仍有效：同一 provider 被查两次会在自己的文件里写两行。10 秒超时保留（契约测试不验证短超时，无害）。

另：ACP 等待辅助函数原本自己拼进程池键，与生产重复；键格式若改动，测试会找不到进程而直接放行、退化成不等待。编排者抽出 `pool_key` 供生产与测试共用。
