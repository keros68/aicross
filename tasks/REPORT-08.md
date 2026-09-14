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

---

# TASK-08b 节点面板模型清单 + 盖预设菜单被压盖

同一分支 `ui/polish`（TASK-08 已并入 main，commit `b88afd5`）。未 commit、未 push、未起窗口。
`src/dispatch.rs`、`src/channels/` 未动；`src-ui/src/graph/` 只读不改（新加的挑模型辅助函数放在 `src-ui/src/resources/model-picker.ts`）。

## 1. 节点面板「模型」下拉只有 4 项

**哪里** `src/resources.rs`（新增 `detected()` / `status_rank()`）、`src/orchestration.rs`（`dispatchable_from`）、新建 `src-ui/src/resources/model-picker.ts`、`NodePanel.tsx`、`GraphTab.tsx` 的 `@` 弹层、`DispatchOverlay.tsx`、`SettingsModal.tsx`

**原来怎样** `list_dispatchable_models` -> `dispatchable_from` -> `resources::connected()`，而 `connected()` 按 `status == "已接入"` 过滤。本机 `~/.aicross/resources.json` 里非 cc-switch 的模型共 47 条，只有 4 条冒过烟，所以下拉里就是截图上那 4 项：

```
non-cc-switch models total: 47
('claude', '', '已接入') 1        ('claude', '', '待冒烟') 3
('codex', '', '已接入') 1         ('codex', '', '待冒烟') 7
('kimi', '', '已接入') 1          ('kimi', '', '待冒烟') 3
('pi', 'deepseek', '冒烟失败') 1   ('pi', 'deepseek', '待冒烟') 2
('pi', 'qwen-token-plan-cn', '待冒烟') 18
('pi', 'zai-coding-cn', '已接入') 1 ('pi', 'zai-coding-cn', '待冒烟') 9
```

pi 一家三个 provider 合计 31 条，和任务书说的数字对上。

**改成怎样**

- `resources.rs` 加 `pub fn detected(path)`：只排除 cc-switch，不按冒烟状态过滤；排序 `status_rank`（已接入 0 -> 待冒烟 1 -> 冒烟失败 2 -> 其它 3），同状态内 `agent -> provider -> id`。`connected()` **原样保留**，两者分工写进了 doc 注释。
- `orchestration.rs` 的 `dispatchable_from` 改用 `detected`，并注明为什么不是 `connected`。
- `DispatchableModel` 的 TS 类型**不用改**：`ResourceModel` 本来就带 `status`，`src-ui/src/graph/types.ts:91` 早就声明了这个字段，序列化一直在传，只是过滤掉了没冒烟的条目。也就是说 `src-ui/src/graph/` 一行没动。
- 新建 `src-ui/src/resources/model-picker.ts`，四个纯函数：`modelGroupLabel`（无 provider -> `agent`；有 -> `agent · provider`）、`modelStatusSuffix`（已接入与空状态不加后缀，其余加「（待冒烟）」「（冒烟失败）」）、`modelOptionLabel`、`groupPickableModels`（按标签归组，**不二次排序**，顺序完全跟后端）、`modelKey`。
- `NodePanel.tsx` 的 `<optgroup>` 从「按 agent」换成这套分组；`GraphTab.tsx` 的 `@` 弹层、`DispatchOverlay.tsx` 的目标下拉、`SettingsModal.tsx` 的角色默认模型下拉都用同一套后缀。
- `DispatchOverlay.tsx` 的 G1 决策卡加一行：选中模型状态不是「已接入」时显示「该模型{状态}，尚未核对真身；派发时通道会核对。」派发逻辑一行未改，纯展示。

**为什么** 后端一次性给全量 + status，前端只管显示，是最小改动面：`connected()` 的语义（可派发清单）保持原样，左栏资源 tab 走的是 `connectedByAgent(store)` 读原始 store，完全不受影响。排序放后端而不是前端，是因为前端有四处消费同一份数组，排一次比排四次靠谱。

**`modelKey` 为什么带 provider**：pi 的三个 provider 之间可能有同名模型 id，原来的 React key `${agent}/${id}` 会撞。

**补的测试**

