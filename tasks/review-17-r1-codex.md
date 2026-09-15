可交付但有未闭合项：9 条修正均已落实；Rust 测试本轮因 `target\debug\.cargo-build-lock` 拒绝访问未能复跑。

1. 已修：解析与端到端落盘断言完整。只读 `status`，`ready` 仅兼容；假 pi 输出含 `credentials/reason`，落盘 JSON 明确断言不含。依据：`src/resources.rs:1137-1193`、`src/resources.rs:2950-3044`、`src/resources.rs:3064-3101`。

2. 已修：G1 的 `logged_out` 与额度分区互斥；引擎按完整资源键去重，并在执行备选前跳过 `logged_out`，写入 `fallback_skipped` 与说明。依据：`src-ui/src/components/center/graph/DispatchOverlay.tsx:139-158`、`src/dispatch.rs:3289-3338`、`src/dispatch.rs:3369-3376`、`src/dispatch.rs:7459-7496`。

3. 已修：混合 provider 保留正常组，组内未检查模型单独显示状态。依据：`src-ui/src/resources/types.ts:275-303`、`src-ui/src/resources/__tests__/resources-login-17.test.tsx:53-70`。

4. 已修：pi 登录检查在获取资源锁之前执行，结束后重新读取并加锁写入；并发行为有测试。依据：`src/resources.rs:2285-2310`、`src/resources.rs:2320-2354`、`src/resources.rs:3143-3195`。

5. 已修：pi 缺失时返回错误提示，并将对应模型状态写为 `unknown`、更新时间；前端显示提示。依据：`src/resources.rs:2334-2353`、`src-ui/src/resources/types.ts:299-303`、`src/resources/__tests__/resources-login-17.test.tsx:126-138`。

6. 已修：全量探测未发现 pi 时清除原有登录状态，统一写为 `unknown`。依据：`src/resources.rs:856-877`、`src/resources.rs:3125-3139`。

7. 已修：折叠入口和重新检查按钮均有 `:focus-visible` 的 2px accent 轮廓。依据：`src-ui/src/components/resources/resources.css:419-453`、`src/resources/__tests__/resources-login-17.test.tsx:210-214`。

8. 已修：测试不再使用 `set_var/remove_var`，改用测试目录内的 `argv.log`；G1 行为函数验证同时覆盖登录与额度冲突场景。依据：`src/resources.rs:2950-3023`、`src-ui/src/resources/__tests__/resources-login-17.test.tsx:141-183`、`src-ui/src/components/center/graph/DispatchOverlay.tsx:163-165`。

9. 已修：`REPORT-17.md` 已同步 `status` 解析、当前测试文件和新增测试。依据：`tasks/REPORT-17.md:10-35`、`src-ui/src/resources/__tests__/resources-login-17.test.tsx:223-230`。

新发现的问题（按严重度）：

- 中：Rust 测试本轮无法复跑，错误为 `failed to open target\debug\.cargo-build-lock: 拒绝访问`。这是当前验证环境限制，不是源码测试失败。
- 低：`tasks/REPORT-17-R1.md:95` 仍写“未 commit”，但当前 `HEAD` 已是 `febca39`，该报告状态描述已过时。

核过没问题的点：

- `status: ready/not_ready` 优先，未知 `status` 不会被误判为未登录。
- 未使用 `--credentials`，也未将 stdout/stderr 原文写入资源库。
- 同一 provider 只检查一次，最大并发为 4。
- 旧资源文件缺少登录字段时默认为 `unknown`，非法值也降为 `unknown`。
- 全量探测和单 provider 重检查均会持久化登录状态及检查时间。
- 前端专项测试：33 个文件、509 项全部通过。
- 工作区当前无未提交改动。