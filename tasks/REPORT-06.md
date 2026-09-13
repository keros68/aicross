# REPORT-06 界面身份第一刀

## 结果

本期把前端壳子的默认路径从“选择 CLI 后打开终端”调整为“进入对话图，再按需打开终端”：新建与兜底 tab 默认进入对话图起始视图；起始视图底部提供次级“打开终端”下拉，只列本机检测到的 agent CLI 与系统终端。右栏默认进入“方案与编排”，任务板退到第二项并改为中性空态。

标题栏不再露出合并输入与多 agent 布局按钮，左右栏快捷键只在 hover 的 `title` 中提示；原左右栏快捷键仍可用，合并输入快捷键不再响应。设置页只露出深、浅两套主题，默认深色；形状固定为 soft、图标固定为 outline，旧主题值启动时迁移为深色。背景图入口与旧值渲染链均已移除；形状、图标主题及合并输入设置不再露出。

没有修改 Rust、`port/`、依赖清单或 CDN 配置。没有修改 `src-ui/src/graph/`、`src-ui/src/components/center/graph/`、`src-ui/src/components/right/orchestration/` 中的逻辑或 CSS。终端、工作区、会话记录和修改记录功能保留。

## 改动文件清单

- `src-ui/index.html`：首屏预设改为深色 + soft，并在 React 挂载前迁移旧主题值。
- `src-ui/src/App.tsx`：不再挂载合并输入界面；独立保留左右栏快捷键监听。
- `src-ui/src/store/app-state.tsx`：默认与兜底 tab 改为 graph；主题有效值收窄；固定形状和图标方案并迁移旧值。
- `src-ui/src/lib/personalization.ts`：设置可选主题收成深、浅两项。
- `src-ui/src/styles/global.css`：深浅主题换成中性灰与单一低饱和强调色；统一 Sans/Mono 字体 token、tabular nums、8px 容器和 4px 行圆角 token。
- `src-ui/src/components/center/CenterPanel.tsx`：新 tab 默认进入 graph；对话图起始视图复用既有 CLI 检测、`selectTool` 与文件夹选择逻辑增加次级终端入口；旧说明入口去除 Coffee 品牌链接。
- `src-ui/src/components/center/CenterPanel.css`：增加对话图壳层的次级终端下拉样式。
- `src-ui/src/components/common/TitleBar.tsx`：移除合并输入和多 agent 布局按钮；快捷键改为 hover 提示。
- `src-ui/src/components/common/SettingsModal.tsx`：移除背景图、形状、图标和合并输入设置；反馈链接改为 aicross 仓库。
- `src-ui/src/components/left/Explorer.tsx`：更新检查改读 aicross GitHub release，点击后打开 aicross release 页，避免调用仍指向 Coffee 下载源的旧安装流程。
- `src-ui/src/components/right/Compiler.tsx`：右栏默认与首项改为“方案与编排”，任务在第二项；使用新存储键，避免旧默认值覆盖。
- `src-ui/src/components/right/TaskBoard.tsx`：去掉首次欢迎说明种子，空态“新建”接到各视图现有新增动作。
- `src-ui/src/components/right/TaskEmptyState.tsx`：空态改为“还没有任务”与“新建”。
- `src-ui/src/components/right/TaskBoard.css`：空态改为普通文字和 4px 圆角按钮。
- `src-ui/src/components/right/TaskNoteView.tsx`：空态回调改接新增任务。
- `src-ui/src/components/right/TaskPromptView.tsx`：为空态补接新增提示词。
- `src-ui/src/i18n/en.ts`、`src-ui/src/i18n/zh-CN.ts`、`src-ui/src/i18n/zh-TW.ts`：现有键名不变，清理任务空态问候与攻略文案；英文内部合并输入标题改为中性文字。

## 与第 3 期可能冲突的文件

### `src-ui/src/store/app-state.tsx`

- 第 402 行：关闭最后一个 tab 时的兜底 session 从 `tool: null` 改为 `tool: 'graph', toolData: ''`，保证任何空工作区都回到对话图起始视图。
- 第 611–635 行：`VALID_THEMES` 收窄为 `dark/light`；默认主题改为 dark，形状固定 soft、图标固定 outline；旧主题值迁移为 dark 并回写。原因是设置只允许深浅两套，同时避免旧本地值继续激活 Coffee 主题。
- 第 745–755 行：初始状态使用上述主题/形状/图标结果，首个 session 改为 graph 起始视图。

### `src-ui/src/components/center/CenterPanel.tsx`

- 第 602 行：旧 launchpad 中仍保留的说明工具改为中性“使用说明”；功能代码保留但不在默认路径露出。
- 第 775–819、846 行：把 CLI 检测触发条件从仅 launchpad 扩为“launchpad 或空 graph 起始视图”，供新的终端下拉显示本机检测结果。
- 第 869–872 行：新增 tab 的 session 默认改为 `graph`。
- 第 889–896 行：外部启动工具时允许复用空 graph 起始 tab，避免额外留下一个空图 tab。
- 第 1482–1528 行：在 `GraphTab` 外加壳层；仅当 `toolData` 为空时显示“打开终端”下拉，复用既有 `selectTool`、`handlePickFolder`、`toolsInstalled` 与 `lastCwdByTool`。`GraphTab` 自身及 `components/center/graph/` 未改。
- 第 1639 行：不可见的旧说明入口链接从 Coffee 课程页改为 aicross 仓库，防止未来重新露出时带回旧品牌。

### `src-ui/src/tauri.ts`

未修改。

## 验证（命令与输出原文）

### `cd src-ui && npx vitest run`（exit=1，未验证）

命令已实际执行。Vitest 尚未加载测试配置就被当前执行环境禁止创建子进程，错误为 `spawn EPERM`；因此测试结果记为“未验证”，不是用例失败。

