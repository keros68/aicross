# REPORT-08 界面细节修整 + Claude 模型清单

工作树 `D:\work\aicross-ui`，分支 `ui/polish`（自 main `0f511fd`）。未 commit、未 push、未起 Tauri 窗口、未写任何安装位。
`src/dispatch.rs`、`src/channels/`、`src-ui/src/graph/` 一行未动。

审的依据：`web-design-guidelines`（逐条检查清单，本次落在「Hover & Interactive States / Dark Mode & Theming / Anti-patterns」几节）与 `design-taste-frontend`（本次只取 Shape Consistency Lock、Color Consistency Lock、Button Contrast Check 三条；该 skill 自己的 Section 13 把「桌面产品 UI / dashboard」列为 out of scope，所以 hero / eyebrow / 图片策略那套没有套用）。

---

## 一、用户 09-14 指出的三处

### 1. 图顶栏「派发」按钮文字偏离基线

**哪里** `src-ui/src/components/center/graph/GraphTab.css` — `.graph-primary-btn`、`.graph-dispatch-toolbar .graph-primary-btn`

**原来怎样** 两个叠加的毛病：

- 基类 `.graph-primary-btn` 带 `align-self: flex-start`（开始视图里要让按钮左对齐用的）。工具条 `.graph-dispatch-toolbar` 是 `display:flex; align-items:center` 的 40px 条，`align-self` 优先级高于容器的 `align-items`，于是这颗按钮被顶到条的上沿，和左边同一行的 `<span>可派 N 路</span>`（按 `align-items:center` 居中）差了约 4-5px。
- 基类靠 `padding: 8px 16px` 撑高度，工具条又压了一个 `height: 30px`。8+8 的上下 padding 加 13px 字的行盒已经超过 30px，内容盒被压扁，文字在按钮框里也不在中线上。

**改成怎样**

- 基类改为 `display:inline-flex; align-items:center; justify-content:center; min-height:32px; padding:0 16px; line-height:1` —— 高度由 min-height 给，上下 padding 归零，文字在任何高度下都由 flex 居中。
- 工具条规则里显式写回 `align-self: center`，并删掉 `height:30px`（40px 条减去 4px 上下 padding 正好 32px 内容盒，按钮填满，不再需要单独压高度）。

**为什么** 「和左边不在一条基线上」有两个独立成因（align-self 顶到上沿、padding 挤扁行盒），只修一个仍会偏。改成 min-height + 行盒居中之后，开始视图里那两处 `.graph-primary-btn`（`GraphTab.tsx:323` / `:357`）的表现只从约 33px 变成 32px，左对齐行为由保留的 `align-self: flex-start` 兜住。

### 2. 右栏「方案与编排」头部上下间距

**哪里** `src-ui/src/components/right/Compiler.css`、`src-ui/src/components/right/orchestration/OrchestrationPanel.css`

**原来怎样** 根因在一条搬家后失效的负 margin：`TaskBoard.css` 的 `.right-tabs` 写着 `margin: 12px 0 -4px 0`，注释说明那 `-4px` 是为了抵消 `.task-board` 自己 `gap: 12px` 中的 4px。但壳层把 tab 提到了 `.compiler-top` 里，而 `.compiler-top` 是 `display:flex; flex-direction:column` **且没有 gap**，这 `-4px` 没有东西可抵消，直接把下面的面板往上拉了 4px。`.orch-panel` 的 `padding-top` 又是 0，于是预设下拉几乎贴着 tab 文字 —— 就是「挤在一起」。

另外 tab 本体 `padding: 6px 0` 在 14px 字号下只有 26px 高，下拉按 `padding: 6px 8px` 算出的高度和按钮行 `padding: 4px 12px` 又各是一档，三行控件没有一个共同高度。

**改成怎样**

