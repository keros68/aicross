# TASK-40 修正轮 R1（10-01）

依据：`tasks/review-40-claude.md`（Claude 子代理，C）、`tasks/review-40-agy.md`（Antigravity Gemini，G）。标「已核」的条目编排者已对照代码确认。范围与「必须保持」仍按 `tasks/TASK-40-window-retire.md`。不 commit，不 push。完成后在 `tasks/REPORT-40.md` 末尾追加「R1」一节：逐条写改了什么、对应测试名；并把报告里与事实不符的说法改正（换行核对按文件逐个写；`platform.ts` 实际留了什么）。

## 必修

**R1-1 macOS 编译失败**（C 高-1，已核）
`src/server.rs` 的 `.run(|_app_handle, event| …)` 改了参数名，但只在 macOS 编译的 Reopen 分支仍用 `app_handle.get_webview_window("main")`。参数名改回 `app_handle`，非 macOS 分支用 `let _ = &app_handle;` 消掉未使用警告。用 `cargo check --target x86_64-apple-darwin` 若本机没装该 target 无法验证，就在报告里写明「已按代码核对，未能在本机编译 macOS」。

**R1-2 无窗口时 ACP 权限请求当场拒绝**（G 中-1，已核）
`src/channels/acp.rs` 的 `ask_user_permission` 登记挂起后一直等到本轮 deadline；窗口删掉后没有任何代码会回答（只有测试辅助 `test_answer_pending`）。改为：生产路径没有回答方，执行命令、写文件、MCP 调用这类需要授权的请求当场按拒绝回复，不挂起等待；读类请求原有的自动放行不变。测试用的回答机制保留在 `#[cfg(test)]` 下，现有测试 `interactive_turn_hands_execute_permission_to_the_user_and_honours_the_answer` 按新行为改写为「需要授权的请求立即被拒、不等 deadline」，并补一条断言耗时远小于 deadline。

**R1-3 首页的状态要做死进程判定**（C 中-1，已核同口径问题）
`list_runs` 直接取 run 文件里的 `status`；`aicross status` 与查看页经 `engine::read_run` 会把进程已死的 running 判为失败。首页用只读的同口径判定（复用引擎里判断 run 是否仍在运行的函数，不写盘），显示与查看页一致。补测试：running 但 pid 已死 → 首页显示失败。

**R1-4 被误删的测试恢复**（C 中-3）
下列测试测的是保留下来的代码，改写成调用保留函数的版本恢复（可为 `detect_all_resources_at` 加一个可注入 pi 路径的内部函数；原测试里只针对已删函数的断言去掉，其余保留）：
- 资源探测：`task17_pi_detect_list_auth_merge_persists_without_auth_payload`（探测后资源库不落 `credentials` / `access_token`）、`task17_missing_pi_detection_clears_stale_login`、`task17_pi_login_probe_runs_before_store_lock`；
- 额度：`task14_fallback_order_roundtrips_and_quota_expiry_is_cleared` 中 `mark_quota_exhausted_at` 与过期清理的断言；
- 派发核心：`r3_8`（空项目时 `dispatch_stage_core` 报错且不写盘）、`r3_16`（worktree 节点的 `successful_validation_output` 提示）、`task07`（`DispatchNodeResult` 的 `cacheRead` 序列化）；
- 前端：`parseAcceptance` 的边界用例（取最后一个判定、只看尾部 1200 字符）补进 `viewer/` 的测试。
原测试位置见 C 中-3（`git show 5e978a5:<路径>` 可取原文）。

**R1-5 文档删掉已删除功能的描述**（C 中-2、低-6，G 低-3）
- `docs/MANUAL.md`：删掉「核对 / 待核对」术语与崩溃后核对入口、「脱敏计数」那句；`STATE.md` 写明「引擎仍会写入，当前没有命令读取」；`resources smoke` 的用法补上必填的 `<agent>`。
- `docs/CONSOLE-CHANNEL.md`：同步 `STATE.md` 的说法。
- `docs/PRODUCT.md`「机制」第 6 条：去掉「查看器里只读展示」，改为「资源状态由 `aicross resources` 输出」。
- `NOTICE`：只改 aicross 自己写的段落（上游那段逐字不动）——去掉已删的图标文件与「terminal handling, workspace, session」等已不存在的来源说明。
- `src-ui/src/styles/global.css` 关于字体来源的注释、`src/engine/golden.rs` 开头指向已删前端测试的注释，改成现状（后者注明「前端生成器已删，原文见 git 历史与夹具 README」）。

## 应修

| 编号 | 问题 | 来源 |
|---|---|---|
| R1-6 | `aicross propose` 现在会拉起窗口：识别为已删除命令，stderr 提示「propose 已删除，建议在宿主对话里给出」并退出 2 | C 低-1 |
| R1-7 | 会进节点错误或用户可见的旧文案改为现行说法：`acp_setup.rs` 的原因文案（不再提聊天、/ 命令、权限卡片、重启本应用）、`dispatch.rs` 的「左栏工作区」提示；源码注释里指向「接入页」「界面」「左栏」的说法同步改 | C 低-2 |
| R1-8 | `src-ui/package-lock.json` 与 `README.md` 恢复原来的 CRLF 换行 | C 低-3 |
| R1-9 | 删除残留：根目录 `skills/`（确认 NOTICE 无对应条目）、`scripts/` 下只服务已删 hook 与图标主题的脚本、`platform.ts` 中无人引用的导出、`dispatch.rs` 中引擎恒为 `None` 的 `AppHandle` 进度事件管道与不再注册的 `#[tauri::command]` 标注 | C 低-4 |
| R1-10 | 首页轮询：窗口不可见时暂停；每次只读一次 presets；按文件修改时间粗排，只解析前 50 个 | C 低-5、G 低-2 |
| R1-11 | `run-list.ts` 的状态文案函数改为从 `viewer/run-view.ts` 引入，不复制 | C 低-7 |

## 不在本轮（进积压）

- ACP 适配器不再在窗口启动时预装，只在 `__run` 里懒触发、结果可能来不及记录（C 低-2）：是否改为由命令行显式安装，另起。
- `STATE.md` 当前无人读取：是否在命令行补崩溃后核对入口，另起。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
```

R1-2、R1-3 的新测试须能在修复临时改回时变红（编排者会抽查）。
