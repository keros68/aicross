### 一、任务书第 1–5 节要求核对表

实现者报告 [tasks/REPORT-37.md](file:///tasks/REPORT-37.md) 所述内容经与代码逐行核对，第 1–5 节各项要求的落实情况如下：

| 章节与要求 | 结论 | 证据（文件:行号） | 说明 / 核实对照 |
|---|---|---|---|
| **1. 打开方式**：新增 `aicross open <run_id>` 冷启动直达查看页 | **做到** | [src/server.rs:27](file:///src/server.rs#L27), [src/server.rs:139](file:///src/server.rs#L139), [src-ui/src/App.tsx:115-117](file:///src-ui/src/App.tsx#L115-L117) | `AppState::pending_open` 暂存，前端挂载时调用 `take_pending_open` 直达查看页。 |
| **1. 打开方式**：热启动单实例转发切到查看页 | **做到** | [src/server.rs:3898-3906](file:///src/server.rs#L3898-L3906), [src-ui/src/App.tsx:120-125](file:///src-ui/src/App.tsx#L120-L125) | 单实例插件回调中重新解析 argv 并向前端发送 `open-run-request` 事件，前端更新 `viewerRunId`。 |
| **1. 打开方式**：参数解析与 `launch` 并列，不改 `launch` 现有行为 | **做到** | [src/main.rs:194-225](file:///src/main.rs#L194-L225), [src/run_view.rs:31-52](file:///src/run_view.rs#L31-L52) | `parse_open_args` 与 `parse_launch_args` 并列，非 open 请求返回 `NotOpen`，不影响 launch。 |
| **1. 打开方式**：`run_id` 校验与退出码（非法退出 2，找不到 run 退出 1） | **做到** | [src/run_view.rs:48-50](file:///src/run_view.rs#L48-L50), [src/main.rs:200-214](file:///src/main.rs#L200-L214) | 过 `valid_id`，非法参数退出 2；`read_run` 失败退出 1，且未起窗口。 |
| **1. 打开方式**：打开查看页只读，不派发、不改文件 | **做到** | [src/run_view.rs:122-226](file:///src/run_view.rs#L122-L226) | `build_run_view` 仅读取 run 文件与图文件构造视图对象，无任何派发或写盘操作。 |
| **2. 数据**：新增 Tauri 命令 `load_run_view` 读 run 与图文件，带死进程判定，顺序一致，不读 `.dispatch/` 原文 | **做到** | [src/run_view.rs:123-136](file:///src/run_view.rs#L123-L136), [src/run_view.rs:280-282](file:///src/run_view.rs#L280-L282) | 经 TASK-36 的 `read_run` 完成死进程判定，`nodes` 严格按照 `run.nodes` 顺序迭代组装，不访问 `.dispatch/`。 |
| **2. 数据**：running 状态每 2 秒轮询，终态停止 | **做到** | [src-ui/src/viewer/run-view.ts:17-19](file:///src-ui/src/viewer/run-view.ts#L17-L19), [src-ui/src/viewer/RunViewer.tsx:54-55](file:///src-ui/src/viewer/RunViewer.tsx#L54-L55) | `pollIntervalMs` 仅在 `running` 时返回 2000，其余终态返回 `null` 停止后续 `setTimeout`。 |
| **3. 页面**：页头（模式名、run 状态、开始时间、总耗时、失败/拒绝原因、遗留） | **做到** | [src-ui/src/viewer/RunViewer.tsx:119-140](file:///src-ui/src/viewer/RunViewer.tsx#L119-L140) | 元素齐全；`RunViewer.css` 布局完备。模式名推断存在逻辑瑕疵（详见问题清单）。 |
| **3. 页面**：按阶段分组，组内各路并排（最多 3 列） | **做到** | [src-ui/src/viewer/run-view.ts:29-41](file:///src-ui/src/viewer/run-view.ts#L29-L41), [src-ui/src/viewer/RunViewer.tsx:145-147](file:///src-ui/src/viewer/RunViewer.tsx#L145-L147) | 阶段按图深度升序，组内保留摘要顺序；列数上限 3，容器窄于 3×280px 时滚动（REPORT-37 做了说明）。 |
| **3. 页面**：卡片标题（主标题「角色 · 任务」，次标题 `agent/model`，任务名层级回退，厂商色点） | **做到** | [src-ui/src/viewer/run-view.ts:45-57](file:///src-ui/src/viewer/run-view.ts#L45-L57), [src-ui/src/viewer/RunViewer.tsx:177](file:///src-ui/src/viewer/RunViewer.tsx#L177) | 取名顺序：`swarmTitle` → `graphTitle` → `agent/model`；色点调用 `vendorColor`。 |
| **3. 页面**：请求与实际模型（拿不到真身标未报告，不回填请求值，不一致标出） | **做到** | [src/run_view.rs:174-176](file:///src/run_view.rs#L174-L176), [src-ui/src/viewer/run-view.ts:80-94](file:///src-ui/src/viewer/run-view.ts#L80-L94), [src-ui/src/viewer/RunViewer.tsx:187-191](file:///src-ui/src/viewer/RunViewer.tsx#L187-L191) | 真实模型为空时不回填请求值；kimi 提示不报真身；切备选后与备选模型比对真身。 |
| **3. 页面**：状态（quota 恢复时间）、token、耗时、备选切换说明、跳过原因、验收结论 | **做到** | [src-ui/src/viewer/run-view.ts:59-107](file:///src-ui/src/viewer/run-view.ts#L59-L107), [src-ui/src/viewer/run-view.ts:118-129](file:///src-ui/src/viewer/run-view.ts#L118-L129), [src-ui/src/viewer/RunViewer.tsx:183-201](file:///src-ui/src/viewer/RunViewer.tsx#L183-L201) | 各字段展示逻辑完整，异常状态有 pill，常态纯文本；验收/验证节点解析结论。 |
| **3. 页面**：正文 Markdown、产物 diff（等宽可折叠）、worktree 手动合并提示 | **做到** | [src-ui/src/viewer/RunViewer.tsx:202-209](file:///src-ui/src/viewer/RunViewer.tsx#L202-L209) | `ConsoleMarkdown` 渲染，`<details>` 折叠 diff，worktree 路径提示明确。 |
| **3. 页面**：动作只有「采纳 / 取消采纳」，调用 `adopt`（合并写，不整写 run 文件） | **做到** | [src/run_view.rs:286-294](file:///src/run_view.rs#L286-L294), [src-ui/src/viewer/RunViewer.tsx:210-217](file:///src-ui/src/viewer/RunViewer.tsx#L210-L217) | 仅对 `canAdopt` 节点提供采纳/取消采纳按钮；调用底层合并写接口，不动 run 文件。 |
| **3. 页面**：复用现成部件，遵守视觉纪律 | **做到** | [src-ui/src/viewer/run-view.ts:9-11](file:///src-ui/src/viewer/run-view.ts#L9-L11), [src-ui/src/viewer/RunViewer.css:1-302](file:///src-ui/src/viewer/RunViewer.css#L1-L302) | 抽取引流函数，遵循单一 `--accent`、无阴影、厂商小色点、阶梯字号（12/13/14/15/18）。 |
| **4. 契约**：命令行退出码 0/1/2/3 | **做到** | [src/main.rs:200-245](file:///src/main.rs#L200-L245) | 0 已打开、1 找不到 run、2 参数错、3 窗口起不来（引擎根不可用同样按 3）。 |
| **4. 契约**：Tauri 命令 `load_run_view`、`adopt_run_node` | **做到** | [src/server.rs:3947-3948](file:///src/server.rs#L3947-L3948) | 在 Tauri handler 中正确注册。 |
| **4. 契约**：文档更新（`CONSOLE-CHANNEL.md`、`MANUAL.md`） | **做到** | [docs/CONSOLE-CHANNEL.md:146-151](file:///docs/CONSOLE-CHANNEL.md#L146-L151), [docs/MANUAL.md:73-93](file:///docs/MANUAL.md#L73-L93) | 补充命令说明、退出码语义及查看器页面功能介绍。 |
| **5. 测试**：Rust 测试（参数解析、四种状态返回、合并写与不动 run 文件） | **做到** | [src/run_view.rs:392-675](file:///src/run_view.rs#L392-L675) | 8 条测试覆盖 flag 解析、非法 ID、四种 run 状态返回、无图降级、合并写防修改等。 |
| **5. 测试**：前端纯函数测试（分组列顺序、标题顺序、真身规则、quota、验收、轮询停止） | **做到** | [src-ui/src/__tests__/run-viewer-37.test.ts:1-216](file:///src-ui/src/__tests__/run-viewer-37.test.ts#L1-L216) | 13 条单测全部覆盖指定纯逻辑分支。 |
| **5. 测试**：不改引擎行为，报告记录发现问题 | **做到** | [src/engine/mod.rs:469](file:///src/engine/mod.rs#L469), [tasks/REPORT-37.md:124-129](file:///tasks/REPORT-37.md#L124-L129) | 仅放宽 `depth_map` 可见性；验证通过未落图问题如实记录。 |

---

### 二、专项核查结论（针对第 2–7 项）

#### 2. `aicross open` 启动链路与进程行为
- **冷/热启动通道**：冷启动由 `src/main.rs` 经 `AppState::pending_open` 存入，前端在 `App.tsx` 挂载时调用 `take_pending_open` 取走，正确呼出查看页；热启动由单实例插件拦截第二个进程后在首个实例内广播 `open-run-request` 事件，前端监听并更新查看页，链路畅通。
- **退出码 0/1/2/3**：
  - 退出码 0：冷启动起窗正常退出 0；热启动第二个进程在单实例转发后由插件退出 0。
  - 退出码 1：找不到 run 时由 `src/main.rs:213` 打印错误并 `exit(1)`。
  - 退出码 2：参数错误（缺少 run_id、带选项参数 `--xxx`、多个位置参数、非法 ID 字符）由 `src/main.rs:204` 打印错误并 `exit(2)`。
  - 退出码 3：引擎根路径不可用（`src/main.rs:221`）或起窗失败（`src/main.rs:242`）均规范 `exit(3)`。在 Windows 下均调用了 `console_cli::attach_parent_console()` 确保宿主终端能看见报错。
- **防干扰与窗口误启**：
  - 若输入非法参数或找不到 run，`main.rs` 在启动 GUI（`server::start_ui`）之前就已拦截并 `exit`，**绝不会在不该起窗的情况下启动窗口**。
  - 与 `launch` 参数并列判断，无选项重叠漏洞；若同时传入 `open` 与非法 flag（如 `--tool`），会被 `parse_open_args` 判定为非法选项直接拦截退出 2。
  - 热启动时若窗口正处于别的页面，`RunViewer` 作为顶层遮罩（`z-index: 11000`）覆盖全屏，关闭或 Esc 后原工作区状态完好保留，不破坏原有页面。

#### 3. `load_run_view` 与 `adopt_run_node` 数据与边界
- **只读性与不派发**：`load_run_view` 仅读取 run 与图 JSON 文件，没有任何触发流水线派发的逻辑；
- **采纳合并写**：`adopt_run_node` 委托 TASK-36 的 `engine::adopt`，经单节点合并写落盘，测试验证了同图其余节点不丢且 run 摘要文件字节分毫不差；
- **节点顺序与分组**：Rust 端按 `run.nodes` 顺序映射，前端 `groupNodesByStage` 升序排列 stage 且 group 内部通过 `filter` 严格保序；
- **异常与状态返回**：run 缺失返回 Error；图文件缺失优雅降级返回 run 摘要字段；运行中与失败状态均保留状态原因和已沉淀的阶段信息，返回设计合理。

#### 4. 前端展示、生命周期与视觉纪律
- **轮询与定时器**：`pollIntervalMs` 在终态（`done` / `failed` / `rejected`）返回 `null` 停止轮询；切换 run 时前置 `cancelled = true` 避免了慢请求覆盖新页。但在异步 IIFE 事件监听注销上存在小漏洞（详见低度问题）。
- **真身显示**：未拿到真身标「未报告」（kimi 标「kimi 不报真身」），运行中标「—」，严格遵守**不回填请求值**；切备选后以备选模型为基准核对真身并标红不一致。
- **标题取名**：严格按照「蜂群条目标题 → 图标题 → agent/model」的顺序退回。
- **Markdown 安全性**：复用 `ConsoleMarkdown`（未启用 `rehype-raw`，无原始 HTML 解析，链接替换为 `span` 悬停提示，图片替换为替代文本占位），无跨站脚本与导航劫持风险。
- **视觉纪律**：全页无阴影，仅用 1 处 `--accent`，厂商色仅用于 8px 圆点，字号严格落在 12/13/14/15/18 阶梯，符合要求。

#### 5. 模式名推断
- 实现者采用的角色前缀匹配逻辑存在贪婪误判问题，会导致特定自定义任务被错误冠以「共享模式」名称（详见中度问题 1 及稳妥改法建议）。

#### 6. 引擎行为与现有窗口界面
- **未改动引擎行为**：仅放宽了 [`depth_map`](file:///src/engine/mod.rs#L469) 与 [`attach_parent_console`](file:///src/console_cli.rs#L158) 的可见性为 `pub(crate)`，核心执行逻辑零改动。
- **未破坏现有窗口界面**：未删除现有界面的任何代码，仅在 [`src-ui/src/App.tsx#L347`](file:///src-ui/src/App.tsx#L347) 条件挂载 `RunViewer`，可随时通过 Esc /「关闭」退出查看层。

#### 7. 其它数据与正确性问题
- 未发现导致数据丢失、引擎崩溃或流程阻断的严重缺陷。

---

### 三、问题清单

#### 【高严重度】
- **未发现问题**。

---

#### 【中严重度】

##### 1. `infer_preset_name` 贪婪前缀匹配导致多节点任务被误判为「共享模式」
- **问题**：在推断模式名时，代码对预设角色序列做 `roles.iter().take(sequence.len())` 前缀匹配。内置「共享模式」的定义仅为单个角色 `["执行"]`。当用户派发非标准形状的多节点任务（例如没有出方案阶段的 `["执行", "验收"]` 或两路并发的 `["执行", "执行"]`）时，由于前缀匹配中了共享模式的 `["执行"]`，且共享模式是内置预设，该任务会被推断为「共享模式」。
- **证据**：[`src/run_view.rs:248-255`](file:///src/run_view.rs#L248-L255)
  ```rust
  let matched = roles
      .iter()
      .take(sequence.len())
      .zip(sequence.iter())
      .all(|(role, preset_role)| role.trim() == *preset_role);
  ```
- **后果**：页头模式名会把用户实际执行的、包含独立验收甚至改写文件的任务，误标为「共享模式」。根据 [docs/PRODUCT.md](file:///docs/PRODUCT.md) 说明，共享模式被定义为「只读、各方结论不独立、不作验证用」，这一误标会给用户带来严重误导。
- **建议改法**：
  第一版在 run 文件未显式记录 `preset_id` 的前提下，推断策略应当收紧：
  1. 默认仅允许**完全匹配**（`sequence.len() == roles.len() && sequence == roles`）；
  2. 仅对「经济模式」等已知特定流水线，允许在首轮 4 节点之后追加标准修正轮角色（如 `["执行", "验收"]`）时判定为经济模式；
  3. 对于长度大于 1 且未精确命中的任务序列，绝不回退匹配单角色的「共享模式」，直接显示为「自定义模式」。

---

#### 【低严重度】

##### 2. `App.tsx` 中单实例事件监听器在异步 IIFE 中注册存在注销时机竞态
- **问题**：[`src-ui/src/App.tsx:117-126`](file:///src-ui/src/App.tsx#L117-L126) 在 `useEffect` 中使用了异步自执行函数去调用 `listen('open-run-request', ...)`。当组件快速卸载（例如 React StrictMode 在开发环境下的首次卸载，或代码热更新）时，`useEffect` 返回的清理函数执行时 `unlisten` 仍为 `undefined`，导致后续异步 resolve 的监听器无法被注销。
- **证据**：[`src-ui/src/App.tsx:117-126`](file:///src-ui/src/App.tsx#L117-L126)
  ```typescript
  let unlisten: (() => void) | undefined;
  (async () => {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten = await listen<{ runId: string }>('open-run-request', (event) => {
        setViewerRunId(event.payload.runId);
      });
    } catch { /* ... */ }
  })();
  return () => { unlisten?.(); };
  ```
- **后果**：在组件重新挂载或 HMR 后，Tauri 底层事件总线上会残留已失效组件作用域的监听闭包，造成轻微内存泄漏并在收到事件时引发潜在的失效调用。
- **建议改法**：在 effect 作用域中增加 `disposed` 标志位防御：
  ```typescript
  let disposed = false;
  let unlisten: (() => void) | undefined;
  (async () => {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      const fn = await listen<{ runId: string }>('open-run-request', ...);
      if (disposed) { fn(); } else { unlisten = fn; }
    } catch { /* ... */ }
  })();
  return () => {
    disposed = true;
    unlisten?.();
  };
  ```

##### 3. 运行中手动采纳与自动轮询存在状态回跳竞态
- **问题**：在 run 处于 `running` 状态时，每 2 秒一次的 `loadRunView` 轮询与用户手动点击采纳操作并发进行。若用户点击「采纳」后 `adoptRunNode` 成功设置了 `view`（节点为已采纳），但此前发出的慢速 `loadRunView` 在采纳之后才 resolve 返回，未带请求时间戳校验的 `setView(next)` 会在短暂时间内将节点重置回采纳前的未采纳状态。
- **证据**：[`src-ui/src/viewer/RunViewer.tsx:48-61`](file:///src-ui/src/viewer/RunViewer.tsx#L48-L61)（`tick` 内部无请求序列号校验直接 `setView(next)`）与 [`src-ui/src/viewer/RunViewer.tsx:90-101`](file:///src-ui/src/viewer/RunViewer.tsx#L90-L101)（`adopt` 异步修改 `view`）。
- **后果**：运行中采纳卡片时，极端情况下界面可能发生约 1~2 秒的采纳状态回跳闪烁（直到下一次轮询拉到最新图数据才恢复）。
- **建议改法**：在 `RunViewer.tsx` 中为轮询请求增加一个自增序列号 `requestSeqRef`，在 `setView` 前核对若已有更新的采纳请求或新序列响应，则丢弃过期响应。
