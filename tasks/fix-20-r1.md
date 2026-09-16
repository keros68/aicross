# fix-20-r1 派发建议条（TASK-20）复审修正轮

依据 DeepSeek 与 GLM-5.3-Flash 对 `console/proposal` 的盲审（`tasks/review-20-ds.md`、`tasks/review-20-glm.md`），以及编排者自己的实测。DeepSeek 判**不可交付**，GLM 判「可交付但有未闭合项」；两家都把第 1、2 条列为最高severity。契约只追加。

**动手前先做一件事**：本分支基于 `1f7882b`，main 已前进（其中 `b20edbc` 新增了 `src-ui/src/lib/focus-enforcer.ts`，第 3 条要用它）。先 `git merge main` 到本分支，解完冲突再改。

## 编排者先定的产品裁决（照这个改，不要另行发挥）

建议条的交互按下面三条定死，**文案必须与真正生效的键一致**：

1. **鼠标路径永远存在**：建议条上加两个可点按钮「采纳」「忽略」，模式名本身可点（点一次换下一档，与 Tab 同序）。现在整条 `div` 没有任何 `onClick`，控制台 tab 里等于没有出路——这是底线，不是可选项。
2. **终端有焦点时只认 `Ctrl+Enter`**：`Esc` 与 `Tab` 一律放行给 PTY。在活着的 codex 会话里吞掉 Esc（打断生成）和 Tab（补全）是真实损失，不做。此时文案写「Ctrl+Enter 采纳 · 或点右侧按钮」，不出现 Esc / Tab 字样。
3. **焦点在任何输入控件里时，一个键都不抢**：`INPUT` / `TEXTAREA` / contenteditable 全部排除（**单行 `input` 也算**，这正是第 2 条缺陷的来源）。只有焦点不在任何输入里时，才认「回车采纳 / Esc 忽略 / Tab 换模式」。

## 必修

1. **控制台 tab 里建议条完全无法裁决（DS 1 / GLM 1 / 编排者已实测）**：xterm 的输入承载就是 `<textarea class="xterm-helper-textarea">`，PTY 有焦点时 `isPtyTarget` 与 `isMultilineTarget` **同时为真**，`proposalShortcutAction` 第一行 `targetMultiline` 早退，于是 Enter / Ctrl+Enter / Esc / Tab **一个都不归建议条**，而文案偏偏显示「Ctrl+Enter 采纳」。
   编排者实测（临时用例，两条都红）：`{key:'Enter',ctrlKey:true,targetMultiline:true}` + `{commander:true,ptyFocused:true}` → `null`；`{key:'Escape',targetMultiline:true}` 同样 → `null`。
   **注意两家审查在这里的描述都不完全对**：GLM 说「PTY 聚焦时 Esc 被建议条抢走、codex 收不到」，以及它「核过没问题」里写的「PTY 聚焦时纯回车还给终端、Ctrl+Enter 采纳且文案同步」——这两句都与实测不符，真实情况是四个键全死。按上面的裁决 2 改：PTY 分支要先于 `targetMultiline` 判定，且只放行 `Ctrl+Enter`。
