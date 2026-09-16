# fix-19-r1 左栏行统一与界面缩放（TASK-19）复审修正轮

依据 GLM-5.3-Flash 与 DeepSeek 对 ui/phase19 的盲审（`tasks/review-19-glm.md`、`tasks/review-19-ds.md`）。契约只追加。

## 必修（两家共同：1、6、7；DeepSeek 独有：2、3、5、8；GLM 独有：9、10、11）

1. **接入页丢了「额度用尽至 HH:MM」（DeepSeek 1 / GLM 1）**：行统一把行内额度文案删了，但详情里没补，只剩悬停；`docs/MANUAL.md` 与 REPORT-14 都把它记作接入页可见状态。修法：`AgentCardModelMeta` 详情里补「额度用尽至 HH:MM」与「状态：已验证 / 未验证 / 验证失败 / 原生」两行（行内仍只留一个点）；删掉已无渲染点的死规则 `.agent-card-model-quota`。测试：给未来的 `quota_exhausted_until` → 详情里出现该时间。
2. **接入页「未验证 N 个 · 展开」没有样式（DeepSeek 2）**：`.agent-card-unverified-toggle` 全仓无选择器，按浏览器默认按钮渲染。修法：与 `.resource-unverified-toggle` 合并成一条规则（含 `:focus-visible`）；补一条 CSS 断言。
3. **CSS zoom 回退不幂等（DeepSeek 3）**：webview 缩放成功过一次后再走回退，会在残留的 webview 倍率上再叠根元素 zoom（150% 后回退到 125% 实际约 1.875 倍）。修法：进回退分支前把 webview zoom 归 1；记住当前生效模式再切换。测试：「成功 → 失败 → 成功」序列断言最终只有一处倍率生效。
4. **缩放与页面布局的真机核对（DeepSeek 3 后半）**：`.resources-page` 用 `100vh` + `position: fixed`，根元素 CSS zoom 下底部可能被裁。本项由编排者在窗口里过 90 / 100 / 150 三档确认（接入页、左栏、终端各看一眼），实现者只需保证回退路径可用并在报告里标注待窗口核。
5. **关掉对话图时悬停信息被整段替换（DeepSeek 4）**：`ResourcesTab.tsx` 的 `hint = graphOpen ? modelHint(model) : '先打开对话图'` 把真实 ID / provider / 验证时间全吃掉了。修法：两段拼起来（`先打开对话图` + 换行 + `modelHint(model)`）。
6. **11px 残留（DeepSeek 5 / GLM 7）**：设置页（本期新增控件所在页）的 `SettingsModal.css` 六处 11px 一并升 12；`CenterPanel.css` 等非对话图页与 `global.css` 的残留，要么一起升、要么在报告「没做」里逐条列明并把 `docs/PRODUCT.md:88` 的适用范围写清楚（终端与 `pre` 不动）。
7. **文案与术语（DeepSeek 6 / GLM 5）**：设置页与手册的快捷键写法统一为 `Ctrl + = / - / 0`；`docs/MANUAL.md` 术语表补「界面缩放」一行。
8. **测试补课（DeepSeek 7、8）**：`updateUiScale` / `initializeUiScale` 加注入 storage + spy apply 的行为测试（持久化键、事件 detail、apply 调用），替掉只做字符串包含的接线断言；快捷键的基准值改为内存当前值（存储不可用时也能从当前档递进）；`resources-overlay-09b.test.tsx` 头注释与 `rail-density-19.test.tsx:47` 绑旧类名的断言一并更新。

9. **快捷键与 WebView2 原生加速键的交互（GLM 2）**：页面上的 `preventDefault()` 拦不住 WebView2 在页面事件前处理的浏览器加速键，Ctrl + = 可能原生缩放与 `setZoom` 双轨触发。修法：Rust 侧关掉 browser accelerator keys（webview 设置），或在应用缩放后回读实际倍率与存档对账；编排者另在窗口里按一轮 Ctrl + = / - / 0 核对。
10. **冷启动一帧 100% 闪烁（GLM 6）**：`initializeUiScale()` 挂在 `useEffect` 后，非 100% 档首帧是 100%。修法：模块加载时或首个布局前应用。
11. **文档与已知限制（GLM 3、4）**：`docs/PRODUCT.md` 的四条定案编号乱序（1、2、3、5、4），换回递增；多窗口缩放不联动（detached 窗口要重启才跟随）记进报告的已知限制；`Explorer.css` 的 `12.5px` 若非本期引入则记 backlog。

## 保持

- 四家行结构一致、原生无状态点、TASK-17 的 provider 分组与折叠、五档缩放与最小权限（只 `allow-set-webview-zoom`）——核过没问题，不动。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；跑 `scripts/copy-audit.py`。保持文件原有换行；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-19-R1.md`（逐条对应本清单）。