```text
(node:40916) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:28732) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
failed to load config from D:\work\aicross-ui\src-ui\vite.config.ts

⎯⎯⎯⎯⎯⎯⎯ Startup Error ⎯⎯⎯⎯⎯⎯⎯⎯
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:458:11)
    at spawn (node:child_process:813:9)
    at Object.execFile (node:child_process:349:17)
    at exec (node:child_process:236:25)
    at optimizeSafeRealPathSync (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:1954:2)
    at windowsSafeRealPathSync (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:1940:3)
    at getRealPath (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:32413:36)
    at tryResolveRealFileOrType (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:32407:9)
    at tryCleanFsResolve (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:32151:21)
    at tryFsResolve (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:32144:14)
    at aggregateBindingErrorsIntoJsError (file:///D:/work/aicross-ui/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:48:18)
    at unwrapBindingResult (file:///D:/work/aicross-ui/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:18:128)
    at #build (file:///D:/work/aicross-ui/src-ui/node_modules/rolldown/dist/shared/rolldown-build-hRnqgxyz.mjs:3313:34)
    at async bundleConfigFile (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34633:17)
    at async bundleAndLoadConfigFile (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34541:18)
    at async loadConfigFromFile (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34510:42)
    at async resolveConfig (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34134:22)
    at async resolveConfig$1 (file:///D:/work/aicross-ui/src-ui/node_modules/vitest/dist/chunks/index.B89dZ0-N.js:14773:25)
    at async createVitest (file:///D:/work/aicross-ui/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:26:17)
    at async prepareVitest (file:///D:/work/aicross-ui/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:421:14) {
  errors: [Getter/Setter]
}
```

### `cd src-ui && npm run build`（exit=0，收尾原文）

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:39020) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
transforming...
✓ 568 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                      2.60 kB │ gzip:   1.15 kB
dist/assets/main-CUK-9z6c.css                      139.50 kB │ gzip:  24.36 kB
...
dist/assets/main-D9LJrjBf.js                     1,206.72 kB │ gzip: 433.57 kB

✓ built in 2.22s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

中间为 Vite 生成的静态资源清单；上面保留命令开头、转换结果、首末资源行、构建结果与完整警告原文。chunk 体积提示为既有构建警告。

### 窗口与截图

编排者窗口复核通过（09-13）：默认页、终端下拉、右栏默认、设置页只剩深浅、明亮主题均已核过。本修正轮按要求没有启动窗口。

### 范围核对

- `git diff --check`：通过，无输出。
- 受保护的三个目录执行 `git diff --name-only -- src-ui/src/graph src-ui/src/components/center/graph src-ui/src/components/right/orchestration`：无输出。
- `*.rs`、`Cargo.toml`、`Cargo.lock`、`src-ui/package.json`、`src-ui/package-lock.json`、`port/`：无改动。
- 未 commit，未 push。

## 修正轮 R1

### 逐条处理

1. **已修：背景图旧值不再渲染。** `getInitialState` 固定返回空 `bgPath` / `none`，启动时删除 `cc-bg-path` 与 `cc-bg-type`；`CenterPanel` 不再读取背景状态、转换本地路径或渲染背景层。上方结果表述已改为“背景图入口与旧值渲染链均已移除”。
2. **已修：删除多语言死键。** 11 个语言文件全部删除 `task.greeting.*`、`task.show_guide`、`task.welcome_note`，并删除 `task-types.ts` 的 `makeWelcomeNote`。全仓复搜 `Feeling ambitious`、`Großes`、`масштабное`、`usage guide`、“大事”、“攻略”，均无结果。
3. **已修：消除 `tool: null` 瞬态。** `applyLaunchRequest` 没有可复用起始 tab 时直接创建带目标 `tool` 与 `folderPath` 的 session，后续 `SET_TERMINAL_TOOL` 保留为幂等更新。
4. **已修：收敛重复表达式。** 新增 `isEmptyGraphSession` 统一判断空 graph 起始视图，CenterPanel 的探测、启动复用与渲染入口均调用它；新增 `createDefaultGraphSession`，由 reducer 兜底、初始状态和“新建 tab”共用。
5. **已修：终端下拉改为受控开合。** `details` 由状态控制，沿用 `RecentFolderMenu` 的 document `mousedown` / `keydown` 模式；Escape、点击外部、选择工具、选择文件夹、打开设置或离开空 graph tab 后都会关闭。
6. **已修：完成本期可见面的胶囊审计。** 任务常态/完成态使用状态色点或普通文字；Explorer 的 badge 是文件大小/差异计数，远程历史 pill 是可操作连接项，不是状态；graph/orchestration 的 `hint-capsule` 仅显示“未接入”或校验异常，符合异常状态用途且位于受保护目录。本期可见面未发现还需改色点或纯文字的常态/完成态胶囊。
7. **已修：soft tab 圆角改为 8px。** `:root` 与 `[data-shape="soft"]` 的 `--shape-radius-tab` 均为 `8px 8px 0 0`，同步修正 CenterPanel.css 示例注释。
8. **已修：注释与现状一致。** CenterPanel 的工具区注释改为 usage help，旧课程链接说明改为项目使用页；app-state 的快捷键监听位置改为 App。
9. **已修：避免无条件写个性化存储。** `index.html` 仅在 `cc-shape` 不同于 soft 时写入；`getInitialState` 仅在 `cc-shape` / `cc-icon-theme` 不同于 soft / outline 时写入。
10. **已修：补主题迁移与任务空态测试。** `app-state.test.ts` 覆盖旧 `sakura` 迁移并写回 dark、light 保留、缺省 dark/soft/outline 和背景旧值清理；`TaskEmptyState.test.tsx` 覆盖“还没有任务”“新建”、点击回调，以及 list/note → `handleAdd`、prompt → `handleAddPrompt` 的三处映射。为使任务书指定的 native config loader 可运行，`vite.config.ts` 改用 `import.meta.url` 推导配置目录，未增加依赖。
11. **不采纳：TitleBar aria-label 改回多语言键。** 维持硬编码中文；这与本期“新增文案直接写中文”的约定一致。

### 窗口验收

编排者窗口复核通过（09-13）：默认页、终端下拉、右栏默认、设置页只剩深浅、明亮主题均已核过。本修正轮遵照要求未启动窗口。

### 验证（命令与输出原文）

#### `cd src-ui && npx vitest run`（exit=1，沙箱启动失败）

~~~
(node:35420) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:27568) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
failed to load config from D:\work\aicross-ui\src-ui\vite.config.ts