- Rust `detected_keeps_unsmoked_drops_cc_switch_and_sorts_by_status`：一个含「已接入 codex 一条 + 待冒烟 pi 四条（跨两个 provider）+ 冒烟失败 pi 一条 + cc-switch 已接入一条」的 store，断言返回顺序是 `codex/gpt -> pi/m-a -> pi/z-a -> pi/z-b -> pi/bad`（已接入在前、待冒烟按 provider 再按 id、冒烟失败垫底）、cc-switch 不出现，并同时断言 `connected()` 仍然只返回那一条已接入。
- vitest `src-ui/src/resources/__tests__/model-picker.test.ts`：10 条，覆盖分组标签（无 provider / 有 provider / provider 全空白）、状态后缀（已接入不加、待冒烟与冒烟失败加、status 为空不显示空括号）、分组顺序与空清单、`modelKey` 不撞。

## 2. 「盖预设」下拉菜单被压住

**哪里** `src-ui/src/components/center/graph/GraphTab.tsx`、`GraphTab.css` 的 `.graph-stamp-menu`

**原来怎样** 菜单是 `.graph-canvas-inner` 里的 `position: absolute; z-index: 6` 元素，按 `top: p.y + NODE_H + 34` 定位。它的容器 `.graph-canvas` 是 `max-height: 44%; overflow: auto` 的滚动容器 —— **菜单一旦超过画布下边沿就被这个容器裁掉**。看起来像「被下方列面板压住」，其实不是层级问题：被 `overflow` 裁掉的内容，z-index 调到多少都没用。

同一个画布里 `.graph-node-actions`（分叉 / 盖预设那一行）也会被裁，但代码里早有两处补偿：`ACTIONS_H = 32` 加进画布内容高度，以及高亮变化时对 `.graph-node-actions` 调 `scrollIntoView`。菜单本身（34px 偏移 + 菜单高度）不在 `ACTIONS_H` 里 —— 这就是只有菜单出问题的原因。

**改成怎样** 选任务书给的第一条路，portal：

- 菜单从 `renderNodeCard` 里拆出来成 `renderStampMenu()`，`createPortal(..., document.body)`。
- 新增 `stampAt` state，在点「盖预设」那一刻用 `e.currentTarget.getBoundingClientRect()` 记下按钮的视口坐标（`left: rect.left, top: rect.bottom + 4`）。
- CSS 从 `position: absolute; z-index: 6` 改成 `position: fixed; z-index: 1200`，加 `max-height: calc(100vh - 32px); overflow-y: auto`。1200 的取值：高于主视图里所有弹层（节点面板遮罩 100、接入页 900），低于设置弹窗 2000 与派发浮层 12000。
- 渲染时再夹一次视口边界（`STAMP_MENU_W` / `STAMP_MENU_MIN_H`），贴着窗口右下角的节点不会把菜单顶出屏幕。
- 加一个 effect：菜单开着时监听 `scroll`（**捕获阶段**，因为滚的是画布不是 window）与 `resize`，触发就收起菜单。fixed 定位是按打开那一刻的坐标钉住的，画布一滚它就和按钮脱节，收起比让它飘着好。
- `setStampFor(null)` 的两处收敛成 `closeStamp()`，同时清掉 `stampAt`。

**为什么选 portal 不选「给画布 overflow: visible」** 画布本身必须保留滚动（图会横向长出去，`.graph-canvas` 的 `overflow: auto` 是功能不是装饰），改成 `visible` 会让整张图溢出到聊天面板上。脱离滚动容器是唯一不牺牲滚动的解法。

**顺手检查的其它绝对定位弹层**

| 弹层 | 结论 |
|---|---|
| `.graph-node-actions`（absolute, z-index 5，同一个画布内） | 已有 `ACTIONS_H` 预留高度 + `scrollIntoView` 两道补偿，未改 |
| `.graph-at-popup`（`@` 弹层，absolute 向上弹，max-height 240px） | 祖先里最近的 `overflow: hidden` 是 `.panel panel-center`，弹层向上生长且高度封顶 240px，正常窗口高度下够用；未改，列进「没改的」 |
| `.np-backdrop` / `.builder`（节点面板、编排构建器） | `NodePanel.tsx` 本来就走 `createPortal` + `position: fixed`，无裁剪 |
| `.explain-backdrop`、`.resources-backdrop`、`.dispatch-overlay`、`.dispatch-collapsed` | 都是 `position: fixed`，不受滚动容器影响 |

