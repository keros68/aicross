# REPORT-47 窗口三页：资源、模式、最近的派发

实现者：Claude 子代理（general-purpose）。报告由编排者按其回复落盘。

## 改动文件

- `src-ui/src/App.tsx`：标题栏下加页签行（资源 · 模式 · 最近的派发），默认进「模式」，选中页签经 `config/page-tabs.ts` 记在 localStorage；查看页打开时不显示页签，查看页「关闭」回到「最近的派发」；`aicross open` 的冷启动与 `open-run-request` 事件处理不变。
- `src-ui/src/tauri.ts`：`commands` 加 `configSnapshot`、`resourcesDetect`、`resourcesSmoke`、`configSetDefaultMode`、`configSetSeat`、`configClearSeat`（对应 TASK-46 第 4 节，未传的可选参数传 `null`）；资源类型（snake_case）`ResourceModelEntry`、`ResourceAgentEntry`、`ResourcesOutput`；模式类型 `SeatFallback`、`Seat`（camelCase）、`SeatSource`、`ModeSeatNode`、`ModeEntry`、`ConfigShow`、`ConfigSnapshot`、`DefaultModeResult`。
- `src-ui/src/viewer/Home.css`：`.home` 的 `top` 由 40px 改为 80px，让出页签行。

## 新增文件（`src-ui/src/config/`）

- `page-tabs.ts`：页签定义、默认页签与 localStorage 读写（try/catch，读不到或值不认时回「模式」）。
- `config-view.ts`：展示用纯函数（状态词映射、冒烟按钮条件、实际模型显示条件、人选下拉选项、来源文字、流程图排布、编辑框初值与人选对象互转）。
- `ResourcesPage.tsx`、`ModesPage.tsx`、`ConfigPages.css`。
- `__tests__/config-view.test.ts`、`__tests__/page-tabs.test.ts`。

## 测试

`config-view.test.ts`：人选下拉选项（6 条）、来源文字（2 条）、流程图排布（3 条）、状态词展示（4 条）、编辑框与人选对象（4 条）。`page-tabs.test.ts`：4 条。

## 验证（实现者自报）

- `npx vitest run --configLoader native --pool threads`：6 个文件、48 项通过。
- `npm run build`：通过。
- `npx eslint src/App.tsx src/config src/tauri.ts`：无输出。
- 换行：改动与新增文件全部 LF。`src-ui/src/tauri.ts` 改前即纯 LF（`git ls-files --eol` 为 `i/lf w/lf`），任务书写的「混合换行」与现状不符。

## 未做与理由

- 没有在窗口或浏览器里手动看页面：TASK-46 未合并，命令不存在；浏览器下两页只显示提示。
- 档位没有下拉（任务书只要求人选、思考、备选）；保存时沿用本模式原有 `tier`，没有就存空串。
- 页面组件没有 DOM 测试（任务书只要求纯函数）。

## 修正轮 r1（`tasks/fix-47-r1.md`）

1. 资源页与模式页常驻：`page-tabs.ts` 新增 `keepsMounted(tab)`，`App.tsx` 对常驻页只用 `hidden` 隐藏；打开查看页时常驻页同样只隐藏，「最近的派发」卸载并停止轮询。模式页加 `active` 参数，每次切回重新取 `config_snapshot()`，保留已选模式与位置。新增测试「资源页与模式页常驻，最近的派发切走即卸载」。
2. `SeatEditor` 成功时先 `setSaving(false)` 再 `onSaved(updated)`。
3. `detect` 成功后 `setRowErrors({})`。
4. 次按钮 hover 只用 `var(--bg-hover)`；主按钮 hover 保持 `var(--accent)`。
5. `.cfg-field select` 改为 Sans（`var(--font)`）。
6. 删掉死类名 `cfg-mode-name`。

编排者复跑：vitest 6 个文件 49 项通过，build 通过，改动文件 CRLF 均为 0（与改前一致）。

## 复审后编排者追加（Gemini 复审，结论「修改后可以合并」）

- 中：资源页常驻后切回不刷新。`ResourcesPage` 加 `active` 参数，`App.tsx` 传 `active={visible}`；切回且没有进行中的探测或冒烟时重新取 `config_snapshot()`，成功时清掉旧的加载错误（`busyRef` 记进行中状态）。
- 低：模式页首次加载时显示「正在读取模式设定…」。
- 不改：复审第 2 条「`model.agent` 为空时回退 `agent.id`」，引擎 `aicross resources` 的每个模型都带 `agent` 字段（取自资源库 `ResourceModel.agent`）。
- 页面组件没有 DOM 测试设施，这两处没有补自动化测试，留到窗口真跑核对。复跑：vitest 49 项通过，build 通过，eslint 无输出。

## 不确定处

- `resources_smoke` 给了 model 时返回该 agent 那一项而不是 `{ agents }`，类型写成两者之一；页面不用返回值，结束后重新取快照。
- Tauri 命令参数名按 `agent`、`model`、`provider`、`mode`、`stage`、`index`、`seat` 传，需与 TASK-46 对齐。