- `Compiler.css` 给 `.right-shell-tabs` 显式写回 `margin: 12px 0 8px`，覆盖掉那条 `-4px`。
- `.right-shell-tabs .right-tab { padding: 8px 0 }`：26px -> 30px，和下面 32px 的下拉同一档。
- `.orch-preset-select` 从 `padding: 6px 8px` 改成 `height: 32px; padding: 0 8px`。
- `.orch-btn` 改成 `inline-flex` 居中 + `min-height: 28px; padding: 0 12px; line-height: 1`。

**为什么** 节奏现在是一条可数的序列：tab 上 12 -> tab 下 8 -> 下拉 32 高 -> `.orch-toolbar` 的 gap 8 -> 按钮行 28 高 -> `.orch-panel` 的 gap 12 -> 预览。三行控件高度 30/32/28 成梯度而不是乱数。固定高度而不是靠 padding，另有一层原因：原生 `<select>` 在 WebView2 里按 padding 算出的高度和 `<button>` 不一致，两行控件的边缘本来就对不齐。

### 3. 接入资源页 Claude Code 卡片没有 fable

**哪里** `src/resources.rs` — `claude_presets()` 及新增的 `claude_aliases()` / `claude_aliases_from_help()` / `parse_claude_model_aliases()` / `claude_model_help_section()`

**原来怎样** `["opus", "sonnet", "haiku"]` 硬编码，`source` 写死 `文档(2026-08-30)`。本机 claude 2.1.270 的 `--help` 里 `--model` 一段写的是 `'fable', 'opus', or 'sonnet'`，卡片缺 fable。

**改成怎样**

- `claude_presets` 收一个 `Option<&PathBuf>`（复用 `probe_agent` 里已经 `resolve_cli` 出来的那份，不再多跑一次 `where.exe`），跑 `claude --help`。
- `claude_model_help_section()` 从 `--model ` 那行起，把缩进更深的续排行折成一行，遇到空行或「缩进不深于 `--model` 且以 `-` 开头」的下一个选项行为止 —— help 是换行续排的，不折行的话引号会被换行切开，折过头又会把下一个选项的说明卷进来。
- `parse_claude_model_aliases()` 用正则 `'([^']+)'` 抓这一段里所有单引号内容，再用「纯 ASCII 小写字母、2..=16 字符」过一遍。这一步是必需的：同一段里还有完整模型名的举例 `'claude-fable-5'`，不过滤就会把它当成别名塞进卡片。
- 抓到就补 `haiku`（它一直不在 `--help` 举例里但确实可用），`source` 写 `claude --help 别名 (2026-09-14)`；抓不到 / 进程失败 / 超时 / 未检测到 claude，一律退回静态表 `["fable","opus","sonnet","haiku"]`，`source` 写 `静态表(2026-09-14)`。

**为什么** 任务书指定的改法。分成「取 help 段」「抓别名」「决定是否退表」三个纯函数，是为了让测试不用起子进程。冒烟按任务书没动。

**补的 Rust 测试**（`src/resources.rs` 的 `mod tests`）：

- `claude_help_yields_fable_opus_sonnet_plus_haiku`：fixture 是本机 2.1.270 `--help` 里那一段的原样（含换行续排、含 `'claude-fable-5'`、前后各夹一个相邻选项行）。断言抓出 `["fable","opus","sonnet"]`，补 haiku 后为四条，且结果里没有带连字符的完整名、没有把下一个选项 `-n, --name` 的说明卷进来。
- `claude_help_without_model_section_falls_back_to_static_table`：空文本与一段没有 `--model` 的 help，都断言 `from_help == false` 且落到静态四条。

---

## 二、其余发现（两组）

### A 组 · 间距节奏与按钮度量

**A1 `GraphTab.css` 派发浮层的间距全面回到 4 的倍数**

**哪里** `GraphTab.css` 第 15-63 行（`.dispatch-*` 整块）

