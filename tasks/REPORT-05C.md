# REPORT-05C — 第 5 期 C 节（打包与体验）

工作树 `D:\work\aicross-ui`，分支 `ui/phase5c`，从 main `0b18601` 起。
只做 TASK-05 的 C 节；A 节（ACP）、B 节（第 2 期预设）在别的工作树。
按任务书第 0 节第 3 条的口径执行，没动 `src/channels/`、`src/dispatch.rs`、
`src-ui/src/graph/`，也没动 `DispatchOverlay.tsx`。
未 commit、未 push、未写任何安装位、未起 dev 窗口、未发真机模型请求。

## 做了什么

### 1. 打包与图标

- `tauri build` 在本机出包成功（原文见「验证」）。修的配置问题只有一处：
  `tauri.conf.json` 的 `beforeBuildCommand` 原本是空串，`tauri build` 不会
  先构前端，打进安装包的是上一次 `src-ui/dist` 的旧产物。改成
  `{ "script": "npm run build", "cwd": "./src-ui" }`，与已有的
  `beforeDevCommand` 同形。bundle 配置和图标路径本来就是对的，没改。
- 图标换成正式的「AC」字标：深底（`#14171c`，不用纯黑）浅字（`#f1f3f6`），
  22% 圆角方块，几何无衬线，A 的横杠压低并做细以保住 32px 下的字腔，
  C 按惯例做 4% 圆形过冲。没有杯子。
- 重写 `scripts/generate-aicross-icons.mjs`：原来用 5×7 点阵字模画字母，
  64px 以上全是锯齿。现在是 SDF 光栅器（4×4 超采样）+ 手写 zlib/PNG 编码，
  仍然零第三方依赖。它产出 `icons/icon-master.png`（1024），以及
  `tauri icon` 不生成、但 `tauri.conf.json` 引用的 `256x256.png` 与
  `512x512.png`。
- 全套图标用官方工具生成，落在本仓库的图标目录 `icons/`（本仓库没有
  `src-tauri/`，`tauri.conf.json` 和 `Cargo.toml` 都在根目录）：
  `npx --yes -p @tauri-apps/cli@2 tauri icon icons/icon-master.png -o icons`
  更新 54 个文件（PNG / ICO / ICNS + Square*/StoreLogo + android/ios）。
- 同一套几何另存两份矢量：`icons/icon-master.svg`（上游
  `scripts/rebuild-icons.py` 的输入）与 `src-ui/public/favicon.svg`；
  三处互相有注释指路。

### 2. 首启引导

- 新增 `src-ui/src/components/center/graph/first-run.ts`（纯函数）与
  `FirstRunGuide.tsx`（只负责画）。
- `shouldShowFirstRun({graphCount, resourceCount, folderPath})`：
  `graphCount === 0 && resourceCount === 0` 才为真；任一为 `null`
  （还在读、或读资源报错）一律不显示，避免先闪一下再消失。
- 三步，每步一句话，点了跳到对应位置：
  - 打开目录 → 系统目录选择器，选完 `dispatch({type:'SET_FOLDER'})`；
  - 接入资源 → `openResourcesPage()`（`TitleBar` 监听并打开接入页覆盖层）；
  - 选预设 → 右栏收起时先 `TOGGLE_RIGHT_PANEL`，再发
    `aicross:open-orchestration`（`Compiler.tsx` 监听并切到「方案与编排」，
    预设下拉就在那儿）。
- 挂在 `GraphTab` 的开始视图（`graphId === ''`）里，纯追加渲染。
  `GraphTab` 另加了一个 `modelsRead` 标记：`models` 初值是 `[]`，
  没有这个标记会把「还没读完」当成「一个资源都没有」。
- 可访问性按 `web-design-guidelines` 对过：`<ol>` + `<button type="button">`、
  `:focus-visible` 描边、`min-height: 44px`、`touch-action: manipulation`、
  序号 `aria-hidden`（顺序由 `<ol>` 承载）、已完成状态带文字不只靠颜色。

