# REPORT-fullcheck-ui（10-08 全面检查，界面）

范围：只改 `src-ui/**`；未改 Rust，未提交，未加依赖，LF。

## 没做完与限制

- 未在真实窗口核对：C7 弹层定位、C5 下拉操作、C3 保存与刷新交错、C6 标红均未在窗口里看过。
- 没有组件级测试：项目无 jsdom / testing-library，加了违反「不加依赖」；只测组件调用的纯函数，`ModelPicker.tsx`、`ModesPage.tsx` 的接线只经 tsc 与 eslint。
- C4 的 `presetRemove` 带 host：不成立，未改（证据见 C4）。
- C1：按任务书不改，由引擎侧修。

## 逐条

### C2（P1）卸载确认路径与实际删除路径不一致：成立，已修
- 证据：`src/skill_install.rs` 的 `uninstall_at` 只按 host 与 scope 算写入位置并删除，不看确认框里的路径；界面对每个 `kind === 'ours'` 的位置都给卸载按钮。
- 改法：`skill-view.ts` 的 `canUninstall(host, location)` 要求「我们装的且是写入位置」（`isWriteTarget`），旧引擎没有 `writeTarget` 时按路径判断；`SkillPage.tsx` 传 host，其它位置不再显示卸载。
- 测试：`skill-view.test.ts` 新增两处都有安装标记的用例（含旧引擎）：`.codex/skills`（非写入位置）不可卸载，`.agents/skills`（写入位置）可卸载。修前失败、修后通过。

### C3（P1）保存结果被刷新丢弃：成立；第一轮只修了一半，第二轮补全
- 第一轮修了：刷新与写入交错时写入结果被丢弃、旧读取落地。
- 第一轮漏了：写入之间仍可并行（设默认与人选写入可同时在途），后回的旧整份 `result.modes` 能盖掉先回的新人选；写完后的重读期间没有锁编辑；自定义模式保存、删除没走写入协调。复审指出后在第二轮修正，见文末「第二轮」。
- 证据：`ModesPage.tsx` 读写共用一个 `snapshotSeq`，写入期间开始的刷新会让写入结果被丢弃，刷新可能显示旧配置。
- 改法：新增 `config/snapshot-sync.ts`（`createSnapshotSync`）：写入进行中读取等待，期间多次刷新合并为写完后的一次读取；写入开始时仍在途的读取作废，调用方拿写完后重读的结果；每个写入结果都应用（即使中间又开始了别的写入）；写入失败照样抛出并触发重读。`ModesPage.tsx` 的所有读取（顶栏刷新、切回页面、保存 / 删除自定义模式后重读）走 `sync.read`；保存 / 清除人选、思考、备选、视角、数量（经 `submitSeat`）、设默认、一键推荐走 `sync.write`。
- 测试：`snapshot-sync.test.ts` 6 条（模块写之前先写测试）：旧读取与成功写入交错；写入中刷新等待且只读一次；先发后回的写入仍应用并重读；写入失败后重读；无写入时后发读取胜出；读取失败。

### C4（P1）自定义模式保存不传 host：成立，已修；presetRemove 不需要 host
- 证据：`src/server.rs` 的 `preset_save(host: Option<String>, preset)` 解析 host，前端只发 `preset`；`preset_remove(id: String)` 只收 id，`preset_remove_at` 跨所有主 agent 清理。
- 改法：`tauri.ts` 的 `presetSave(host, preset)` 发 `{ host, preset }`；`PresetEditor` 加 `host` 属性，`ModesPage` 传当前主 agent；`presetRemove` 不变。
- 测试：新增 `src/__tests__/tauri-commands.test.ts`（假的 `window.__TAURI_INTERNALS__.invoke`）：`preset_save` 收到 `{ host: 'codex', preset }`（修前失败）；`preset_remove` 只收 `{ id }`（修前已通过，记录契约）。

### C5（P2）搜索后重开、回车可能清除人选：成立，已修
- 证据：`toggle` 清空查询后，仍在按旧查询过滤的列表里找当前值的序号；搜索时当前人选可能是第 0 项，重开后第 0 项是「清除人选」，回车即清除。
- 改法：`picker.ts` 新增 `pickerItems(options, query, extra)`（组件的键盘顺序用它）与 `openActive(options, extra, value)`（按空查询的列表定位；没有当前值时取第一个可选项），`toggle` 用后者。备选下拉同一组件，一并覆盖。
- 测试：`picker.test.ts` 新增 3 条，含「搜索使当前人选成为第 0 项 → 重开 → 选中项仍是当前人选而非第 0 项」。