⎯⎯⎯⎯⎯⎯⎯ Startup Error ⎯⎯⎯⎯⎯⎯⎯⎯
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:458:11)
    at spawn (node:child_process:813:9)
    at Object.execFile (node:child_process:349:17)
    at exec (node:child_process:236:25)
    at optimizeSafeRealPathSync (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:1954:2)
    at windowsSafeRealPathSync (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:1940:3)
    at getRealPath (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:32413:36)
    at tryResolveRealFileOrType (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:32407:9)
    at tryCleanFsResolve (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:32151:21)
    at tryFsResolve (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:32144:14)
    at aggregateBindingErrorsIntoJsError (file:///D:/work/aicross-ui/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:48:18)
    at unwrapBindingResult (file:///D:/work/aicross-ui/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:18:128)
    at #build (file:///D:/work/aicross-ui/src-ui/node_modules/rolldown/dist/shared/rolldown-build-hRnqgxyz.mjs:3313:34)
    at async bundleConfigFile (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34633:17)
    at async bundleAndLoadConfigFile (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34541:18)
    at async loadConfigFromFile (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34510:42)
    at async resolveConfig (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34134:22)
    at async resolveConfig$1 (file:///D:/work/aicross-ui/src-ui/node_modules/vitest/dist/chunks/index.B89dZ0-N.js:14773:25)
    at async createVitest (file:///D:/work/aicross-ui/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:26:17)
    at async prepareVitest (file:///D:/work/aicross-ui/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:421:14) {
  errors: [Getter/Setter]
}
~~~

#### `cd src-ui && npx vitest run --configLoader native --pool threads`（首次 exit=1）

native loader 首次运行发现 Vite ESM 配置仍依赖隐式 `__dirname`；改用 `fileURLToPath(import.meta.url)` 后以同一命令重跑。

~~~
(node:31488) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:43504) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
failed to load config from D:\work\aicross-ui\src-ui\vite.config.ts

⎯⎯⎯⎯⎯⎯⎯ Startup Error ⎯⎯⎯⎯⎯⎯⎯⎯
ReferenceError: __dirname is not defined in ES module scope
    at file:///D:/work/aicross-ui/src-ui/vite.config.ts?t=1789300067859:82:23
    at ModuleJob.run (node:internal/modules/esm/module_job:439:25)
    at process.processTicksAndRejections (node:internal/process/task_queues:104:5)
    at async node:internal/modules/esm/loader:643:26
    at async nativeImportConfigFile (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34528:18)
    at async loadConfigFromFile (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34510:42)
    at async resolveConfig (file:///D:/work/aicross-ui/src-ui/node_modules/vite/dist/node/chunks/node.js:34134:22)
    at async resolveConfig$1 (file:///D:/work/aicross-ui/src-ui/node_modules/vitest/dist/chunks/index.B89dZ0-N.js:14773:25)
    at async createVitest (file:///D:/work/aicross-ui/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:26:17)
    at async prepareVitest (file:///D:/work/aicross-ui/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:421:14)
~~~

#### `cd src-ui && npx vitest run --configLoader native --pool threads`（重跑 exit=0）

~~~
(node:23416) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20456) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 RUN  v5.0.0 D:/work/aicross-ui/src-ui

(node:20456) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20456) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20456) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20456) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20456) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20456) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20456) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 Test Files  7 passed (7)
      Tests  56 passed (56)
   Start at  19:54:42
   Duration  272ms (transform 53%, import 29%, tests 11%, worker 6%)
~~~

#### `cd src-ui && npm run build`（首次 exit=1）

首次构建发现已移除的背景状态 setter 仍有旧 JSX 引用；删除该背景层后以同一命令重跑。

~~~
> aicross-ui@0.0.0 build
> tsc -b && vite build

src/components/center/CenterPanel.tsx(1586,18): error TS2367: This comparison appears to be unintentional because the types '"none"' and '"video"' have no overlap.
src/components/center/CenterPanel.tsx(1587,89): error TS2304: Cannot find name 'setBgUrl'.
src/components/center/CenterPanel.tsx(1588,62): error TS2304: Cannot find name 'setBgUrl'.
~~~

#### `cd src-ui && npm run build`（重跑 exit=0）