## 3. 这次改动带来的一个语义漂移（记录在案）

`src-ui/src/graph/validate.ts` 的 `findModel` 在 `models` 里找不到目标时会打「未接入」提示（图上的节点卡与右栏预览都吃这个）。`models` 现在包含未冒烟条目，所以**「未接入」的含义从「没冒过烟」变成了「本机压根没探测到」**。

这是任务书要的方向（未冒烟改由选项后缀 + G1 决策行提示表达），但它是 `src-ui/src/graph/` 里的逻辑，按约束没动。记在这里，免得以后有人以为节点上不再冒「未接入」是回归。

## 4. 验证（命令与输出原文）

### `cd src-ui && npx vitest run`

```
 RUN  v5.0.0 D:/work/aicross-ui/src-ui


 Test Files  10 passed (10)
      Tests  101 passed (101)
   Start at  10:09:37
   Duration  394ms (transform 63%, import 23%, tests 9%, worker 4%)
```

（TASK-08 时是 9 文件 91 条，新增的 `model-picker.test.ts` 带来 1 文件 10 条。）

### `cd src-ui && npm run build`

```
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...OK 581 modules transformed.
rendering chunks...
computing gzip size...
```

（中间约 100 行分包体积清单略；结尾原文：）

```
dist/assets/wasm-DA6mYL5T.js                      622.32 kB | gzip: 232.09 kB
dist/assets/cpp-Cyj1bR0k.js                       626.13 kB | gzip:  48.10 kB
dist/assets/emacs-lisp-DzXDqNne.js                779.87 kB | gzip: 197.56 kB
dist/assets/main-CUUDkFu-.js                    1,247.36 kB | gzip: 445.33 kB

built in 2.14s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

`581 modules`（TASK-08 时 580）= 新增的 `model-picker.ts`。chunk 体积告警同前，既有问题。

### `cargo test resources`（根目录）

```
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.19s
     Running unittests src\main.rs (target\debug\deps\aicross-3605bca6cf2a3148.exe)

running 22 tests
test resources::tests::live_codex_and_kimi_fact_sources_parse ... ignored, explicit local fact-source check
test resources::tests::live_detect_four_agents ... ignored, 真机验收：探测四个内置 agent（内存 store，不写 resources.json）
test resources::tests::live_smoke_pi_models ... ignored, 真机验收：冒烟两条 pi 模型（2 次模型请求；内存 store，不写盘）
test resources::tests::add_model_requires_provider_for_pi ... ok
test resources::tests::claude_base_url_read_from_settings_env ... ok
test resources::tests::claude_help_without_model_section_falls_back_to_static_table ... ok
test resources::tests::ensure_agents_drops_cc_switch_and_seeds_builtins ... ok
test resources::tests::pi_list_models_is_primary_store_only_supplies_metadata ... ok
test resources::tests::merge_preserves_user_fields_adds_pending_and_marks_missing ... ok
test resources::tests::declared_models_are_not_marked_missing ... ok
test resources::tests::pi_source_matching_uses_exact_tokens_and_auth_uses_top_level_status ... ok
test resources::tests::pi_three_providers_parse_from_list_models ... ok
test resources::tests::kimi_alias_keeps_full_prefix ... ok
test resources::tests::detection_refreshes_meta_preserves_note_and_parses_commented_kimi_toml ... ok
test resources::tests::parses_four_local_fact_sources ... ok
test resources::tests::claude_help_yields_fable_opus_sonnet_plus_haiku ... ok
test resources::tests::connected_filters_status ... ok
test resources::tests::cc_switch_produces_hints_only_no_dispatchable_models ... ok
test resources::tests::safe_summary_redacts_common_cli_secret_shapes ... ok
test resources::tests::detected_keeps_unsmoked_drops_cc_switch_and_sorts_by_status ... ok
test resources::tests::corrupted_store_blocks_writes_and_keeps_bytes ... ok
test resources::tests::smoke_agent_models_partial_failure_writes_each_result ... ok