**原来怎样** 文件自己开头的注释写着纪律「容器圆角 8px、字号 11/12/13/14、间距 4 的倍数」，但这一整块里散着 3 / 5 / 6 / 7 / 9 / 10 / 14 / 46 / 58 这些数：`.dispatch-sub` margin-top 3、`.dispatch-material-row` gap 5、其 label gap 7、`.dispatch-usage th/td` padding 7、`.graph-resume button` padding 0 9、`.dispatch-route-card + .dispatch-route-card` margin-top 10、`.dispatch-controls` margin-top 10、`.dispatch-result-grid` / `.dispatch-stage-column` gap 10、`.dispatch-result-text` margin-top 10、`.dispatch-records pre` padding 10、`.dispatch-authorization` / `.dispatch-records` margin-top 14、`.dispatch-collapsed` padding 0 14、`.dispatch-head` min-height 58、`.dispatch-running-row` min-height 46。

**改成怎样** 就近取 4 的倍数：3->4、5->4、7->8、9->8、10->8 或 12（看该处是「同级紧邻」还是「跨组」）、14->16、46->48、58->56、130->132。

**为什么** 这个浮层是结果屏的主场（路由卡、运行行、结果栅格、用量表），每处差 1-3px 累加起来就是用户说的「没有节奏」。纪律是文件自己写的，只是没执行到底。

**A2 浮层里的按钮统一按钮度量**

**哪里** `GraphTab.css` — `.dispatch-close, .dispatch-actions button, .dispatch-card-actions button, .dispatch-running-row button, .dispatch-controls button`；`.dispatch-collapsed`；`resources.css` — `.res-btn`、`.resources-close`

**原来怎样** 各写各的：浮层按钮 `height:30px; padding:0 12px`（无 flex，靠浏览器默认行盒居中）、`.res-btn` 是 `padding: 5px 12px` 无固定高度（同一张卡片里几颗按钮会因字体度量差 1-2px）、`.resources-close` 是 `padding: 6px 14px`。

**改成怎样** 全部统一成和 `.orch-btn` 一样的 `inline-flex + align-items:center + justify-content:center + min-height: 28px + padding: 0 12px + line-height: 1`。`.dispatch-collapsed` 加 `inline-flex + align-items:center`，padding `0 14px` -> `0 16px`。

**为什么** 任务书点名「按钮内文字垂直居中」。`height` 加零上下 padding 在纯拉丁文本下看不出问题，但界面是中英混排，CJK 字的行盒基线和 Latin 不同，靠默认行盒居中会飘；`min-height` + flex 居中对任何字形都成立。28px 作为次级按钮档，32px 作为主按钮/下拉档。

### B 组 · 主题 token：形状一致 + 暗/亮对比度

**B1 几个从未定义过的 CSS 变量（其中一个是亮色主题下的「黑字压黑底」）**

**哪里** 用 grep 把所有 `var(--x)` 与所有 `--x:` 定义对了一遍，CSS 里有 7 个只用不定义的名字。其中 `--dot` / `--ring` / `--gambit-dock-h` / `--wallpaper-opacity` 是 JS 行内注入的，正常；剩下的是真漏：

| 位置 | 原来 | 改成 |
|---|---|---|
| `SettingsModal.css:334` | `background: var(--bg-2, #222)` | `var(--bg-input)`，同时 `color` 的 `#fff` 兜底也删掉 |
| `MultiAgentGrid.css:196` | `color: var(--text-secondary, #aaa)` | `var(--text-2)` |
| `MultiAgentGrid.css:213` | `border-radius: var(--shape-radius-button, 6px)` | `var(--shape-radius-btn)` |
| `MultiAgentGrid.css:134` | `font-family: var(--font-display, 'SF Pro Display', ...)` | `var(--font)` |
| `CenterPanel.css:1591` | `font-family: var(--font-primary)`（无兜底） | `var(--font)` |
| `DiffPanel.css:212` | `background: var(--surface-hover, rgba(127,127,127,.12))` | `var(--bg-hover)` |

