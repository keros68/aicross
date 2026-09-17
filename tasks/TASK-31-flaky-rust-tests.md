# TASK-31 Rust 测试并发下偶发失败（09-17）

对应 backlog 93。**只做这一件事。** 不碰 `src-ui/**`。

## 现象（编排者 09-17 实测）

main 合并后连跑三次全量 `cargo test`：
1. `dispatch::tests::run_one_dead_acp_falls_back_once_without_resume_and_rewrites_full_task` 失败；
2. `resources::tests::task17_pi_login_check_uses_safe_args_once_and_ignores_credentials` 失败；
3. 473 全过。

两条单独跑都通过（前者连跑 3 次都过）。当时机器负载偏高（dev 窗口 + Vite + 其它工作树构建，全量耗时 31–39 秒，平常约 25 秒）。

## 要做

1. **先复现并定量**：全量 `cargo test` 默认并发连跑至少 10 次、`cargo test -- --test-threads=1` 连跑至少 5 次，记录每次失败的测试名与 panic 信息。若默认并发复现不出，可以人为加压（例如同时跑一个 CPU 占满的进程）再测。报告里给失败率表。
2. **找根因**，重点排查：测试间共享的进程级状态（`std::env::set_var` / PATH / 当前目录 / 固定的临时文件或目录名 / 全局单例或缓存）、依赖墙钟的超时或 `sleep` 等待、子进程 stub（python 脚本）启动慢导致超时。上面两条和同类测试都要看，不要只修这两条表面。
3. **修**：共享状态改为每个测试独立（唯一临时目录、通过参数注入而不是改全局环境、必要时用同一把测试锁串行化确实要改全局环境的测试）；超时类放宽到负载下稳定、或改成等事件而不是等时间。不改生产行为；若发现是生产代码本身有竞态，停下在报告里写清证据，不要顺手改。
4. 修完后同样的连跑次数再跑一遍，给出修后失败率。

## 验证

`cargo check --all-targets` 零 warning；上面的连跑统计（修前 / 修后）；`cd src-ui && npx vitest run --configLoader native --pool threads` 与 `npm run build` 各跑一次确认没被波及。

保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF，其它 Rust LF）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-31.md`。backlog 不用改。