test result: ok. 19 passed; 0 failed; 3 ignored; 0 measured; 147 filtered out; finished in 0.04s
```

另外跑了一次 `cargo check`（确认改了 `orchestration.rs` 的调用点后全仓能过）：

```
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.76s
```

**没有实机看过的部分**：同 TASK-08，不起窗口，portal 菜单的实际落点、47 条模型在 240px 宽右栏里的下拉观感都没有渲染核对过。

## 5. TASK-08b 没改的

1. **节点面板选模型时不写 `provider`。** `TargetEditor` 的 `<option value>` 仍是 `agent/id`，provider 由下面单独一行填。现在下拉里能看到 `pi · zai-coding-cn` 这样的分组标签了，但选中后 Target 的 provider 还是空的。补上是个明显的顺手改进，但它会改变 `TargetEditor` 的写回行为（进而影响 `routeLine` 与派发 argv 的 provider 段），属于「改派发相关行为」，不看窗口不该顺手做。`DispatchOverlay` 里的状态查找已经做了「provider 匹配不上就退到 agent+id」的兜底，所以至少提示不会漏。

2. **47 条模型的下拉没有搜索/过滤。** `@` 弹层本来就按输入过滤，节点面板的原生 `<select>` 没有。原生 select 支持键盘首字母跳转，`optgroup` 分组后也还能扫，够用；换成自绘可搜下拉是新交互，不在本期。

3. **`.graph-at-popup` 没有一并 portal。** 见上表：它向上弹且 `max-height: 240px`，在正常窗口高度下不会碰到 `.panel-center` 的 `overflow: hidden`。真要在极矮窗口下复现才值得改，需要开窗口。

4. **`DispatchOverlay` 的目标下拉仍是平铺的 47 项**（只加了状态后缀，没有 `optgroup`）。那颗 select 的 value 是 `[agent, provider, id]` 三元组，和节点面板的两元组不是一套，套同一个分组函数要先统一 value 形状，改动面比收益大。

5. **TASK-08 那份「没改的」清单（焦点环、`transition: all`、`prefers-reduced-motion` 等 8 条）原样有效**，本次没有触及。

## 6. 追加（09-14 批准后续）：节点面板选模型时把 provider 一起写回

**哪里** `src-ui/src/resources/model-picker.ts`（新增 `modelSelectValue` / `targetSelectValue` / `parseModelSelectValue`）、`src-ui/src/components/right/orchestration/NodePanel.tsx` 的 `TargetEditor`

**原来怎样** 模型下拉的 option value 是 `${agent}/${id}`，选中时只 `patch({ agent, model })`，provider 完全没进这条路径 —— 它只能靠下面那行「provider」输入框手填。pi 一家挂三个 provider（本机 deepseek / qwen-token-plan-cn / zai-coding-cn），从下拉里选一条 pi 模型，Target 的 provider 就是空的。

**实际后果比预想的好一点，但仍然是 bug**：`src/channels/mod.rs:229-232` 对 pi 有硬校验 ——

```rust
"pi" => {
    if provider.is_empty() {
        return Err("pi 通道缺少 provider".into());
    }
```

所以不是静默派到错的 provider，而是**派发直接失败，报「pi 通道缺少 provider」**。另外 `routeLine` 查模型用的是 agent + id + provider 精确匹配，provider 空着时 pi 的条目查不到，G1 决策行的档位会退成「未分档」。两个症状同源。

**改成怎样**

- option value 从 `agent/id` 换成 `JSON.stringify([agent, provider, id])` 三元组。用 JSON 不用拼字符串有两个原因：kimi 的模型 id 自带斜杠（`kimi-code/k3-256k`），拼出来切不回去；而且 `DispatchOverlay` 的目标下拉本来就是这个形状，两边对齐。
- `onChange` 改成 `parseModelSelectValue(v)` → `patch({ agent, provider, model })` 一次写三个字段。provider 空串写回 `undefined`（Target 里 `provider` 就是「留空按通道默认」的语义，和手填框的 `value === '' ? undefined` 一致）。
- 「未指定模型」那一项现在也清 provider（`patch({ agent: '', model: '', provider: undefined })`），否则换成未指定后会残留上一次的 provider。
- **老图兼容**：已存的 Target 可能只有 agent + model 没有 provider，新的 value 形状在清单里对不上任何一项，原生 `<select>` 会显示空白。加了一条「当前值」option 如实显示它现在是什么（`pi/glm-5.3-flash`，同 `DispatchOverlay` 对未列出目标的做法），用户重选一次即绑上 provider。**没有做静默回填**——按 agent+id 猜一个 provider 塞进去，会让界面显示的和图里存的不一致，且是在用户没点过的情况下改了图。
- 手动输入路径（`__manual` + 两个手填框 + 单独的 provider 行）一行未动。
- `routeLine` / `graph/dispatch.ts` / `channels/` 一行未动——它们本来就读 `target.provider`，这次是把值补上，不是改读法。

**为什么** 下拉是用户挑模型的主路径，分组标签上已经写着 `pi · zai-coding-cn` 了，选中却不带走 provider，是界面显示和写回不一致。

**补的 vitest**（`model-picker.test.ts` 新增 8 条，共 18 条）：用一个 `select(target, value)` 小函数模拟 `TargetEditor` 的 onChange（`parseModelSelectValue` → 展开进 Target），覆盖：

- 选中 `pi · zai-coding-cn` 的模型后 `target.provider === 'zai-coding-cn'`，且 `role` 等其它字段不动；
- 同 id 不同 provider（`pi/zai-coding-cn/glm-4.6` 与 `pi/qwen-token-plan-cn/glm-4.6`）value 不同、写回的 provider 也不同，不会混；
- 换 provider 时旧值被覆盖不残留；
- 没有 provider 的 agent（claude）写回 `undefined` 而不是空串；
- 模型 id 自带斜杠（`kimi-code/k3-256k`）能原样切回来；
- `targetSelectValue(select(...)) === modelSelectValue(m)`，保证选完能回选上；
- 老图 Target（无 provider）确实对不上清单里带 provider 的那条——这条测试是「当前值」option 存在的理由；
- 坏 value（空串、`__manual`、旧的 `pi/glm-4.6` 形状、二元组、对象、非字符串元素）一律返回 null，调用侧原样不动。

### 验证

`cd src-ui && npx vitest run`

```
 RUN  v5.0.0 D:/work/aicross-ui/src-ui


 Test Files  10 passed (10)
      Tests  109 passed (109)
   Start at  10:14:36
   Duration  385ms (transform 69%, import 19%, tests 8%, worker 3%)
```

（109 = 08b 的 101 + 本次 8 条。）

`cd src-ui && npm run build`

```
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...OK 581 modules transformed.
rendering chunks...
computing gzip size...
...
dist/assets/cpp-Cyj1bR0k.js                       626.13 kB | gzip:  48.10 kB
dist/assets/emacs-lisp-DzXDqNne.js                779.87 kB | gzip: 197.56 kB
dist/assets/main-CtF7buzp.js                    1,247.80 kB | gzip: 445.49 kB

built in 2.27s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

`cargo test resources` 本次没有改 Rust，未重跑（上一节的结果仍然有效）。

**没有实机看过的部分**：不起窗口，所以「当前值」option 在老图上的实际显示、以及 47 条模型分 6 组后原生 select 的观感都没有渲染核对。

### 这条改动后仍然没改的

- **`PresetBuilder` 里的 Target 也走同一个 `TargetEditor`**，所以内置/用户预设里的节点现在也能选到带 provider 的模型。但**已存的 `presets.json` 与 `graphs/*.json` 里的老 Target 不会被自动补 provider** ——它们要么显示「当前值」option 等用户重选，要么在 pi 上派发时仍然撞「pi 通道缺少 provider」。要批量回填得写迁移，那是另一件事。
- **内置预设这条查清了，不用改**：`builtin_presets()`（`src/orchestration.rs:232`）里的 Target 全是 `blank_target()` 打底，只写 role / tier / thinking / visibility / write，agent 与 model 留空；具体模型由 `fill_role_defaults(&mut presets, &roles_from(root), &models)` 从设置页的角色默认值填。而 `RoleProfile` 本来就有 `provider` 字段，设置页那颗下拉的 value 也一直是 `[agent, provider, model]` 三元组 —— 所以走「角色默认值」这条路进来的 Target 一直带 provider，出问题的只有节点面板手选这一条路径，也就是本次修的这条。