**为什么** 最严重的是第一条：`--bg-2` 在 `global.css` 的任何主题块里都没有，所以设置页「派发」那一组 input / select 永远吃 `#222` 兜底 —— **亮色主题下就是近黑输入框配 `--text-1` = `#1c1917` 的近黑文字，字基本看不见**。`--text-secondary` 的 `#aaa` 兜底在亮色主题 `#f5f5f4` 上约 2:1，远低于 AA。`--shape-radius-button` 是把真实 token 名 `--shape-radius-btn` 拼错了，结果那颗按钮对 sharp / slab / glass / carbon 五套形态完全不响应。`--font-primary` 没有兜底，整条声明直接无效。全部换成主题里真有的 token，不引入新色、不新增变量。

**B2 接入资源页与派发浮层的圆角接回形态系统**

**哪里** `resources.css` 全文（13 处）、`GraphTab.css` 的 `.dispatch-*` 块（6 处）、`SettingsModal.css` 的 `.settings-dispatch-field` 与 `.settings-role-template pre`

**原来怎样** 这两个文件的圆角全是写死的 `4px / 6px / 8px / 10px`，而 `global.css` 的 `[data-shape]` 系统（soft / slab / sharp / glass / panel / carbon 六套）只改 `--radius-*` 变量。写死的值等于这两个页面整体不参与形态切换：用户切到 sharp（全直角）或 carbon（16px），接入页和派发浮层还是圆 6px。

**改成怎样** 按尺寸映射到已有 token，收敛成两档：行 / 徽标 / 输入 / 内层卡 / 提示条 -> `var(--radius-xs)`；按钮 / 浮层 / 对话框 / 外层卡 -> `var(--radius-sm)`。（`resources.css` 的 `.res-dialog` 从 10px 归到 `--radius-sm`，和 `.dispatch-modal` 同档。）

**为什么** `design-taste-frontend` 的 Shape Consistency Lock：一页只能有一套圆角规则。这里不是「选一个好看的半径」，而是把两个漏网页面接回项目已有的形态系统 —— token 已存在、值不变（soft 下 xs=4、sm=8，视觉上只有 6px 那几处变成 4 或 8），只是切主题时不再脱节。

**B3 主按钮与错误色不再用会自我矛盾的值**

**哪里** `resources.css` — `.res-btn.primary`；`GraphTab.css` — `.dispatch-error, .dispatch-warning`；`SettingsModal.css` — `.settings-btn-danger`

**原来怎样**

- `.res-btn.primary { color: var(--accent) }`。但 `.orch-btn-primary` 和 `.graph-primary-btn` 的代码注释里已经写过这条结论：「文字用 text-1 不用 accent —— 单色主题的 accent 接近边框色，按 accent 上色会像禁用态」。cappuccino 的 `--accent` 是 `#4a4a4a`、obsidian 是 `#5a5a5a`，压在 `--accent-bg`（白 5-6% 透明）上几乎看不见。接入页的主按钮漏了这条。
- `.dispatch-error, .dispatch-warning { color: #e08b79 }` 写死。亮色主题 `#f5f5f4` 上约 2.6:1。
- `.settings-btn-danger { color: #e06666 }` 写死，同样不随主题走。

**改成怎样** `.res-btn.primary` -> `color: var(--text-1); font-weight: 600`（与另两颗主按钮完全一致）；两处写死的红 -> `var(--red)`。

**为什么** 前者是把仓库里已经写下来的规则补齐到第三颗主按钮（Button Contrast Check）；后者是让报错色随主题走（`--red` 亮色是 `#ff3b30` 约 3.3:1，仍不到 AA 4.5:1，但比 `#e08b79` 的 2.6:1 强，且 `resources.css` 本来就用 `var(--red)`，此处只是对齐）。`--red` 的绝对值没动，不算引入新色。

---

## 三、验证（命令与输出原文）

### `cd src-ui && npx vitest run`