### 3. 右栏「任务」tab

任务书写「现在是占位」，实际不是：这个 tab 已经是上游 Coffee CLI 带来的
完整 TaskBoard（849 行），`{id, title, description, status}`、点一下循环推进
（待办 → 进行中 → 完成）、拖动排序都在，数据也已经落 `~/.aicross/tasks.json`。
所以本期不重写，只补任务书点名要的两件事：

- **纯函数化 + 测试**：新增 `src-ui/src/components/right/task-reducer.ts`，
  把散在 `setTasks(prev => ...)` 里的 12 处变更原样搬出来
  （`addTask` / `removeTask` / `cycleStatus` / `setTaskStatus` / `patchTask` /
  `renameCategory` / `duplicateTask` / `moveTask` / `parseTasks`），
  `TaskBoard.tsx` 改成调用它们。行为不变，最长的那段拖放重排是逐字搬的。
- **原子写 + 损坏文件不覆盖**（`src/server.rs`）：
  - `save_tasks` 走新的 `write_tasks_at`：先校验是 JSON **数组**，再写同目录
    临时文件 + rename 覆盖。半截内容不可能落到 `tasks.json` 上。
  - `load_tasks` 走 `read_tasks_at`：文件不存在 = 空板返回 `"[]"`；
    文件在但读不出 = 报错，绝不悄悄返回 `"[]"`。
  - 前端 `parseTasks` 解析失败返回 `null` → `TaskBoard` 置 `loadDamaged`，
    本次不载入，并且**整个会话都不再写盘**，界面给一条说明（含下一步：
    修好或移走文件再重开窗口）。原路径是 `JSON.parse` 抛错 → 落回
    localStorage → 下一次 effect 就把空数组写回去，损坏文件被清掉。
  - 多窗口同步的 `tasks-changed` 回调也换成 `parseTasks`，坏载荷直接忽略。
- 与派发无耦合：没有引用 `graph/` 或 `dispatch`。

### 4. 前端拆包

改前四个 chunk 超 500 kB：main 1253 kB、emacs-lisp 780 kB、cpp 626 kB、
wasm 622 kB。改后最大 chunk 是 xterm 465.79 kB，警告消失。三处改动：

- `src-ui/src/lib/shiki.ts`
  - 从 `shiki` 根入口换成 `shiki/core` + 显式主题 import。根入口带着全部
    ~600 个语法和主题的注册表，整包都会被打进来。
  - `highlighter.loadLanguage(someString)` 换成显式的 `LANG_LOADERS` 表
    （31 条，与既有 `LANG_MAP` 对齐）。原来的动态字符串 import 让打包器只能
    为每个语法各产一个 chunk，包括 emacs-lisp、wolfram 这些永远用不到的。
    不在表里的围栏语言退回纯文本，和不认识的扩展名一个待遇。
  - Oniguruma WASM 从 `shiki/wasm`（base64 的 JS 模块，622 kB）换成
    `shiki/onig.wasm?url` + `fetch`，变成真正的 .wasm 静态资源，不再是 JS
    chunk。引擎本身没换（没改成 JS 正则引擎），高亮结果应当与原来一致。
- `src-ui/vite.config.ts`：用 rolldown 原生的
  `rollupOptions.output.codeSplitting.groups`。Rollup 的 `manualChunks` 在这版
  vite 的兼容层里只对已经自成 chunk 的模块生效，共享语法还是被内联进去。
  分出 `xterm` / `markdown` / `react` / `vendor`，外加「每个语法文件一个
  chunk」的 name 函数。`includeDependenciesRecursively: false` 是关键：
  默认开启时一个 group 会连带吞掉依赖，cpp 因此把 cpp-macro 吞进去
  （660 kB）、graphql 把整个 JS/TS 家族吞进去（727 kB）。
