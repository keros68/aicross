# REPORT-22-R1 控制台就是对话窗 · 第一期复审修正

## 结果

`fix-22-r1.md` 的 10 条必修已完成。两条经编排者验伪的问题未改：没有在 Rust 中新增续聊参数拼装，也没有改备选作答的 session id 清空逻辑。未启动窗口、未发送真机请求、未调用真实模型，未运行 `cargo fmt`，未 commit、未 push。

## 逐条修正

1. `@` 解析同时识别显示名与机器名，`@kimi`、`@claude`、`@codex`、`@pi` 均可改派；大小写、空白或串尾边界和未知名称原样发送的原契约保持。
2. 还原本期误改的全部历史 `tasks/review-*.md` 与 `tasks/REPORT-*.md`。旧称扫描只检查 `src-ui/src`、`src`、`docs/MANUAL.md` 和 `docs/PRODUCT.md` 的 09-16 当前定案段，不再扫描历史引用和 `tasks/` 归档。
3. 新增真实接线测试：单线请求断言 `runId`、原文 prompt 与 `sessionId`；应用事件断言 tab 匹配、busy/空文本拦截，并经 `dispatchConsoleApply` 调到 `dispatchConsolePreset`。测试使用假 IO 和原生 `EventTarget`，未引入 jsdom。
4. 控制台空态与 `docs/MANUAL.md` 明示聊天当前只读，需要改文件时使用模式派发；`tasks/REPORT-22.md` 同步补充。手册还说明消息与续聊 id 关闭 tab 或重启后不保留。
5. tab 新增 `lastAppliedMessageId` 和 `consoleGraphId`。空输入重复应用同一条历史消息时给系统提示；模式派发复用 `selectConsoleGraph` 选出的图和 TASK-16 的追加材料逻辑，首次落图即把 graph id 记回 tab，旧目标节点在续图派发时跳过。
6. `@` 改派清单优先使用该 tab 消息元信息里同一 agent 最近实际使用的 provider/model；记录不可用时才回退到资源清单第一项。
7. 补全点击改为只替换开头的 `@` 段，已输入正文保留。
8. 回复元信息显示 `fallbackTo` 与 `modelReported`；备选作答时卡片按实际 agent/model 归属显示。
9. 已选 agent 的控制台 tab 标题复用 `commanderTabTitle`，显示“agent · 模型 · 目录名”；启动器态仍显示“控制台”。
10. 右栏聊天空态只在控制台已经选完 agent 后出现，启动器态不再提前显示。

## 必修 1、3、5、7 的红测试

定向命令均为：

```text
cd src-ui && npx vitest run src/__tests__/console-chat-22.test.ts --configLoader native --pool threads -t "<用例名>"
```

- 必修 1：临时去掉机器名候选后，`@kimi` 返回 `recognized: false`、`target: null`，用例 1 failed；恢复后通过。
- 必修 3：临时去掉应用事件的 session id 匹配后，别的 tab 的事件追加了 1 条提示，期望 0 条，用例 1 failed；恢复后通过。另有单线请求形状断言钉住 `runId`、原文 prompt 与续聊 id。
- 必修 5：临时去掉历史消息已应用判断后，实际 `blocked: null`，期望 `already-applied`，用例 1 failed；临时强制 `newGraph` 后，实际图 id 为 `id-4`，期望复用的 `id-2`，用例 1 failed；两处恢复后通过。
- 必修 7：临时恢复整体覆盖草稿后，实际为 `@Kimi Code `，期望 `@Kimi Code 帮我重构这段`，用例 1 failed；恢复后通过。

所有临时故障均已恢复。定向文件最终为 `10 passed (10)`，完整前端测试为 `602 passed (602)`。

## 最终验证

```text
> cargo check --all-targets
Compiling aicross v0.1.0 (D:\work\aicross-p6)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.41s
```

零 warning。

```text
> cargo test
running 466 tests
test result: ok. 455 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 25.86s
```

11 项 ignored 为既有真机、真实模型或本地事实源测试，本轮按要求未启用。

```text
> cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  40 passed (40)
Tests  602 passed (602)
Duration  1.27s
```

```text
> cd src-ui && npm run build
✓ 347 modules transformed.
✓ built in 560ms
```

```text
> python scripts/copy-audit.py
共 6 处命中，5 个文件（含误报，需人工复核）
```

审计退出码为 0。命中均在既有文件 `DispatchOverlay.tsx`、`SettingsModal.tsx`、`console-channel.ts`、`stamp.ts`、`resources/types.ts`；本轮新增或修改的用户文案无命中。

换行复核：`src/server.rs`、`src/main.rs` 和 `CenterPanel.tsx` 保持纯 CRLF；本轮新增 TypeScript 文件保持 LF；`src-ui/src/tauri.ts` 未修改并保持原有混合换行。未运行 `cargo fmt`。

## 未做与工作树说明

- 未做清单列入 backlog 的额度预检、会话持久化和直启辅助函数删除；只在手册说明当前内存状态。
- 未改 `src/dispatch.rs`、各家 resume 参数、留痕格式、控制台通道协议、真实模型路径或原生 TUI。
- 开始与结束时 Git 索引均显示 `src-ui/src/commander/launcher.ts`、`src-ui/src/settings/__tests__/ui-scale-19.test.ts` 为继承的 `UU`；两份工作树文件没有冲突标记，本轮未修改、未 merge、未写索引。