```
 RUN  v5.0.0 D:/work/aicross-ui/src-ui


 Test Files  9 passed (9)
      Tests  91 passed (91)
   Start at  09:21:34
   Duration  682ms (transform 66%, import 24%, tests 6%, worker 4%)
```

### `cd src-ui && npm run build`

```
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...OK 580 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                     2.65 kB | gzip:   1.16 kB
dist/assets/main-D9ggHvQ_.css                     152.98 kB | gzip:  26.39 kB
dist/assets/dist-js-5OZv1H5C.js                     0.16 kB | gzip:   0.15 kB
dist/assets/app-XgSAopP-.js                         0.25 kB | gzip:   0.21 kB
```

（中间约 100 行是各语法高亮分包的体积清单，与本次改动无关，略；以下是结尾原文）

```
dist/assets/wasm-DA6mYL5T.js                      622.32 kB | gzip: 232.09 kB
dist/assets/cpp-Cyj1bR0k.js                       626.13 kB | gzip:  48.10 kB
dist/assets/emacs-lisp-DzXDqNne.js                779.87 kB | gzip: 197.56 kB
dist/assets/main-BO3GZlfy.js                    1,246.06 kB | gzip: 444.84 kB

built in 2.98s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

chunk 体积告警是既有的，跟本次改动无关（本次只动 CSS 与一个 Rust 文件）。

### `cargo test resources`（根目录）

```
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 39.91s
     Running unittests src\main.rs (target\debug\deps\aicross-3605bca6cf2a3148.exe)

running 21 tests
test resources::tests::live_codex_and_kimi_fact_sources_parse ... ignored, explicit local fact-source check
test resources::tests::live_detect_four_agents ... ignored, 真机验收：探测四个内置 agent（内存 store，不写 resources.json）
test resources::tests::live_smoke_pi_models ... ignored, 真机验收：冒烟两条 pi 模型（2 次模型请求；内存 store，不写盘）
test resources::tests::add_model_requires_provider_for_pi ... ok
test resources::tests::claude_base_url_read_from_settings_env ... ok
test resources::tests::claude_help_without_model_section_falls_back_to_static_table ... ok
test resources::tests::declared_models_are_not_marked_missing ... ok
test resources::tests::merge_preserves_user_fields_adds_pending_and_marks_missing ... ok
test resources::tests::pi_list_models_is_primary_store_only_supplies_metadata ... ok
test resources::tests::ensure_agents_drops_cc_switch_and_seeds_builtins ... ok
test resources::tests::pi_source_matching_uses_exact_tokens_and_auth_uses_top_level_status ... ok
test resources::tests::kimi_alias_keeps_full_prefix ... ok
test resources::tests::detection_refreshes_meta_preserves_note_and_parses_commented_kimi_toml ... ok
test resources::tests::parses_four_local_fact_sources ... ok
test resources::tests::pi_three_providers_parse_from_list_models ... ok
test resources::tests::cc_switch_produces_hints_only_no_dispatchable_models ... ok
test resources::tests::claude_help_yields_fable_opus_sonnet_plus_haiku ... ok
test resources::tests::connected_filters_status ... ok
test resources::tests::safe_summary_redacts_common_cli_secret_shapes ... ok
test resources::tests::corrupted_store_blocks_writes_and_keeps_bytes ... ok
test resources::tests::smoke_agent_models_partial_failure_writes_each_result ... ok

test result: ok. 18 passed; 0 failed; 3 ignored; 0 measured; 146 filtered out; finished in 0.03s
```

### 事实源核对（本机）

```
$ claude --version
2.1.270 (Claude Code)