- 三个按需组件改 `React.lazy` + `<Suspense fallback={null}>`：
  `SettingsModal`（App.tsx，外层用 `state.settingsOpen` 把关；该组件内部所有
  effect 除版本号外都已 gate 在 `open` 上，延后挂载不改变行为）、
  `ResourcesOverlay`（TitleBar.tsx）、`SkillsPanel`（CenterPanel.tsx）。
  三处原本就是条件渲染。

安装包因此也瘦了：MSI 6.24 MB → 5.08 MB，NSIS 5.33 MB → 4.17 MB。

### 5. `docs/MANUAL.md`

用户手册第一版，6 节：安装 / 接入资源 / 三个内置预设 / `.dispatch/` 是什么 /
密钥怎么处理 / 首次上手。说明体、每节 ≤ 10 行（实测 4/7/7/6/10/5）、
不用 emoji、不拟人化。预设那节按 `src/orchestration.rs::builtin_presets()`
的真实定义写（共享模式一路执行；经济模式高档位出方案 → 低档位执行 →
第三家盲验收；盲审模式三路盲）。`.dispatch/` 一节按 `src/dispatch.rs` 的
实际行为写（一次调用一个 Markdown、YAML 头、`STATE.md`、自带 `*` 的
`.gitignore`、关掉后改写 `~/.aicross/runs/`）。密钥一节是六铁律的用户版，
并写明 aicross 自己不读 `~/.pi/auth.json` 和 `~/.cc-switch/*.db`。

## 没做什么

- 没碰 `src/channels/`、`src/dispatch.rs`、`src-ui/src/graph/` 的逻辑，
  也没碰 `DispatchOverlay.tsx`。
- 没重写 TaskBoard 的 UI、拖放交互或便签/提示词两个视图 —— 只把变更逻辑搬进
  纯函数，行为保持原样。
- 没改 `TierTerminal.tsx` 和 `Explorer.tsx` 里那两个 24px 的行内「AC」小标记
  （`<text>` + monospace）。它们是界面元素不是应用图标，改动会和别的工作树
  撞文件，留给后面统一。
- 没动 `scripts/rebuild-icons.py`（上游的 resvg 管线，本机没有 resvg）。
  只保证它的输入 `icons/icon-master.svg` 还在，且换成了新几何。
- 新界面文案沿用第 2 期做法，中文硬编码，没进 11 语言 i18n 字典。
- 没 commit、没 push、没写安装位、没起 dev 窗口、没发真机模型请求。

## 验证

### cargo check（零 warning）

```
$ cargo check
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.46s

$ cargo check --tests
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.09s

$ cargo check --tests 2>&1 | grep -c -i warning
0
```

### cargo test

```
$ cargo test
...
test server::tasks_tests::missing_file_reads_as_an_empty_board ... ok
test server::tasks_tests::a_damaged_file_is_returned_verbatim_rather_than_as_an_empty_board ... ok
test server::tasks_tests::invalid_payload_never_replaces_a_good_file ... ok
test server::tasks_tests::write_then_read_round_trips_and_leaves_no_temp_file ... ok

test result: ok. 179 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 3.99s
```

本期新增 4 条（改前 175）。

### npx vitest run

```
$ cd src-ui && npx vitest run
 RUN  v5.0.0 D:/work/aicross-ui/src-ui

 Test Files  13 passed (13)
      Tests  148 passed (148)
   Start at  11:36:38
   Duration  473ms (transform 78%, import 13%, tests 6%, worker 2%)
```

本期新增 2 个文件 24 条（改前 11 文件 124 条）：

- `src-ui/src/components/right/__tests__/task-reducer.test.ts`（15 条）：
  增删、状态循环、直接置状态与字段写入、分类改名、复制、拖放重排 5 种情形
  （同区重排 / 跨区改状态 / 落在分区标题 / 目标分区为空 / 空目标与未知 id
  的 no-op）、`parseTasks` 4 组（正常、空板、未知状态回落、8 种坏输入）。