~~~
> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:6944) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
transforming...✓ 568 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                         2.65 kB │ gzip:   1.16 kB
dist/assets/main-DeOMFOqf.css                         139.50 kB │ gzip:  24.36 kB
dist/assets/dist-js-5OZv1H5C.js                         0.16 kB │ gzip:   0.15 kB
dist/assets/app-XgSAopP-.js                             0.25 kB │ gzip:   0.21 kB
dist/assets/codeowners-YLWRiiGA.js                      0.54 kB │ gzip:   0.31 kB
dist/assets/image-B_hDXdWL.js                           0.61 kB │ gzip:   0.32 kB
dist/assets/shellsession-uwY3V5Kj.js                    0.71 kB │ gzip:   0.42 kB
dist/assets/tsv-FmpeqnFg.js                             0.73 kB │ gzip:   0.34 kB
dist/assets/html-derivative-y8cTjkde.js                 0.83 kB │ gzip:   0.45 kB
dist/assets/git-rebase-BkCIAZbs.js                      0.98 kB │ gzip:   0.44 kB
dist/assets/qmldir-DtRCXXGi.js                          1.00 kB │ gzip:   0.45 kB
dist/assets/csv-BCAXOHVN.js                             1.14 kB │ gzip:   0.37 kB
dist/assets/git-commit-D9uUyrpH.js                      1.22 kB │ gzip:   0.53 kB
dist/assets/xsl-Bo_Acr9l.js                             1.36 kB │ gzip:   0.50 kB
dist/assets/event-CCOELuOC.js                           1.40 kB │ gzip:   0.69 kB
dist/assets/dotenv-BG-R-dnJ.js                          1.42 kB │ gzip:   0.53 kB
dist/assets/sparql-I6g4RV9o.js                          1.47 kB │ gzip:   0.81 kB
dist/assets/ini-D1hC5V4K.js                             1.52 kB │ gzip:   0.50 kB
dist/assets/fortran-fixed-form-Dh8pw333.js              1.66 kB │ gzip:   0.69 kB
dist/assets/docker-BgEUgtmq.js                          1.73 kB │ gzip:   0.60 kB
dist/assets/hxml-_7MT1TaI.js                            1.74 kB │ gzip:   0.88 kB
dist/assets/desktop-DeW1bojR.js                         1.83 kB │ gzip:   0.76 kB
dist/assets/core-CAYfUUdT.js                            1.98 kB │ gzip:   0.82 kB
dist/assets/erb-Bx3F2RzE.js                             2.13 kB │ gzip:   0.62 kB
dist/assets/wenyan-B7WaDmpb.js                          2.15 kB │ gzip:   1.09 kB
dist/assets/jssm-nqlDcCik.js                            2.23 kB │ gzip:   0.61 kB
dist/assets/edge-BMCHHRKD.js                            2.30 kB │ gzip:   0.67 kB
dist/assets/reg-C9YB0Q6j.js                             2.34 kB │ gzip:   0.69 kB
dist/assets/diff-IPIU9Fzm.js                            2.56 kB │ gzip:   0.69 kB
dist/assets/gleam-Bqam2yKl.js                           2.57 kB │ gzip:   0.82 kB
dist/assets/defineProperty-Cc0ioC06.js                  2.59 kB │ gzip:   1.30 kB
dist/assets/hy-B7p7U1BT.js                              2.64 kB │ gzip:   1.18 kB
dist/assets/openscad-C-9uKvUO.js                        2.82 kB │ gzip:   1.01 kB
dist/assets/log-vQtdJHWP.js                             2.85 kB │ gzip:   0.90 kB
dist/assets/json-DG5xeaaD.js                            2.88 kB │ gzip:   0.81 kB
dist/assets/cairo-ClvR2owv.js                           2.93 kB │ gzip:   0.80 kB
dist/assets/berry-BUpy9_cc.js                           3.00 kB │ gzip:   0.81 kB
dist/assets/jsonl-DhVam5Gu.js                           3.00 kB │ gzip:   0.78 kB
dist/assets/jsonc-BK4mmDTw.js                           3.10 kB │ gzip:   0.79 kB
dist/assets/logo-CCxtNjv2.js                            3.12 kB │ gzip:   1.47 kB
dist/assets/po-CJUqynl0.js                              3.23 kB │ gzip:   0.91 kB
dist/assets/json5-B0H0ftRm.js                           3.25 kB │ gzip:   0.83 kB
dist/assets/mipsasm-BLGWrDdP.js                         3.25 kB │ gzip:   1.17 kB
dist/assets/tasl-BGeyykQ_.js                            3.28 kB │ gzip:   0.84 kB
dist/assets/webview-Dcvwb5z8.js                         3.31 kB │ gzip:   1.12 kB
dist/assets/genie-CN4rq7A0.js                           3.35 kB │ gzip:   1.21 kB
dist/assets/rel-DifwsMId.js                             3.36 kB │ gzip:   1.11 kB
dist/assets/vala-C6OpFbMA.js                            3.36 kB │ gzip:   1.19 kB
dist/assets/splunk-Cf3CW7wI.js                          3.43 kB │ gzip:   1.52 kB
dist/assets/hurl-2ACva3j7.js                            3.51 kB │ gzip:   1.10 kB
dist/assets/fluent-DGOyQ8Ws.js                          3.60 kB │ gzip:   0.90 kB
dist/assets/ssh-config-CKUNcAMa.js                      3.61 kB │ gzip:   1.60 kB
dist/assets/jsonnet-BOcQ7pWS.js                         3.61 kB │ gzip:   1.04 kB
dist/assets/kdl-DBigZy8w.js                             3.62 kB │ gzip:   1.04 kB
dist/assets/narrat-Cq3CR6rl.js                          3.67 kB │ gzip:   1.10 kB
dist/assets/turtle-D_eMCpxI.js                          3.69 kB │ gzip:   0.97 kB
dist/assets/glsl-DtQQAfSz.js                            3.70 kB │ gzip:   1.46 kB
dist/assets/zenscript-wcLUgsD2.js                       3.91 kB │ gzip:   1.28 kB
dist/assets/ron-ChSsJUxP.js                             3.91 kB │ gzip:   0.98 kB
dist/assets/gn-DJDjd0xf.js                              4.00 kB │ gzip:   1.48 kB
dist/assets/pascal-BHiDpJdH.js                          4.14 kB │ gzip:   1.67 kB
dist/assets/http-duTJ0maY.js                            4.41 kB │ gzip:   1.06 kB
dist/assets/tcl-CGMQ2Gre.js                             4.42 kB │ gzip:   1.50 kB
dist/assets/nextflow-DYYpq5fR.js                        4.50 kB │ gzip:   1.16 kB
dist/assets/rosmsg-C8yR4p9t.js                          4.52 kB │ gzip:   1.06 kB
dist/assets/polar-BiOMIDty.js                           4.66 kB │ gzip:   1.12 kB
dist/assets/sdbl-DbzyTi_y.js                            4.70 kB │ gzip:   2.01 kB
dist/assets/fennel-Cp7mIZmg.js                          4.76 kB │ gzip:   1.54 kB
dist/assets/bibtex-CMf_khHV.js                          4.79 kB │ gzip:   0.83 kB
dist/assets/llvm-BUQffIyu.js                            5.04 kB │ gzip:   2.02 kB
dist/assets/wgsl-8R5OMxfU.js                            5.14 kB │ gzip:   1.39 kB
dist/assets/gdresource-VMh-5Lix.js                      5.28 kB │ gzip:   1.34 kB
dist/assets/zig-7Lbxfjhx.js                             5.33 kB │ gzip:   1.55 kB
dist/assets/qml-DV-9MRuy.js                             5.33 kB │ gzip:   1.38 kB
dist/assets/dax-DlTOXg5S.js                             5.36 kB │ gzip:   2.22 kB
dist/assets/bicep-mN0ruteN.js                           5.38 kB │ gzip:   1.15 kB
dist/assets/xml-CyIjMK9K.js                             5.45 kB │ gzip:   1.25 kB
dist/assets/awk-C2cQpOjH.js                             5.45 kB │ gzip:   1.38 kB
dist/assets/coq-M9LrrHou.js                             5.52 kB │ gzip:   1.91 kB
dist/assets/jinja-CaX9vMDt.js                           5.62 kB │ gzip:   1.36 kB
dist/assets/lean-Dtsklvza.js                            5.78 kB │ gzip:   1.92 kB
dist/assets/moonbit-DelbyvlD.js                         5.89 kB │ gzip:   1.68 kB
dist/assets/powerquery-DDoDbTf0.js                      5.90 kB │ gzip:   1.52 kB
dist/assets/shaderlab-C-QDXGQP.js                       5.92 kB │ gzip:   2.08 kB
dist/assets/verilog-BNuILg-8.js                         5.92 kB │ gzip:   1.89 kB
dist/assets/cypher-Bf0d0j36.js                          5.95 kB │ gzip:   1.72 kB
dist/assets/vb-vQ7FBk14.js                              6.09 kB │ gzip:   2.34 kB
dist/assets/red-CgmMwQ5s.js                             6.26 kB │ gzip:   1.60 kB
dist/assets/min-dark-CBFjRQ4v.js                        6.28 kB │ gzip:   1.70 kB
dist/assets/gdshader-DLlTfkMm.js                        6.32 kB │ gzip:   1.72 kB
dist/assets/prisma-CPh4c3GQ.js                          6.32 kB │ gzip:   1.39 kB
dist/assets/ara-CpyWD-c6.js                             6.36 kB │ gzip:   1.81 kB
dist/assets/clojure-C3jjSFuY.js                         6.41 kB │ gzip:   1.42 kB
dist/assets/postcss-DAA_T9V0.js                         6.41 kB │ gzip:   1.91 kB
dist/assets/toml-BlynkCpr.js                            6.42 kB │ gzip:   1.28 kB
dist/assets/solarized-light-DLLndLS4.js                 6.47 kB │ gzip:   1.71 kB
dist/assets/proto-C0LdvFNL.js                           6.54 kB │ gzip:   1.42 kB
dist/assets/smalltalk-BIVJlCyX.js                       6.58 kB │ gzip:   1.62 kB
dist/assets/r-B4zVi_fs.js                               6.61 kB │ gzip:   1.83 kB
dist/assets/talonscript-Dr-lPTln.js                     6.75 kB │ gzip:   1.49 kB
dist/assets/solarized-dark-DgqPf06l.js                  6.84 kB │ gzip:   1.78 kB
dist/assets/riscv-BM628xIq.js                           6.91 kB │ gzip:   1.98 kB
dist/assets/soy-jLaB-7Vb.js                             6.92 kB │ gzip:   1.62 kB
dist/assets/min-light-OvUhfe4y.js                       6.96 kB │ gzip:   1.88 kB
dist/assets/scheme-BIfFz6Pe.js                          7.16 kB │ gzip:   2.05 kB
dist/assets/hlsl-I4TUSC5a.js                            7.26 kB │ gzip:   2.19 kB
dist/assets/qss-BgVkvApy.js                             7.47 kB │ gzip:   2.57 kB
dist/assets/systemd-D4187P0y.js                         7.86 kB │ gzip:   2.54 kB
dist/assets/monokai-DHFwemSO.js                         7.88 kB │ gzip:   1.90 kB
dist/assets/regexp-BnAW9hSB.js                          8.05 kB │ gzip:   1.47 kB
dist/assets/haml-DtItqECI.js                            8.34 kB │ gzip:   1.86 kB
dist/assets/typst-BP5N7E5y.js                           8.38 kB │ gzip:   1.67 kB
dist/assets/vue-html-C5v0WTbT.js                        8.47 kB │ gzip:   1.67 kB
dist/assets/plsql-DEXRAcVm.js                           8.51 kB │ gzip:   2.99 kB
dist/assets/dart-3JwWll0a.js                            8.62 kB │ gzip:   2.00 kB
dist/assets/horizon-C9nqL9R6.js                         8.77 kB │ gzip:   1.95 kB
dist/assets/kotlin-BYL6QRVS.js                          8.78 kB │ gzip:   2.13 kB
dist/assets/horizon-bright-Css32xyz.js                  8.78 kB │ gzip:   1.96 kB
dist/assets/make-Dw3yrZ0Q.js                            8.95 kB │ gzip:   1.77 kB
dist/assets/andromeeda-DGoCYgJo.js                      9.01 kB │ gzip:   2.36 kB
dist/assets/sas-CbDNbSM0.js                             9.06 kB │ gzip:   3.82 kB
dist/assets/dark-plus-DY33niKa.js                       9.09 kB │ gzip:   2.10 kB
dist/assets/slack-dark-BAvxGu1N.js                      9.11 kB │ gzip:   1.96 kB
dist/assets/sass-DocrZT3r.js                            9.28 kB │ gzip:   2.49 kB
dist/assets/plastic-iniznJdO.js                         9.29 kB │ gzip:   1.97 kB
dist/assets/slack-ochin-DWqn_Lk-.js                     9.42 kB │ gzip:   2.09 kB
dist/assets/tex-6AKuZd1g.js                             9.67 kB │ gzip:   3.06 kB
dist/assets/jison--pZ8m-HK.js                           9.69 kB │ gzip:   1.87 kB
dist/assets/cmake-DrqqGEK_.js                           9.85 kB │ gzip:   3.38 kB
dist/assets/light-plus-DfRHv4Xo.js                      9.93 kB │ gzip:   2.27 kB
dist/assets/hcl-WYMZvHje.js                            10.05 kB │ gzip:   1.93 kB
dist/assets/rst-CK2J_xNG.js                            10.30 kB │ gzip:   2.25 kB
dist/assets/pkl-DawJyhw3.js                            10.37 kB │ gzip:   1.38 kB
dist/assets/beancount-QTI4W_q2.js                      10.37 kB │ gzip:   1.43 kB
dist/assets/nextflow-groovy-A4by5qzv.js                10.43 kB │ gzip:   2.14 kB
dist/assets/dream-maker-C3tt4vD1.js                    10.46 kB │ gzip:   2.26 kB
dist/assets/raku-Ba22PpAY.js                           10.47 kB │ gzip:   2.94 kB
dist/assets/yaml-CzLfUaQ1.js                           10.57 kB │ gzip:   2.32 kB
dist/assets/just-RTSWN6SD.js                           10.77 kB │ gzip:   2.60 kB
dist/assets/elm-D_V6tPeg.js                            10.94 kB │ gzip:   2.10 kB
dist/assets/github-light-CIJNwzyc.js                   11.18 kB │ gzip:   2.49 kB
dist/assets/prolog-BCLGsSxa.js                         11.35 kB │ gzip:   3.85 kB
dist/assets/terraform-DfAmkD3Z.js                      11.38 kB │ gzip:   2.51 kB
dist/assets/github-dark-OFaljh7J.js                    11.40 kB │ gzip:   2.53 kB
dist/assets/puppet-BI7VePLm.js                         11.43 kB │ gzip:   2.11 kB
dist/assets/laserwave-CQtCIGFc.js                      11.49 kB │ gzip:   2.57 kB
dist/assets/gherkin-Cr0wk206.js                        11.94 kB │ gzip:   5.05 kB
dist/assets/wasm-Beqnk49Y.js                           12.00 kB │ gzip:   2.18 kB
dist/assets/hjson-CbFL5VWg.js                          12.05 kB │ gzip:   1.64 kB
dist/assets/handlebars-Dit8JC0x.js                     12.16 kB │ gzip:   2.38 kB
dist/assets/window-Cf5upF_J.js                         12.41 kB │ gzip:   3.01 kB
dist/assets/apache-mOm_gFPl.js                         12.45 kB │ gzip:   3.71 kB
dist/assets/vesper-B0XbIQa2.js                         12.69 kB │ gzip:   1.98 kB
dist/assets/bat-CD8xesoU.js                            12.88 kB │ gzip:   3.23 kB
dist/assets/fish-Bw4coD8r.js                           13.03 kB │ gzip:   1.73 kB
dist/assets/v-DG0D3hZq.js                              13.21 kB │ gzip:   2.73 kB
dist/assets/vitesse-light-piMchJtN.js                  13.61 kB │ gzip:   3.03 kB
dist/assets/aurora-x-WbeSdnAE.js                       13.65 kB │ gzip:   2.28 kB
dist/assets/vitesse-black-DuzBzXNC.js                  13.67 kB │ gzip:   3.06 kB
dist/assets/vitesse-dark-ChPfzWLu.js                   13.75 kB │ gzip:   3.06 kB
dist/assets/pug-CM6j4wdg.js                            13.85 kB │ gzip:   2.58 kB
dist/assets/luau-CsHfLQlf.js                           13.96 kB │ gzip:   3.17 kB
dist/assets/synthwave-84-B1wrF6Pb.js                   14.03 kB │ gzip:   2.85 kB
dist/assets/github-light-default-Cskbnfwe.js           14.15 kB │ gzip:   3.02 kB
dist/assets/clarity-zNHM1tMD.js                        14.27 kB │ gzip:   2.47 kB
dist/assets/github-light-high-contrast-BUZL_cRA.js     14.27 kB │ gzip:   3.00 kB
dist/assets/github-dark-dimmed-DLGfz6FO.js             14.43 kB │ gzip:   3.11 kB
dist/assets/github-dark-default-BqbMvdcT.js            14.43 kB │ gzip:   3.12 kB
dist/assets/github-dark-high-contrast-wBnEARx0.js      14.59 kB │ gzip:   3.08 kB
dist/assets/gnuplot-Bd1MpVfM.js                        14.78 kB │ gzip:   3.28 kB
dist/assets/rust-jpCPbsEK.js                           15.06 kB │ gzip:   2.72 kB
dist/assets/kusto-COSTdHqw.js                          15.17 kB │ gzip:   3.93 kB
dist/assets/actionscript-3-LMiqWuSE.js                 15.20 kB │ gzip:   2.68 kB
dist/assets/nix-BTV8Kl4V.js                            15.50 kB │ gzip:   2.48 kB
dist/assets/lua-BB1sjGF9.js                            15.60 kB │ gzip:   3.19 kB
dist/assets/abap-s17gYGWC.js                           15.85 kB │ gzip:   5.93 kB
dist/assets/solidity-D-b1Sb1s.js                       16.07 kB │ gzip:   3.10 kB
dist/assets/matlab-D7Ymh7TH.js                         16.08 kB │ gzip:   3.07 kB
dist/assets/cue-CHmhgwIJ.js                            16.20 kB │ gzip:   2.05 kB
dist/assets/elixir-1Wj0-XTx.js                         16.26 kB │ gzip:   2.76 kB
dist/assets/odin-CzGDjIwK.js                           16.51 kB │ gzip:   2.93 kB
dist/assets/ts-tags-NZ2iHfv6.js                        16.54 kB │ gzip:   2.05 kB
dist/assets/kanagawa-wave-D_Hm3vX6.js                  17.12 kB │ gzip:   2.90 kB
dist/assets/kanagawa-lotus-CqgQE5Hl.js                 17.12 kB │ gzip:   2.91 kB
dist/assets/kanagawa-dragon-CByus8H8.js                17.12 kB │ gzip:   2.92 kB
dist/assets/move-BG_bNIu4.js                           17.50 kB │ gzip:   3.07 kB
dist/assets/graphql-DZnKSmmH.js                        18.08 kB │ gzip:   2.54 kB
dist/assets/liquid-DAb-uuE2.js                         18.10 kB │ gzip:   3.15 kB
dist/assets/svelte-CPH69QWw.js                         18.25 kB │ gzip:   3.14 kB
dist/assets/material-theme-CY5ZxTT5.js                 18.61 kB │ gzip:   3.10 kB
dist/assets/material-theme-darker-BnDPOEon.js          18.62 kB │ gzip:   3.10 kB
dist/assets/material-theme-ocean-CAu3HLd7.js           18.62 kB │ gzip:   3.12 kB
dist/assets/material-theme-lighter-CXYE62LH.js         18.63 kB │ gzip:   3.11 kB
dist/assets/material-theme-palenight-BLPSe7SZ.js       18.63 kB │ gzip:   3.12 kB
dist/assets/gdscript-C2D5WM82.js                       18.99 kB │ gzip:   3.75 kB
dist/assets/groovy-BeOakm-O.js                         19.17 kB │ gzip:   3.60 kB
dist/assets/mdc-C5YQj5X-.js                            19.54 kB │ gzip:   6.67 kB
dist/assets/ayu-dark-C8AZSEqd.js                       20.07 kB │ gzip:   3.93 kB
dist/assets/ayu-mirage-DJDt9_Ik.js                     20.08 kB │ gzip:   3.91 kB
dist/assets/glimmer-js-Z7X_LugF.js                     20.08 kB │ gzip:   2.94 kB
dist/assets/glimmer-ts-CmBNQRbz.js                     20.08 kB │ gzip:   2.94 kB
dist/assets/powershell-DThFFtVM.js                     20.14 kB │ gzip:   4.06 kB
dist/assets/ayu-light-DGy4vufb.js                      20.15 kB │ gzip:   3.90 kB
dist/assets/viml-CP_eTIVX.js                           20.36 kB │ gzip:   6.74 kB
dist/assets/nushell-_kwMquEq.js                        20.41 kB │ gzip:   5.22 kB
dist/assets/snazzy-light-DEGtvnBx.js                   20.77 kB │ gzip:   3.81 kB
dist/assets/twig-EY5SnVCc.js                           20.90 kB │ gzip:   3.66 kB
dist/assets/dracula-C8tduV4h.js                        21.06 kB │ gzip:   4.00 kB
dist/assets/dracula-soft-i03TrcGR.js                   21.07 kB │ gzip:   4.02 kB
dist/assets/wit-Q3uzgIzy.js                            21.46 kB │ gzip:   2.88 kB
dist/assets/rose-pine-IZyio8vo.js                      21.74 kB │ gzip:   3.86 kB
dist/assets/rose-pine-moon-BfRCxBHe.js                 21.75 kB │ gzip:   3.88 kB
dist/assets/rose-pine-dawn-D1oZXPoh.js                 21.75 kB │ gzip:   3.88 kB
dist/assets/nim-mCDe1XWm.js                            22.45 kB │ gzip:   3.16 kB
dist/assets/common-lisp-D0DsnuEJ.js                    22.57 kB │ gzip:   6.07 kB
dist/assets/surrealql-DhK7dDdV.js                      22.58 kB │ gzip:   4.31 kB
dist/assets/gruvbox-dark-hard-BqtMDTUh.js              22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-dark-soft-CV57n6pA.js              22.63 kB │ gzip:   4.15 kB
dist/assets/gruvbox-light-hard-b4k0L1XS.js             22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-light-soft-DP66dnY9.js             22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-dark-medium-Dgfy8-dz.js            22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-light-medium-DSewW6QF.js           22.63 kB │ gzip:   4.16 kB
dist/assets/bird2-Bh111mYM.js                          22.94 kB │ gzip:   4.66 kB
dist/assets/sql-DcHRMm-Q.js                            23.47 kB │ gzip:   7.49 kB
dist/assets/cadence-DXH0Dgwr.js                        23.66 kB │ gzip:   3.66 kB
dist/assets/astro-XmnDH_Dd.js                          24.02 kB │ gzip:   7.60 kB
dist/assets/apl-DjqofZJD.js                            24.03 kB │ gzip:   4.17 kB
dist/assets/templ-BkokcB7D.js                          24.06 kB │ gzip:   5.41 kB
dist/assets/angular-html-BlerCZUb.js                   24.25 kB │ gzip:   3.98 kB
dist/assets/vhdl-3hDyUkZR.js                           24.26 kB │ gzip:   3.86 kB
dist/assets/vue-CqFBltHt.js                            24.50 kB │ gzip:   2.97 kB
dist/assets/purescript-DKMYP_A1.js                     24.68 kB │ gzip:   3.24 kB
dist/assets/typespec-BQy10TzM.js                       25.04 kB │ gzip:   2.62 kB
dist/assets/one-light-27e-9kYd.js                      25.29 kB │ gzip:   3.63 kB
dist/assets/fsharp-BQAqzjqk.js                         25.30 kB │ gzip:   4.13 kB
dist/assets/marko-Cj1_9Cb9.js                          25.48 kB │ gzip:   3.59 kB
dist/assets/c3-qRBCI8Uv.js                             25.62 kB │ gzip:   3.86 kB
dist/assets/night-owl-light-DtpYOdyl.js                25.90 kB │ gzip:   4.24 kB
dist/assets/system-verilog-jOLk1CBm.js                 26.20 kB │ gzip:   4.83 kB
dist/assets/nord-BQkWtC2v.js                           26.72 kB │ gzip:   4.36 kB
dist/assets/codeql-BoicTpVa.js                         26.88 kB │ gzip:   3.80 kB
dist/assets/scss-Dw42T1BI.js                           27.27 kB │ gzip:   4.23 kB
dist/assets/java-CoApPKT4.js                           27.28 kB │ gzip:   4.31 kB
dist/assets/coffee-fAb6LsdV.js                         27.42 kB │ gzip:   6.35 kB
dist/assets/razor-ccbgdA8j.js                          27.45 kB │ gzip:   3.54 kB
dist/assets/scala-D0LjI2Qi.js                          28.88 kB │ gzip:   3.93 kB
dist/assets/night-owl-DDmCLEp1.js                      28.91 kB │ gzip:   5.15 kB
dist/assets/crystal-HHV-Zo9X.js                        29.41 kB │ gzip:   4.44 kB
dist/assets/mermaid-CAtOlIbf.js                        29.50 kB │ gzip:   3.65 kB
dist/assets/applescript-C5yVau2U.js                    29.56 kB │ gzip:   5.95 kB
dist/assets/julia-jUIBt023.js                          30.99 kB │ gzip:   4.30 kB
dist/assets/stylus-CNnrGufP.js                         31.07 kB │ gzip:   8.02 kB
dist/assets/poimandres-H4_Eyox2.js                     33.49 kB │ gzip:   5.49 kB
dist/assets/one-dark-pro-C4hn0Zoq.js                   33.78 kB │ gzip:   5.50 kB
dist/assets/bsl-ny3e_7dl.js                            33.86 kB │ gzip:   8.35 kB
dist/assets/haxe-C2AyQsKn.js                           35.16 kB │ gzip:   5.91 kB
dist/assets/nginx-B8zy1DQG.js                          35.35 kB │ gzip:   4.46 kB
dist/assets/houston-ByNQ9eAt.js                        35.42 kB │ gzip:   5.76 kB
dist/assets/tokyo-night-DQIh82d3.js                    35.66 kB │ gzip:   6.23 kB
dist/assets/erlang-Q-lfVOgV.js                         37.47 kB │ gzip:   4.36 kB
dist/assets/cobol-DQafa4Ab.js                          39.07 kB │ gzip:  10.89 kB
dist/assets/asm-Bbv1919r.js                            40.71 kB │ gzip:   8.17 kB
dist/assets/haskell-DTAtmdln.js                        41.48 kB │ gzip:   6.41 kB
dist/assets/shellscript-3Eyrpdfm.js                    41.54 kB │ gzip:   6.13 kB
dist/assets/perl-CEA6MP1k.js                           43.14 kB │ gzip:   4.65 kB
dist/assets/d-Dt1IpaRS.js                              43.79 kB │ gzip:   8.43 kB
dist/assets/ruby-B-fKc9x2.js                           46.60 kB │ gzip:   5.71 kB
dist/assets/go-D3d4JNoO.js                             46.81 kB │ gzip:   5.15 kB
dist/assets/apex-Bkns-Kf9.js                           46.98 kB │ gzip:   6.73 kB
dist/assets/catppuccin-mocha-D4_Pt6ay.js               47.25 kB │ gzip:   7.98 kB
dist/assets/catppuccin-latte-Coqss7en.js               47.25 kB │ gzip:   7.99 kB
dist/assets/catppuccin-frappe-C5l_8kFT.js              47.25 kB │ gzip:   7.99 kB
dist/assets/catppuccin-macchiato-DEc0mBz1.js           47.26 kB │ gzip:   7.99 kB
dist/assets/ada-CgdG45k-.js                            48.08 kB │ gzip:   5.96 kB
dist/assets/css-BaIEaAP4.js                            49.11 kB │ gzip:  11.96 kB
dist/assets/imba-D2BupS_o.js                           49.92 kB │ gzip:   9.47 kB
dist/assets/everforest-dark-DnGtViwz.js                53.74 kB │ gzip:   8.37 kB
dist/assets/everforest-light-CrRagu-i.js               53.74 kB │ gzip:   8.37 kB
dist/assets/wikitext-CokaWg4C.js                       55.88 kB │ gzip:   4.82 kB
dist/assets/stata-LzqjS0NL.js                          56.98 kB │ gzip:  12.39 kB
dist/assets/html-B1M_0gd8.js                           57.32 kB │ gzip:  11.79 kB
dist/assets/ballerina-C5NCmhdh.js                      58.69 kB │ gzip:   8.11 kB
dist/assets/markdown-DrnMgzIX.js                       59.33 kB │ gzip:   5.67 kB
dist/assets/ocaml-X3W8brJ7.js                          62.44 kB │ gzip:   5.05 kB
dist/assets/mojo-BEOkTXe0.js                           69.79 kB │ gzip:   9.22 kB
dist/assets/python-BtjLSkiI.js                         69.95 kB │ gzip:   9.10 kB
dist/assets/c-yOg8G6Qk.js                              72.17 kB │ gzip:  10.55 kB
dist/assets/latex-DFkYhUza.js                          72.61 kB │ gzip:   6.69 kB
dist/assets/vyper-BGjJ65g4.js                          74.64 kB │ gzip:  10.70 kB
dist/assets/hack-CB6YBmfz.js                           80.18 kB │ gzip:  26.27 kB
dist/assets/swift-ByDBHGEa.js                          86.68 kB │ gzip:  14.69 kB
dist/assets/fortran-free-form-BXrNE5X_.js              88.96 kB │ gzip:  11.19 kB
dist/assets/csharp-DK3gKUDm.js                         89.68 kB │ gzip:  10.64 kB
dist/assets/racket-DChGz2wN.js                         92.38 kB │ gzip:  15.08 kB
dist/assets/less-CCCdi_4d.js                           97.63 kB │ gzip:  14.85 kB
dist/assets/blade-DhyUajd-.js                         104.97 kB │ gzip:  28.30 kB
dist/assets/objective-c-Do3Dh539.js                   105.41 kB │ gzip:  23.64 kB
dist/assets/php-D1n20mva.js                           111.31 kB │ gzip:  28.66 kB
dist/assets/asciidoc-Mnh7mpqW.js                      131.52 kB │ gzip:   9.32 kB
dist/assets/mdx-BhxtouIL.js                           136.11 kB │ gzip:  23.54 kB
dist/assets/objective-cpp-DzvX7O7S.js                 171.97 kB │ gzip:  30.98 kB
dist/assets/javascript-DMDj563O.js                    174.89 kB │ gzip:  16.68 kB
dist/assets/tsx-Du47Vi3R.js                           175.60 kB │ gzip:  16.67 kB
dist/assets/jsx-BgYHCIzu.js                           177.86 kB │ gzip:  16.77 kB
dist/assets/typescript-Biw9qNLM.js                    181.14 kB │ gzip:  16.29 kB
dist/assets/angular-ts-BErpjn_O.js                    183.73 kB │ gzip:  16.77 kB
dist/assets/vue-vine-DaNosUn7.js                      190.06 kB │ gzip:  18.07 kB
dist/assets/dist-CbAFeoTq.js                          212.97 kB │ gzip:  67.93 kB
dist/assets/wolfram-BPifCLvx.js                       262.38 kB │ gzip:  77.64 kB
dist/assets/wasm-DA6mYL5T.js                          622.32 kB │ gzip: 232.09 kB
dist/assets/cpp-Cyj1bR0k.js                           626.13 kB │ gzip:  48.10 kB
dist/assets/emacs-lisp-DzXDqNne.js                    779.87 kB │ gzip: 197.56 kB
dist/assets/main-D9M-Eptb.js                        1,195.77 kB │ gzip: 430.26 kB

✓ built in 1.95s
[plugin builtin:vite-reporter] 
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
~~~

构建仅有既有的 chunk 体积提示。

### 范围复核

- 受保护的 `src-ui/src/graph/`、`src-ui/src/components/center/graph/`、`src-ui/src/components/right/orchestration/` 无改动。
- `*.rs`、`Cargo.toml`、`Cargo.lock`、`src-ui/package.json`、`src-ui/package-lock.json`、`port/` 无改动；没有新增依赖。
- 本修正轮没有启动窗口，没有 commit，没有 push。