$ claude --help | grep -n -A 12 -e "--model"
127:  --model <model>                       Model for the current session. Provide
128-                                        an alias for the latest model (e.g.
129-                                        'fable', 'opus', or 'sonnet') or a
130-                                        model's full name (e.g.
131-                                        'claude-fable-5').
132-  -n, --name <name>                     Set a display name for this session
```

测试 fixture 就是这段的原样（含相邻选项行，用来验证段边界）。

**没有实机看过的部分**：任务书要求不起 Tauri 窗口，所以上面所有视觉判断来自 CSS 规则与 `global.css` 的 token 定义推算，没有渲染截图核对。`npm run build` 只保证 CSS 语法与 TS 类型通过，不保证像素。

---

## 四、没改的

留给编排者决定，或需要开窗口才能判的：

1. **全局没有任何 `:focus-visible` 样式。** 12 个 CSS 文件里共 18 处 `outline: none`，没有一处给出替代焦点环 —— 键盘 Tab 走界面时完全看不出焦点在哪（`web-design-guidelines` 的 Focus States 整节全挂）。这是真缺陷，但补一个全局焦点环会改变每一个可聚焦元素的观感，不看窗口不该单方面定样式。建议单开一条：在 `global.css` 加 `:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 1px }` 再实机调。

2. **18 处 `transition: all`**（`CenterPanel.css` 4、`SettingsModal.css` 7、`TaskBoard.css` 3、`HistoryBoard.css` 2、`Explorer.css` 1、`global.css` 1）。属于清单里的 anti-pattern（会连 `width` / `height` 一起补间，非合成器属性）。改成逐条列属性是机械活，但每处要列哪几条得看实际 hover 状态，而且这些多半是上游 coffeecli 带来的，适合和「去 coffee 味」的下一刀一起做。

3. **没有 `prefers-reduced-motion` 断路。** `SettingsModal.css` 的 `settings-fade` / `settings-pop`、面板 250ms 滑入、`.titlebar-left-slot` 的 width 过渡都无条件播放。同样是加一条全局 `@media` 的事，但会影响全应用手感，留给编排者。

4. **`resources.css` 里其余用 `--accent` 当前景色的地方**：`.row-action`、`.resources-empty-link`、`.resource-manage-link:hover`、`.resource-status.pending`。在 cappuccino / obsidian 这类单色主题下和 B3 说的是同一个病（accent 是中灰）。只改了 `.res-btn.primary` 一颗，因为那是主按钮、且仓库里已有明文规则；剩下几处是链接态/徽标，换成 `--text-1` 会让它们失去「可点」的暗示，得先定一个「单色主题下用什么表示可点」的规则。

5. **`hint-capsule` / `builder-notation-warn` 的黄 `#d9a13b`、`builder-notation-error` 的 `#c4534a` 仍是写死色。** 项目没有 `--warning` token，任务书又说不引入新色，所以没动。要修的话是先在 `global.css` 的 13 套主题各加一个 `--warning`，那超出本期范围。

6. **三个 TSX/TS 里的未定义变量**：`Explorer.tsx:719` 的 `fill="var(--bg-0)"`（无兜底，实际渲染成黑色，是 AC logo 徽标里的字）、`lib/explorer-drag.ts:34` 的 `var(--bg-1, #2a2a2a)`（拖拽幽灵元素）、`ErrorBoundary.tsx:46` 的 `var(--warning, #f59e0b)`。后两个有可用兜底；第一个是真 bug，但它是品牌标记的一部分，改颜色要用户点头。

7. **`SettingsModal.css` 的 `.settings-chip { padding: 7px 14px }`、`.settings-rail-item { padding: 9px 12px; gap: 9px }`、`.settings-key-card { padding: 14px 16px }` 等一批 off-grid 值。** 设置页有独立的一套间距（9/11/14/15/18），和主界面的 4 倍数制不是一套。统一它意味着重排整个设置页，属于「推翻既有布局」，任务书明确不要。

8. **`.dispatch-usage` 用量表没有 `font-variant-numeric: tabular-nums`。** `body` 上有全局的 `tabular-nums`，但这张表用 `font:` 简写重设了 font，简写会把 `font-variant-numeric` 复位成 `normal` —— 数字列可能不对齐。没改是因为它跟 `--font-mono` 的实际字形有关，等距字体本来就等宽数字，是否真出问题得开窗口看。