- `src-ui/src/components/center/graph/__tests__/first-run.test.ts`（9 条）：
  显示判定 5 条（该显示 / 有图不显示 / 有资源不显示 / 任一未知不显示 /
  与工作目录无关）、步骤表 3 条、渲染 2 条。

### npm run build（chunk 警告消失）

```
$ cd src-ui && npm run build
...
dist/assets/lang-cpp-macro-CgwvTOCX.js   224.24 kB | gzip:  18.05 kB
dist/assets/lang-cpp-BAHFRj0j.js         402.02 kB | gzip:  31.11 kB
dist/assets/main-eGNoQRYR.js             420.08 kB | gzip: 209.44 kB
dist/assets/xterm-_r9IGi5_.js            465.79 kB | gzip: 123.12 kB
✓ built in 839ms
```

输出里不再有 `(!) Some chunks are larger than 500 kB after minification`。
最大 chunk 465.79 kB。对照：

| chunk | 改前 | 改后 |
|---|---|---|
| main | 1253.06 kB | 420.08 kB |
| xterm | （在 main 里） | 465.79 kB |
| lang-cpp | 626.13 kB | 402.02 kB |
| lang-cpp-macro | （在 cpp 里） | 224.24 kB |
| wasm（oniguruma） | 622.32 kB | 变成 .wasm 静态资源，不再是 chunk |
| emacs-lisp / wolfram / 其余未用语法 | 780 / 262 kB … | 不再产出 |

`dist/index.html` 的 `<link rel="modulepreload">` 是 11 条（运行时 + tauri +
markdown + vendor + xterm + react + 几个共享 app chunk），没有语法 chunk ——
语法仍然是打开 diff 时才拉。

### npm run tauri build

```
$ npx --yes -p @tauri-apps/cli@2 tauri build
...
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `release` profile [optimized] target(s) in 3m 18s
       Built application at: D:\work\aicross-ui\target\release\aicross.exe
     Running light to produce D:\work\aicross-ui\target\release\bundle\msi\aicross_0.1.0_x64_en-US.msi
     Running makensis to produce D:\work\aicross-ui\target\release\bundle\nsis\aicross_0.1.0_x64-setup.exe
    Finished 2 bundles at:
        D:\work\aicross-ui\target\release\bundle\msi\aicross_0.1.0_x64_en-US.msi
        D:\work\aicross-ui\target\release\bundle\nsis\aicross_0.1.0_x64-setup.exe

EXIT=0
```

产物：

```
target/release/bundle/msi/aicross_0.1.0_x64_en-US.msi     5,083,136 bytes
target/release/bundle/nsis/aicross_0.1.0_x64-setup.exe    4,169,916 bytes
```

## 不确定处

1. **没有窗口手测。** 任务书禁止起 dev 窗口，所以首启引导的三步跳转、
   任务 tab 的拖放、以及 diff 语法高亮都只有单测和构建产物做依据。
   建议编排者手测这三项，尤其是**打开一个 diff 看高亮还在不在** —— shiki
   从 `shiki` 根入口换到 `shiki/core`、WASM 从 JS 模块换成 `?url` 资源，
   这条路径单测覆盖不到。
2. **`includeDependenciesRecursively: false` 的取舍。** rolldown 文档建议同时
   开 `strictExecutionOrder: true`。实测开了以后每个语法 chunk 都被提升进
   入口的静态预载集，`index.html` 冒出 55 条 modulepreload，启动时把 45 个
   语法全下一遍 —— 比不拆还糟。所以没开，理由写进了 vite.config 的注释：
   语法是纯数据模块、没有副作用，这条顺序保证在这里买不到东西。
   以后若给 `codeSplitting.groups` 加了有副作用的库分组，要重新评估。