### C6（P2）忽略后端的 `effective.invalid`：成立，已修
- 证据：`src/console_cli.rs` 的 `mode_entry` 输出 `"invalid": seat_invalid_reason(target)`，前端类型没有这个字段。
- 改法：`tauri.ts` 的 `EffectiveSeat` 加 `invalid?: string | null`；`config-view.ts` 新增 `seatRuleViolation`，`seatInvalidState` 优先用它（位置标红、不计入「已配好」、计入「待处理位置」）；`seatRowState` 加 `reason`，下拉标记为「不能放在此位置」；`ModesPage.tsx` 在位置行下用错误色显示完整原因。
- 测试：`config-view.test.ts` 新增 1 条（人选违规、备选违规、`invalid` 为 null / 空 / 缺失）；两处 `seatRowState` 的 `toEqual` 补 `reason: ''`。

### C7（P2）弹层被滚动容器裁切：成立；第一轮只修了打开时的定位，第二轮补全（均未做真实布局核对）
- 第一轮修了：打开时按页面正文等裁切祖先的可见范围算方向与列表高度。
- 第一轮漏了：位置只在打开时算一次，展开后滚动正文或缩放窗口，弹层仍会被裁切。第二轮改为展开期间外部滚动或窗口缩放即关闭，见文末「第二轮」。
- 证据：定位按 `window.innerHeight`，弹层在 `overflow-y: auto` 的 `.page-body` 里。
- 改法：`picker.ts` 新增 `visibleBounds(clips, viewportHeight)`（窗口与所有裁切祖先的交集）；`pickerPlacement` 第三个参数改为 `{ top, bottom }`；`ModelPicker` 打开时向上找所有 overflow 非 visible 的祖先并以其为界。没有改成 portal + 固定定位。
- 测试：原 4 条 `pickerPlacement` 改传窗口边界、数值不变；新增 2 条（正文起点 120 / 200、交集计算）。

### C8（P2）下拉过滤与「可用模型」统计口径不一：成立，已修
- 证据：「冒烟超时」（显示为「连接超时」）、「额度用尽」可选；「可用模型」统计还含这些与「未验证」。
- 改法：`config-view.ts` 统一可选状态集合（可用、已验证，`isSelectableState`），`seatOptions` 的 disabled、`optionsWithCurrent` 过滤、「可用模型」统计都用它；当前人选状态不在集合里时照常列出（禁用）。
- 测试：新增一条覆盖连接超时、额度用尽、未验证；两条已有测试随之改（「未验证」改为禁用；资源「可用模型」从 3 改为 2）。

## 验证（`src-ui/`）
- `npx tsc --noEmit -p .` 通过（另跑 `-p tsconfig.app.json` 通过，`tsconfig.json` 只含 references）。
- `npx eslint src` 通过。
- `npx vitest run` 219 通过（原 202）。
- 修前先跑新增与改动的测试：19 条失败，`snapshot-sync.test.ts` 因模块不存在无法加载；`preset_remove` 用例修前已通过。

## 改动文件
- 新增：`src-ui/src/config/snapshot-sync.ts`、`src-ui/src/config/__tests__/snapshot-sync.test.ts`、`src-ui/src/__tests__/tauri-commands.test.ts`
- 逻辑：`config-view.ts`、`picker.ts`、`skill-view.ts`
- 组件：`ModesPage.tsx`、`ModelPicker.tsx`、`PresetEditor.tsx`、`SkillPage.tsx`
- `src-ui/src/tauri.ts`
- 测试：`config-view.test.ts`、`picker.test.ts`、`skill-view.test.ts`

## 第二轮

来源：Codex（gpt-6.1-sol）复审，结论「修后合并」，1 条 P1、2 条 P2。范围与规矩同第一轮。

### 没做完与限制
- 仍未在真实窗口核对：写入排队与编辑锁定、自定义模式保存 / 删除、弹层随滚动 / 缩放关闭，都只有纯函数测试与 tsc / eslint，没有在窗口里操作过。
- 弹层关闭用的是捕获阶段的 `scroll` 监听：弹层内部（模型列表）滚动不关闭；如果键盘上下移动时 `scrollIntoView` 带动了页面正文滚动，弹层也会关闭。按打开时的定位，列表在可见范围内，正常不会出现，未实测。

