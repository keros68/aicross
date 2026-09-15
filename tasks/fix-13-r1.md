# fix-13-r1 红绿核对（TASK-13）复审修正轮

依据 GLM-5.3-Flash 与 DeepSeek 对 phase/13 的盲审（`tasks/review-13-glm.md`、`tasks/review-13-ds.md`）。编排者已定的取舍写在各条里。契约只追加。

## 必修（两家共同：1/2/3/5/6/7/9；DeepSeek 独有：4/8/10/11/12；GLM 独有：13/14/15）

1. **回退失败后重放本次改动的结果被 `let _ =` 吞掉，可能留下半回退状态且节点仍 ok（DeepSeek 1 / GLM 1）**：`restore_snapshot_paths` 任一路径失败即 `?` 返回，前面已回退的文件停在回退后内容。修法：恢复改成尽力全量（逐路径收集失败清单继续处理其余路径）；重放失败时把原因写进 `error`，status 改为 `error`（不是 ok，也不是 validation_failed），note「红绿核对恢复本次改动失败：<路径清单>，请手动核对工作区」；结果卡红字。测试：注入某路径只读 → 其余路径仍恢复、status=error、note 列出失败路径。
2. **核对窗口内进程被杀留下回退状态且无记录（DeepSeek 2 / GLM 2）**：核对前先落一个状态文件 `.dispatch/<run>/<node>.redgreen`（含 before_dir、changed 清单、阶段 red/green），核对完成删除；TASK-06 崩溃对账（`reconcile_dispatch_state`）发现该文件时把该节点标「待核对」并在说明里写「上次红绿核对被打断，工作区可能停在回退状态，快照在 <before_dir>」。测试：写入状态文件后跑对账 → 节点待核对且说明含路径。
3. **unpinned 复用「验证命令未通过」文案（DeepSeek 3 / GLM 8）**：unpinned 时验证命令是通过的。等待验收的卡改「测试未钉住，未送审」，验证兄弟/到顶时的 `skipped_reason` 改「上游测试未钉住」；`closeWaitingAcceptance` 加原因入参；同步改 `review-defaults-12.test.ts` 相应断言；术语表加「测试已钉住 / 测试未钉住 / 未核对」「红绿核对」行；设置页「修正轮上限」说明补「测试未钉住也计入」。
4. **恢复失败与「恢复后验证失败」共用一句（DeepSeek 4）**：重放 after 失败时绿跑根本没跑，不能报「恢复后验证未通过」；单独一句「核对恢复本次改动失败：<原因>」（与第 1 条同处）。
5. **超时分支遮蔽恢复后失败（DeepSeek 5 / GLM 5：红跑超时直接 skipped、不再跑绿）**：先判 `green.is_err() && !green_timed_out` → validation_failed；绿跑超时单独「恢复后验证命令超时，未核对」；红跑超时 → skipped。
6. **flaky 验证命令（DeepSeek 6 / GLM 3，编排者裁决）**：绿跑失败时同一命令再跑一次，两次都失败才判 validation_failed；红跑失败一次即可算红（假「已钉住」的代价低于误判 validation_failed）。`pinned_note` 写明跑了几次。
7. **核对占锁与叫停（DeepSeek 7 / GLM 9）**：核对用独立超时 `min(节点超时, 300s)`；进入核对前检查 `is_cancelled` → skipped；红/绿两次跑之间也检查一次。
8. **回退会删掉核对前就存在的空目录（DeepSeek 8）**：`remove_empty_snapshot_parents` 只删本次确实新建出来的目录（快照 `.created` 连带记录新建目录）。
9. **本轮 0 个文件改动被报成「缺少派发前快照」（DeepSeek 9 / GLM 6）**：单独一句「本轮没有文件改动，未做核对」，pinned=skipped。
10. **链接处理一致（DeepSeek 10）**：恢复路径对链接与回退路径同策略（删除后重建并记账）。
11. **测试补课（DeepSeek 11、12）**：补 `DispatchOverlay` 接线断言（`fixRound: isFixRoundExecution(sourceGraph, id)`）；结果卡三态标签抽到 `DispatchBits.tsx` 用 `renderToStaticMarkup` 断言；触发条件负例（`fix_round=false` → pinned None；验证命令空 → 不触发且 frontmatter 无 pinned 值；角色非执行不触发）。
12. **杂项（DeepSeek 13）**：`DispatchRecord.pinned` 加 `skip_serializing_if = "String::is_empty"`；Reply 正文里的 `[红绿核对]` 标题保留但登记进术语表；测试文件头注释补 TASK-13。

13. **红跑「启动失败」被当成测试失败判出假 pinned（GLM 4）**：红阶段只有验证命令真正跑起来并以非零退出才算红；启动失败（找不到命令、shell 起不来）→ skipped + 原因。测试：验证命令指向不存在的程序 → skipped。
14. **超时判定靠错误字符串前缀（GLM 7）**：`run_validation` 返回结构化的 `timed_out` 布尔（或枚举），红绿判定不再匹配「验证命令超时」文本。
15. **杂项（GLM 10、12）**：非 Windows 恢复重建的文件保留权限位（快照记 mode）；Rust 侧 `pinned` 改成三值枚举（serde 成小写字符串），拼错值编译期就挡住。

## 保持

- 触发条件、只用本次快照、只碰本次 diff、worktree 内核对、单写者沿用项目写锁、三态语义与计入上限——核过没问题，不动。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src/server.rs` CRLF；`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-13-R1.md`（逐条对应本清单）。