3. **`shiki` 的子路径 import。** 用的是 `shiki/langs/*.mjs`、
   `shiki/themes/*.mjs`、`shiki/onig.wasm`，都在 `shiki` 包自己的 exports 里
   （没有新增 `@shikijs/langs` 这类传递依赖），但不是 shiki 文档主推的写法，
   升级 shiki 大版本时要复核路径还在不在。
4. **图标在 16px 的表现。** ICO 里最小是 16px，那个尺寸下 A 的字腔会糊成实心。
   所有字母标在 16px 都这样，不算缺陷，但任务栏观感请自己看一眼。
5. **行尾。** `src/server.rs` 和 `src-ui/src/components/center/CenterPanel.tsx`
   在仓库里是 CRLF/LF 混排，任何整文件重写都会把少数派翻面、把 diff 撑成几千
   行。这两个文件最后是按字节打的补丁；`git diff --ignore-cr-at-eol --numstat`
   与普通 `--numstat` 完全一致，确认没有行尾噪声。其余改动文件同样核过。
6. **`docs/MANUAL.md` 用 LF**，而同目录的 `PRODUCT.md` 是 CRLF。新建源码文件
   在本仓库是 LF，就跟了源码的惯例；要统一 docs/ 风格请直接改。
7. **安装包未签名，也未安装试运行**，只验证了能出包。

---

## 修正轮 R1（GLM 盲审回执）

盲审提的 5 条全部采纳，无「不采纳」。改法与对应测试如下。

### R1-1 手册把脱敏范围说大了（高）

原文写「替换 `sk-` 开头的串、URL 里的内嵌凭据和 Bearer 头」。核对
`src/dispatch.rs::redaction_patterns()`，实际只有六条：PEM 私钥块、
`Authorization: Bearer`、`password|passwd|pwd` 赋值、
`token|api_key|secret_key|access_key` 赋值、`AKIA[0-9A-Z]{16}`、
以及 `postgres|mysql|mongodb` 三种协议的连接串内嵌凭据。既没有通配 `sk-`
前缀（那条正则在 `resources.rs::safe_summary()` 里，只管错误摘要，不管派发
正文），也不覆盖 `https://user:token@host`。

**改法**（选推荐方案，不动派发代码）：`docs/MANUAL.md` 第 5 条改成「默认过一
遍脱敏，见下」，另起一节「脱敏认什么」逐条列出真实六类，并明确点出两种**认
不出来**的形态（裸写 `sk-…`、`https://用户:令牌@主机`），末尾写「规则以
`src/dispatch.rs` 的 `redaction_patterns()` 为准」。同时把原来那句「数字异常
高就查材料」改成「数字为 0 不等于材料干净」—— 原措辞会让人把 0 当安全信号。
拆成两节后每节仍 ≤ 10 行（密钥 8 行、脱敏 7 行）。

**测试**：文档改动，无测试。逐条对照过 `redaction_patterns()` 的六个正则。

### R1-2 loadTasks 被拒时回落成可写空板（高）

`TaskBoard.tsx` 原 `loadTasksFromBackend()` 在 `commands.loadTasks()` reject 时
（Rust 侧「文件存在但读不出」）落进 localStorage 分支，返回 `[]`，
`isLoadedRef` 置 true —— 下一次任何改动都会把空数组写回
`~/.aicross/tasks.json`。上一轮只挡住了「读得出但解析不了」，没挡住「读不出」。

**改法**：新增 `src-ui/src/components/right/task-storage.ts`，把载入流程抽成
依赖注入的 `loadTaskBoard(io: TaskStorageIo)`，返回
`{kind:'ok', tasks} | {kind:'damaged', reason:'unreadable'|'unparsable'}`。
Tauri 下 reject → `damaged/unreadable`，**不碰 localStorage**；解析失败 →
`damaged/unparsable`；localStorage 回退只留给非 Tauri（浏览器里跑前端，那里
没有需要保护的用户文件）。`TaskBoard` 的 `loadDamaged` 从 boolean 变成
`TaskLoadDamage | null`，两种原因给两句不同的提示（读不出 → 查权限或占用；
不认识 → 修好或移走），`isLoadedRef` 两种情况都保持 false，整个会话不写盘。

