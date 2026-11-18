# REPORT-89 桌面端补派发设置入口

任务书：`tasks/TASK-89-dispatch-settings-ui.md`。

## 改了什么

### 后端（Rust）

- `src/console_cli.rs`
  - 从 `run_config_set` 抽出 `config_set_at(root, patch, acp)`：接受一组「键 → JSON 值」，逐键走 `normalize_setting_value`（数字键沿用 `clamp_setting_value` 的引擎区间夹取，越界与取整分别说明；布尔、文本键校验类型），与现有文件合并后整份原子写入；开启 `acpEnabled` 且结果为开时当场安装适配器，结果与退出码带出。CLI 的 stdout / stderr / 退出码不变。
  - `AcpOps` 与其 `production` 改为 `pub(crate)` 供窗口命令使用。
  - 新增三个测试：多键合并写入与夹取说明、未知键与类型错误整批不写入、ACP 分支带出报告且装不上时 exit 1 但设置保留。
- `src/server.rs`
  - 新窗口命令 `dispatch_settings`：返回 `{ dispatch, path, acp }`（设置、设置文件路径、适配器状态），不经主 agent 解析；适配器状态查询放阻塞线程。
  - 新窗口命令 `dispatch_save(patch)`：走 `config_set_at`，返回 `{ dispatch, note, acp? }`；设置已写入、仅适配器装不上时仍算成功，说明带回前端。

### 前端（src-ui）

- `src/tauri.ts`：新增 `DispatchSettings` / `AcpAdapterReport` / `DispatchSettingsView` / `DispatchSaveResult` 类型；`ConfigShow.dispatch` 从 `unknown` 改为 `DispatchSettings`；新增 `dispatchSettings` / `dispatchSave` 命令封装。
- `src/config/settings-view.ts`（新）：字段定义（分三节：上限 / 超时 / 其它，共 12 项）、草稿初值与分钟换算（`scaleText`，除不尽保留三位小数、换算回秒不产生假改动）、数值校验（`numberInputError`，上限按显示单位、下限按存储单位说明）、补丁只带改动的键（`settingsPatch`）、适配器状态行（`acpStatusText`）。
- `src/config/SettingsDialog.tsx`（新）：顶栏「设置」打开的对话框。改动收进草稿，点「保存」一次写入；保存后重读设置，带说明（夹取 / ACP 安装结果）时留在对话框里显示，否则关闭；Esc、点背景、取消均可关；底部注明设置文件路径。
- `src/App.tsx`：顶栏刷新按钮右侧加「设置」图标按钮（仅窗口内显示），打开对话框。
- `src/config/ConfigPages.css`：对话框加宽与各控件行样式。
- 测试：`src/config/__tests__/settings-view.test.ts`（新，含「字段区间镜像引擎」的守恒测试）；`src/__tests__/tauri-commands.test.ts` 补两条命令参数断言。

### 文档

- `docs/MANUAL.md`：派发设置一节补窗口入口（同一份文件、同一套夹取）；ACP 一节补对话框开关；术语表加「派发设置」一行。

## 修了哪些不方便

1. 派发设置只能命令行改 → 对话框一页改完（主诉求：并发上限现在桌面端可改）。
2. 合法区间只有代码知道 → 分段控件（并发 1–3、在飞 1–3、修正轮 0–3、蜂群 1–4）与数值输入的上下限说明；区间镜像引擎并有测试守恒。
3. 超时以秒为单位不直观 → 按分钟显示与填写，保存换算回秒；命令行设过分钟以下的秒数（如 30 秒）照常显示与保存，不算非法。
4. 设置文件位置不可见 → 对话框底部显示实际路径（含 `AICROSS_HOME` 覆盖时的位置）。
5. `ConfigShow.dispatch` 无类型 → 补全类型。

## 怎么验证的

- `cargo check`：`Finished 'dev' profile [unoptimized + debuginfo] target(s) in 5.16s`
- `cargo test`：`test result: ok. 829 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out`（含新增 `task89_config_set_at_*` 三个测试，单跑确认通过）
- `cd src-ui && npm test`：`Test Files 25 passed (25)` / `Tests 277 passed (277)`
- `cd src-ui && npm run lint`：无输出（通过）
- `cd src-ui && npm run build`：`✓ built in 175ms`
- `python scripts/copy-audit.py`：命中 16 处全部为既有文件（StartPage 第二人称等），新文件零命中

## 没做什么

- 没放宽并发上限 3：夹取逻辑与上限没动，只补了入口。
- 没给 `dispatch-settings.json` 加进程间写锁：窗口与 `aicross config set` 同时写的窗口极小，与现状一致。
- 设置文件损坏时不自动覆盖：`dispatch_save` 会失败并原样显示「dispatch-settings.json 解析失败」（与命令行一致），需手工删掉或修好该文件。
- `ccSwitchPath`、`classificationModel`、`classifyProposals` 仍在命令行改，不进对话框。
- 没跑 `tauri dev` 开真窗口点一遍：两个新命令是薄封装（实现体 `config_set_at` 有单测覆盖，`load_settings` 为既有路径），窗口内的实际交互未做人工走查。

## 不确定处

- `dispatch_save` 的 `patch` 参数用 `serde_json::Map<String, Value>` 接收 JS 对象，按 Tauri v2 的 serde 反序列化应无问题，但没有真窗口验证过参数透传；若有问题会表现为保存时报参数错误，改动只在这一处。
- 分钟显示对命令行设的奇数秒（如 610 秒）显示为「10.167」：换算精确但观感一般，实际几乎不会有这种值。
