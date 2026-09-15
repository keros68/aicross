# REPORT-10-R1 总指挥与快速启动复审修正

对应 `tasks/fix-10-r1.md`。10 条必修均已完成。

## 逐条修正

1. 旧总指挥 tab 重启回写过期默认
   - 新增 `CONSUME_COMMANDER_SETTINGS`。PTY 首次启动并成功保存默认后，清除该 tab 的待保存快照；后续换目录或 `RESTART_TERMINAL` 不再注册保存回调。
   - 测试：`TASK-10-R1 保存后消费 commanderSettings，随后重启不恢复旧快照`；`保存成功消费快照后，组件重启路径不再生成 saveCommander 回调`。

2. 其它已装 CLI 的新开入口
   - 启动器次级入口新增可展开的「其它 CLI」，从原有内置 AI CLI 清单中过滤出已安装且不属于 Claude Code、Codex、Kimi Code CLI、pi 的工具。
   - 点击仍走 `selectTool` / `SET_TERMINAL_TOOL`，不附加总指挥模型参数；未改标题栏「+」菜单。
   - 测试：`其它 CLI 只列已安装的非总指挥工具，点击沿原工具路径转发`；`有其它已装 CLI 时出现第三项与工具名`。

3. 首屏假空态
   - 工具探测增加 `scanned` 状态。首次结果返回前显示「探测中…」，不显示未安装空态和默认一键行；结果为空后才显示未探测到工具。
   - 探测结果通过 `commander/tool-scan.ts` 在 CenterPanel 与 Explorer 间共享；可用总指挥仍只认 `installed[tool] === true`。
   - 测试：`未扫描显示探测中；扫描完成且为空才显示没探测到`。

4. 模型与 provider 白名单
   - 前端启动参数和启动动作都要求非空标识符满足 ASCII 白名单，且不能以 `-` 开头；空串仍表示不指定。非法值不 trim、不启动，启动器显示「模型 id 含非法字符」或「provider id 含非法字符」。
   - Rust `save_commander_in` 使用同一规则，拒绝把非法默认写入文件。
   - 测试：`模型与 provider 非法值整项拒绝，不 trim 或当成旗标传出`，逐项覆盖空格、`&`、`-` 开头、纯空白及非法 provider；Rust 往返测试同时断言拒写且原文件不变。

5. 参数追加顺序
   - 保持工具设置 `extra_args` 在前、本次 `commanderArgs` 在后；重复旗标以本次启动为准。
   - `src/server.rs` 抽出 `append_per_launch_args`，同时锁住空串过滤；`tasks/TASK-10-commander.md` 与 `docs/PRODUCT.md` 已改成相同口径。
   - 测试：`server::tests::task10_r1_per_launch_args_follow_config_and_drop_empty_values`。

6. 空白启动器 tab 标题
   - 空白 graph session 未进入对话图时显示「总指挥」；从次级入口显式进入后显示「对话图」。已有图仍优先显示图标题。
   - 测试：`空白 tab 只有显式进入对话图后才显示“对话图”标题`。

7. 目录行按钮检查安装状态
   - Explorer 使用与启动器相同的默认有效性与工具探测快照。探测未完成、默认无效或默认 agent 未安装时，不显示「在此启动总指挥」。
   - 测试：`目录快捷入口只在扫描完成且默认 CLI 已安装时可用`。

8. 启动器细节
   - 空模型提示增加 `modelsLoaded` 条件；用户点击 agent 卡时立即置位 `seeded`，异步默认不会覆盖选择；`cwdName` 对 Windows 盘符根目录和 Unix 根目录分别返回盘符与 `/`。
   - 测试：`资源库空模型提示必须等模型清单加载完成`；`用户点 agent 卡立即锁定初值，晚到默认不能再 seed`；`目录名取末级，尾斜杠不算一级`（含 `C:\`、`D:/`、`/`）。

9. 测试缺口
   - 模型 select 抽成可独立渲染的 `CommanderModelSelect`，选中值与 option 都复用固定的 `{ provider, model }` 序列化顺序。
   - `commanderAgentOfTool` 已用于「其它 CLI」生产过滤；Rust 写侧拒绝 version 现在读回断言文件未变。
   - 测试：`picked 与 option 共用固定 key 顺序，select 能选中 provider/model 往返值`；`task10_commander_roundtrip_and_missing_field_defaults` 的 version 拒写读回断言。

10. 损坏 commander.json 拒写
    - `save_commander_in` 在目标文件存在时先解析；损坏内容拒绝覆盖，读侧仍按空默认返回。
    - 测试：`task10_commander_roundtrip_and_missing_field_defaults` 的损坏文件保存失败及原文不变断言。

## 验证

- `cargo check --all-targets`：通过，零 warning。
- `cargo test`：通过；325 passed，0 failed，11 ignored。
- `cd src-ui && npx vitest run --configLoader native --pool threads`：通过；27 个测试文件、390 个测试全部通过。
- `cd src-ui && npm run build`：通过；TypeScript 编译通过，Vite 处理 333 个模块并完成生产构建。

## 没做 / 部分做

- 无未完成或部分完成的必修项。
- 按任务书接受目录被删除时标题在 OSC7 回报前可能与后端回退目录短暂不一致。
- 未修改 `src/channels/`、`src/dispatch.rs`、`src/terminal.rs`；未修改 `src-ui/src/tauri.ts`，未运行 `cargo fmt`。
- 未启动桌面窗口，未发送真实 CLI 或模型请求。
- 未 commit，未 push。