**测试**：`src-ui/src/components/right/__tests__/task-storage.test.ts`，8 条。
关键那条是
`loadTaskBoard > stops writing instead of falling back when Rust refuses the read`
—— 断言结果是 `damaged/unreadable`，且 `readLegacy`、`saveTasks` 一次都没被
调用。其余：`reads the board the Rust side returns`、
`treats an empty file as an empty, writable board`、
`stops writing when the file is there but unparsable`、
`migrates legacy localStorage tasks into the empty Rust file once`、
`survives a failed migration with an empty board rather than a damage flag`、
`uses localStorage only outside the app`、
`outside the app, a broken localStorage value is just an empty board`。

### R1-3「选预设」展开右栏时事件丢失（高）

`GraphTab` 原来是 `dispatch(TOGGLE_RIGHT_PANEL)` 之后同步
`window.dispatchEvent(...)`，此刻 `RightPanel` 还没 mount，没人监听，事件丢；
老用户会停在 localStorage 记着的上一个 tab（「任务」）。

**改法**：新增 `src-ui/src/lib/open-orchestration.ts`，事件之外再留一个一次性
标记。`requestOrchestrationPanel()` 置标记 + 发事件；`RightPanel` 的
`useState` 初始化里调 `initialRightTab(saved, consumeOrchestrationRequest())`，
待办请求优先于记忆值；已挂载的 `RightPanel` 在事件回调里也调一次
`consumeOrchestrationRequest()`，免得标记留到下次重新 mount 又莫名跳转。
没有用 `setTimeout(0)`/rAF —— 一次性标记不依赖时序，面板什么时候 mount 都对。
`GraphTab` 的「选预设」改调 `requestOrchestrationPanel()`；
`Compiler.tsx` 的事件名也改用模块导出的 `OPEN_ORCHESTRATION_EVENT` 常量。

**测试**：`src-ui/src/lib/__tests__/open-orchestration.test.ts`，7 条。
时序逻辑已抽成纯函数，不需要 jsdom。关键那条是
`initialRightTab > lets a pending request beat the remembered tab`
（`initialRightTab('tasks', true) === 'orchestration'`）。其余：
`honours the remembered tab when nothing is pending`、
`falls back to 方案与编排 for a missing or junk value`、
`leaves a one-shot flag that the next mount can read`、
`starts with nothing pending`、
`also fires the event for an already-mounted panel`（用 `EventTarget` 冒充
window，断言事件被处理后标记已清）、
`still records the request when there is no window`。

### R1-4 listGraphs 失败让 graphCount 变 0（低）

`setMetas([])` 同时承担了「一张图都没有」和「读不出来」两种含义，后者会让
`graphCount` 变 0，首启引导在图状态未知时也可能弹出来。

**改法**：`GraphTab` 加 `metasError` 状态。catch 里仍 `setMetas([])`（列表区要
停止转圈），但同时 `setMetasError(true)`；`firstRunFacts.graphCount` 改成
`metas === null || metasError ? null : metas.length`，落进「不知道 → 不显示」
那一支。开始视图的空列表文案也按 `metasError` 分成「读取已保存的图失败」与
「还没有保存的图」。

**测试**：判定逻辑本身已被
`first-run.test.ts > shouldShowFirstRun > hides while either fact is still unknown`
覆盖（`graphCount: null` → false）。本条只是把失败路径接到那个 `null` 上，
接线部分在 `GraphTab` 里，属于窗口手测项。

### R1-5 三处收尾（低）

