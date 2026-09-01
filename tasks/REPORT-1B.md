# REPORT-1B — 界面对齐到原型

## 改了什么

- 重写前端视觉 token 与基础样式：系统 Sans / Mono 字体栈、浅色与深色两套 token、灰色 chrome 与白色内容面、单一强调色、1px 边框、8px 容器圆角、4px 列表行圆角和六级字号。
- 顶栏改为 `AC` 品牌标记、产品名、分隔线、等宽项目名和 28px 线框图标按钮。接入资源仍由顶栏进入；打开目录、设置和最近项目功能保留。
- 增加深浅色切换。现有 Rust `Settings` 没有主题字段，按任务书选择前端 `localStorage` 兜底，键为 `aicross.theme`；没有新增或修改 Rust 结构。终端配色同步响应手动主题。
- 左栏资源只显示 `status=已接入` 的模型，按 agent 折叠分组；组头只保留 agent 名和版本，模型行只显示模型 ID 与来源。无已接入模型的 agent 不显示。栏底增加待冒烟／未接入数量与“管理 →”入口。
- 接入资源改为最大宽度 980px 的 agent 卡片网格。卡片按 agent id 生成两字母标记，显示检测状态、版本、按 provider 分组的全部模型与状态，并保留探测、整卡冒烟、单模型冒烟、手动输入、移除、厂商／档位编辑及模型详情。
- 中栏和两侧 tab 改为原型的上圆角标签形态；选中标签与内容面连成一体。工作区、资源、记录、终端、修改记录、方案与编排、任务列表 tab 及其开关行为均保留。

## 验证

### 1. 前端构建

构建本身已通过。工具直接以 `D:\work\aicross\src-ui` 为工作目录运行，等价于先执行 `cd src-ui`：

```text
> npm run build

> aicross-ui@0.0.0 build
> tsc && vite build

(node:41284) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v7.3.6 building client environment for production...
transforming...
✓ 47 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                   0.45 kB │ gzip:   0.29 kB
dist/assets/index-BJy_4tI1.css   21.53 kB │ gzip:   4.67 kB
dist/assets/index-rPjW2P_J.js   520.13 kB │ gzip: 144.81 kB

(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
✓ built in 1.33s
```

字面命令 `cd src-ui && npm run build` 也实际运行过，但该命令链在当前沙箱中两次遇到 Node 启动 esbuild 的 `spawn EPERM`；同一最终代码随后用上述等价工作目录调用构建成功。失败原文：

```text
> cd src-ui && npm run build

> aicross-ui@0.0.0 build
> tsc && vite build

failed to load config from D:\work\aicross\src-ui\vite.config.ts
error during build:
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:420:11)
    at Object.spawn (node:child_process:787:9)
    at ensureServiceIsRunning (D:\work\aicross\src-ui\node_modules\esbuild\lib\main.js:2272:29)
    at build (D:\work\aicross\src-ui\node_modules\esbuild\lib\main.js:2170:26)
    at bundleConfigFile (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/config.js:35895:23)
    at bundleAndLoadConfigFile (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/config.js:35882:24)
    at loadConfigFromFile (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/config.js:35851:179)
    at resolveConfig (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/config.js:35500:28)
    at resolveConfigToBuild (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/config.js:33523:9)
    at createBuilder (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/config.js:33955:25)
```

### 2. TypeScript

任务书指定的配置文件在仓库中不存在，因此原命令为 **未验证**，实际运行输出：

```text
> npx tsc --noEmit -p tsconfig.app.json
(node:32832) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
error TS5058: The specified path does not exist: 'tsconfig.app.json'.
```

使用仓库实际配置 `src-ui/tsconfig.json` 的等价检查通过：

```text
> npx tsc --noEmit -p tsconfig.json
(node:16232) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
```

### 3. Rust

```text
> cargo check --manifest-path src-tauri/Cargo.toml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.44s
```

### 4. 字号阶梯

任务书原命令已实际运行，但 Git for Windows 的 `grep.exe` 在当前沙箱不能创建共享映射，因此原命令为 **未验证**：

```text
> grep -o 'font-size:[^;]*' src-ui/src/styles.css | sort -u
      0 [main] grep (40452) C:\Program Files\Git\usr\bin\grep.exe: *** fatal error - CreateFileMapping S-1-5-21-1830235385-2720854875-1956724854-1002.1, Win32 error 5.  Terminating.
```

使用 PowerShell 等价命令完成自查：

```text
> (Select-String -LiteralPath src-ui/src/styles.css -Pattern 'font-size:\s*[^;]*' -AllMatches).Matches.Value | Sort-Object -Unique
font-size: 11px
font-size: 12px
font-size: 13px
font-size: 14px
font-size: 16px
font-size: 18px
```

### 5. 阴影

任务书原命令已实际运行，但同样受 `grep.exe` 沙箱错误影响，原命令为 **未验证**：

```text
> grep -c 'box-shadow' src-ui/src/styles.css
      0 [main] grep (33920) C:\Program Files\Git\usr\bin\grep.exe: *** fatal error - CreateFileMapping S-1-5-21-1830235385-2720854875-1956724854-1002.1, Win32 error 5.  Terminating.
```

使用 PowerShell 等价命令得到 1 处：

```text
> (Select-String -LiteralPath src-ui/src/styles.css -Pattern 'box-shadow').Count; Select-String -LiteralPath src-ui/src/styles.css -Pattern 'box-shadow' | ForEach-Object { "{0}:{1}" -f $_.LineNumber,$_.Line.Trim() }
1
270:.modal { width: min(460px, 100%); padding: 24px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface); box-shadow: var(--shadow); }
```

该处只用于弹窗，属于任务书允许保留阴影的浮层。

## 没做什么

- 未改 `src-tauri/`、`src-ui/src/api.ts`、npm 依赖、lockfile，也未引入 CDN 或 CSS 框架。
- 当前代码没有预设下拉，因此没有执行“X 模式”命名替换；流水线阶段名和结果页文案也未改。
- 未运行真实 agent 探测或冒烟，避免产生外部模型调用。
- 未做窗口手测；按任务书由编排者在沙箱外验收，本文不声称已完成视觉手测。
- 未 commit，未 push。

## 不确定处

- Vite 构建有既有的单 chunk 超过 500 kB 警告，不影响本次构建通过。
- `tsconfig.app.json` 缺失以及 Git `grep.exe` 的沙箱错误使任务书对应字面命令无法通过；已分别用仓库实际 TypeScript 配置和 PowerShell 等价检查补充验证。