### 1（P1）迟回的写入响应覆盖新配置：成立，已修
- 证据：第一轮 `snapshot-sync` 的写入各自直接执行，设默认（返回整份 `modes`）与人选写入可同时在途；设默认后回时整份旧 `modes` 盖掉人选写入的结果。`editing` 也没算设默认，重读期间不锁。
- 改法：`snapshot-sync.ts` 的写入排队，前一个返回（成功或失败）后才发出下一个，结果按发出顺序落地；新增 `busy` 回调，从第一个写入开始到全部写完后的重读结束（成功或失败）为 true。`ModesPage.tsx` 用它得到 `syncBusy`，`editing` 改为 `syncBusy || seatBusy || defaultBusy || recommendBusy || removingPreset || countBusy !== '' || countingImpact`，设默认、视角、数量、推荐、人选、新建 / 编辑 / 删除模式都随之锁定。重读失败时 `loadError` 照常显示、忙碌结束，可用顶栏刷新重读。
- 测试（`snapshot-sync.test.ts`）：「写入排队：后发写入等前一个返回才发出」；「后发写入不会先返回：旧的整份响应先落地，新人选随后落地，重读延迟期间一直忙」；「重读失败交给 fail 并结束忙碌，之后可手动刷新」；「还没读过快照时写完即结束忙碌」。实现前 4 条失败，实现后通过。

### 2（P2）自定义模式保存、删除绕过写入协调：成立，已修
- 证据：第一轮编辑器直接调 `commands.presetSave`，删除直接调 `commands.presetRemove`，之后靠 `reloadKey` 触发读取；期间发起的读取不受协调。
- 改法：去掉 `reloadKey`。`PresetEditor` 的 `onSaved` 改为 `onSave(preset)`，由 `ModesPage.savePreset` 经 `sync.write` 调 `presetSave(hostId, preset)`，结果里的 `modes` 先落地，写完统一重读；失败抛回编辑器显示。删除经 `sync.write` 调 `presetRemove`，写完统一重读。`PresetEditor` 不再直接调命令，第一轮加的 `host` 属性随之去掉（host 在模式页传给 `presetSave`）。
- 测试：`snapshot-sync.test.ts` 新增「自定义模式保存 / 删除期间发起的读取不落地旧快照」。这条在实现前即通过：它测的是第一轮已有的协调行为，本轮的修正是把保存与删除接到这个入口上，接线由 tsc / eslint 保证。

### 3（P2）下拉展开后滚动或缩放仍被裁切：成立，已修
- 改法：`picker.ts` 新增 `closesOnViewportChange(type, insidePicker)`：窗口缩放关闭；滚动发生在弹层外（页面正文等）关闭，弹层内部不关。`ModelPicker.tsx` 展开期间在 window 上监听 `scroll`（捕获阶段，收得到祖先滚动）与 `resize`，按它关闭弹层，收起时移除监听。
- 测试：`picker.test.ts` 新增 2 条（缩放关闭；外部滚动关闭、内部滚动不关）。实现前因函数不存在失败，实现后通过。

### 4 报告
- 第一轮 C3、C7 的结论改为如实描述（第一轮修了什么、漏了什么）。

### 验证（`src-ui/`）
- `npx tsc --noEmit -p .` 通过；`npx tsc --noEmit -p tsconfig.app.json` 通过。
- `npx eslint src` 通过。
- `npx vitest run` 225 通过（第一轮后 219）。

### 改动文件
- `src-ui/src/config/snapshot-sync.ts`、`picker.ts`
- `src-ui/src/config/ModesPage.tsx`、`PresetEditor.tsx`、`ModelPicker.tsx`
- 测试：`src-ui/src/config/__tests__/snapshot-sync.test.ts`、`picker.test.ts`

## 第三轮

来源：Codex 第二轮复审，1 条 P1。

### （P1）保存进行中已展开的下拉仍可提交旧草稿：成立，已修
- 证据：`ModelPicker.tsx` 的 `disabled` 只禁用触发按钮；已展开的弹层不收起，选项按钮只看 `option.disabled`，`choose` 不看组件的 `disabled`。展开下拉后 Tab 到同行思考下拉改值（保存进行中、编辑锁定），再 Shift+Tab 回弹层按回车，会带着旧的 `draft.thinking` 再发一次保存，覆盖刚保存的值。
- 改法：`picker.ts` 新增 `pickerShown(open, pickerDisabled)` 与 `pickerAccepts(pickerDisabled, itemDisabled)`。`ModelPicker.tsx`：被禁用时在渲染期间把 `open` 置为 false（弹层收起，重新启用后不会自己弹出）；`choose`（回车与点击共用）按 `pickerAccepts` 拒绝禁用期间的提交；额外项与各选项按钮在禁用期间也设 `disabled`；外部点击、滚动、缩放的监听只在弹层显示时挂上。
- 测试：`picker.test.ts`「下拉被禁用时（fix-fullcheck 第三轮）」2 条：「已展开的弹层在禁用后不再显示」「禁用期间不接受选择（回车或点击），未禁用时只拒绝禁用项」。这两条与实现同一步写入，没有先单独跑出失败；组件接线只经 tsc / eslint，未在真实窗口复现原问题。
- 验证（`src-ui/`）：`npx tsc --noEmit -p .`、`-p tsconfig.app.json`、`npx eslint src` 通过；`npx vitest run` 227 通过（第二轮后 225）。