- **`tokenizeByLang` 全仓无调用** → 删。它本是给 Markdown 围栏代码块准备的，
  从没接上；而上一轮的 `LANG_LOADERS` 改了它对未知围栏标签的行为，与其留一个
  没人跑过、语义又变了的导出，不如删掉并在原处留注释说明从 git 历史取回。
  `src-ui/src/lib/shiki.ts` 里 `tokenizeFile` 与 `getShikiTheme` 不变。
- **`vite.config.ts` 的 `as never`** → 直接删掉，不需要任何铸型。试着换成
  `@ts-expect-error` 时 `tsc` 报 `TS2578: Unused '@ts-expect-error' directive`，
  说明 Vite 8 的 `rollupOptions.output` 类型本来就接受 `codeSplitting`，
  上一轮那个 `as never` 是多余的。注释里写明了这一点。
- **shiki 冒烟测试** → 新增 `src-ui/src/lib/__tests__/shiki.test.ts`，4 条，
  走真实 loader：`highlights TypeScript`（tokens 非空、分了词、带主题色）、
  `highlights Python through a second grammar load`（第二次加载语法也走通）、
  `returns null for an extension we ship no grammar for`、
  `maps only the light theme to light; everything else is a dark variant`。
  测试跑在 node 环境（本仓库没装 jsdom），没有能 fetch `/assets/...` 的服务器，
  所以 `beforeAll` 里把 `globalThis.fetch` 换成从 `node_modules` 直接读那个
  `.wasm`；被测代码一行没改。这条补上了上一轮「不确定处 1」里最要紧的缺口 ——
  `shiki/core` + 显式主题 import + `?url` WASM + 显式语法表这四处改动，
  现在有一条真的会 token 化的测试压着了。
  （两个 `node:` 内置模块 import 各带一条 `@ts-expect-error`：
  `tsconfig.app.json` 的 `types` 故意只有 `vite/client`，把 `@types/node`
  整个拉进来会改写 `setTimeout` 的返回类型、打断 `TierTerminal.tsx`。）

### R1 验证

```
$ cargo check --tests 2>&1 | grep -c -i warning
0

$ cargo test
...
test result: ok. 179 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 4.10s

$ cd src-ui && npx tsc -b
（无输出）

$ cd src-ui && npx vitest run
 RUN  v5.0.0 D:/work/aicross-ui/src-ui

 Test Files  16 passed (16)
      Tests  167 passed (167)
   Start at  16:24:29
   Duration  536ms (transform 65%, import 18%, tests 14%, worker 3%)

$ cd src-ui && npm run build
...
dist/assets/main-B2CopKmA.js    420.88 kB | gzip: 209.76 kB
dist/assets/xterm-_r9IGi5_.js   465.79 kB | gzip: 123.12 kB
✓ built in 743ms

$ grep -c "chunks are larger" <build 输出>
0
```

vitest 从 R0 的 13 文件 148 条涨到 16 文件 167 条（+19：task-storage 8、
open-orchestration 7、shiki 4）。cargo 仍是 179 条（本轮无 Rust 改动）。
chunk 警告仍然没有，最大 chunk 465.79 kB 不变。

`tauri build` 本轮没有重跑：R1 只动前端与文档，Rust 侧一行未改，
R0 那次出包的产物与流程不受影响。要重新出包直接
`npx --yes -p @tauri-apps/cli@2 tauri build` 即可。

### R1 之后仍未消除的不确定处

- 窗口手测依旧没做（任务书禁 dev 窗口）。R1-3 的跳转、R1-4 的失败分支、
  以及任务 tab 的拖放仍需真窗口验证。R0「不确定处 1」里最担心的 diff 高亮，
  现在有 `shiki.test.ts` 兜着，优先级降下来了。
- R1-2 的「读不出」分支在真机上没造过场景（要把 `tasks.json` 锁住或改权限）。
  单测覆盖了逻辑，没覆盖 Rust 到前端这一跳的真实 reject。