2. **对话图输入框按回车 = 静默起一次真派发（DS 2 / GLM 2，最严重）**：捕获阶段的 `document` 监听 + `stopPropagation()` 把回车掐在到达 `graph-input` 之前，`sendInColumn` 根本不触发；用户以为在发消息，实际执行了采纳、真的开跑并花额度、可能写文件，草稿还留在框里像没发出去。按裁决 3 改：单行 `input` 一并排除。这条直接违反「永不自动执行」的定案，必须有一条能红的用例钉住。
3. **建议条吞掉弹窗的 Esc，把今早刚修好的「Esc 能关设置」又打回去了（GLM 3）**：`SettingsModal` 的 Esc 监听挂在 document 冒泡阶段，建议条在捕获阶段 `stopPropagation` 直接短路它。复现：有建议条时打开设置 → 按 Esc → 设置不关，建议反而被忽略。修法：建议条的 handler 在页面上存在打开的模态弹层时直接返回——**复用 main 上的 `src-ui/src/lib/focus-enforcer.ts` 里那条 `[aria-modal="true"]` 判定**，不要另写一套选择器。确认派发页等其它 document 级 Esc 处理同理。
4. **propose 超时后留下无法裁决的幽灵建议（DS 3 / GLM 5）**：`PROPOSAL_TIMEOUT` 25 秒，而开启二次判断时分类是一次真模型调用（按 `settings.timeoutSecs`，默认 600 秒），常常超时；此时 CLI 已拿到 503、Rust 里没有这条 run，前端却把建议条画了出来，采纳时 `consoleRunUpdate` 抛「找不到 run」被 `await` 中断——不派发、不报错、也清不掉，`void operation.finally(...)` 还留下 unhandled rejection。修法：**先 `await` 回执成功再 `publishProposals`**；采纳/忽略路径加 `try/catch`，失败时把该建议本地移除并给一句可见提示；二次判断的调用超时单独收窄，不要用派发的 600 秒档。
5. **采纳撞在飞上限时静默失败（GLM 4 / DS 3 同源）**：已有 3 路在飞时 Rust 返回「同时在飞 run 已达上限 3」，前端 `await` 抛出后建议条原地不动、没有任何提示。修法：catch 后在条上显示原因（「在飞已满，稍后再采纳」），并补测试（见第 8 条）。
6. **「派发」按钮的本地建议绕开在飞上限、也不进 run 表（DS 4 / GLM 7）**：`localIo` 把 `listConsoleRuns` 打桩成空数组、`consoleRunUpdate` 打桩成空操作，于是本地建议采纳后既不受前端 `consoleRunLimit` 也不受 Rust `INFLIGHT_LIMIT` 约束，`aicross status/cancel` 看不到、停不了，界面「N 路在飞」也不计，连点连发能起任意条真派发。修法：本地建议也要在 run 表里登记（新增一个只做登记的 Tauri 命令，契约只追加），采纳走与通道建议完全相同的上限检查，不要用打桩 io 绕开。
7. **未裁决建议会变成看不见也清不掉的幽灵（DS 5 / GLM 6、8）**：(a) 同项目两条建议时 `proposalForTab` 只返回第一条，旧的既不显示也没入口；(b) 目标项目没有打开的 tab 时建议只在 Rust 表里躺着，界面完全不出现；(c) webview 重载后前端列表丢了、Rust 里仍是 `proposed`，而 `aicross cancel` 对 `proposed` 走 `should_relay_cancel == false`，返回 200 却什么都不做；(d) 两条建议并发发布时 `publishProposals` 整表替换会互相挤掉。修法：`cancel` 对 `proposed` 执行 `dismissed`；前端启动/`list_console_runs` 时把遗留 `proposed` 重新装配或翻成 `dismissed`；`publishProposals` 按 id 合并而不是整表替换；同项目多条时在条上标「还有 N 条」。
8. **测试（DS 7 / GLM 4）**：
   - (a) `proposed_run_transitions_only_to_running_or_dismissed` 末段是同义反复——插 10 条 proposed + 3 条 running 后断言 running 计数为 3，无论建议是否占上限都会通过。改成：先塞满 `INFLIGHT_LIMIT` 条 running，再把一条 proposed 改成 running，期望 `Err`（409）。
   - (b) 前端整套用例没有一格把「从事件目标算出的两个标志」一起过一遍，这正是第 1 条漏网的原因。**本仓 vitest 跑在 node 环境、没有 jsdom，不要为此引入 jsdom**：把「从事件目标算出 `targetMultiline` / `ptyFocused`」这一步也抽成可测的纯函数，用假 target 覆盖四格——xterm helper（`TEXTAREA` + 该 class）、单行 `input`、普通多行 `textarea`、普通 `div`，每格断言最终动作。
   - (c) 第 2、3 条各补一条能红的用例（单行输入回车不采纳；有 modal 弹层时建议条不吃键）。
9. **归类模型的判据过宽，失败与否决共用一句话（DS 6）**：`/\bDISPATCH\b/i.test(answer.text)` 是全文包含，模型复述规则（「应输出 DISPATCH 或 SINGLE，本次为 SINGLE」）就会被判成该派——开这个开关的目的正是少误触发。修法：只取回复里第一个 `DISPATCH|SINGLE` 词判定；「分类失败，已按不派处理」与「归类模型判定不必派发」分成两句；用户点名的分类模型不在资源列表时给提示，不要静默换成 `models[0]`。
10. **同屏两个「派发」按钮（DS 9）**：工具条的「派发」开确认屏真派，输入框旁的「派发」只生成建议，同名不同行为。按 AGENTS.md「一个概念一个词」，把**新增的那个**改成「建议派发」，工具条保持「派发」不动。另修 `targets` 生成的建议在用户已有自定义模式时被 `presetDisplayName` 拼成「自定义模式 · 自定义模式」：`name` 传空串或节点来源名。
11. **二次判断每跑一次留一个 run 目录（DS 10）**：`runId: proposal-classify-<uuid>` 会在 dispatch 数据根下不断新建 `runs/…/dispatch/STATE.md` 且永不清理。改成固定 runId 复用同一目录，或调用结束后清掉。项目目录没被污染（`writeProjectDispatch:false` 两家都已核），只是自身数据目录会涨。
12. **文档小出入（GLM 9）**：`reason` 挂在共享的 `ConsoleDispatchRequest` 上，`/v1/dispatch` 也照收而文档只在 propose 下写它——要么给 dispatch 明确写「忽略该字段」，要么拆成独立请求体；`docs/PRODUCT.md:287` 写的入口是 `aicross dispatch --propose`，实现与任务书都是 `aicross propose`，同步过来。

## 说明（不用改）

- `src-ui/src/__tests__/tmp-orch-check.test.ts`（DS 8）是**编排者的核对探针**，用来实测第 1 条是否真失效，审查时恰好还在树上，**现已删除**。不要去找它、也不要加回来；它的内容按第 8(b) 条并进正式用例即可。

## 保持

- `/v1/propose` 只登记不派发、Rust 薄中继（不拼提示词/不写图/不碰 `.dispatch`/不调模型）、`proposed` 不计在飞上限、状态机方向闭合（`proposed` 只转 `running`/`dismissed`，`dismissed` 终态，终态拒绝迟到的 `running`）、CLI 与 `dispatch` 同口径退出码、多行只走 `--text-file`、`--reason` 单行校验、小任务判据同源、二次判断默认关且失败方向安全、隐藏 tab 用 `offsetParent === null` 守卫、`docs/CONSOLE-CHANNEL.md` 与实现一致——两家都核过没问题，不动。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF，其它 Rust LF，`CenterPanel.tsx` 原有 CRLF，`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-20-R1.md`，逐条对应本清单。
