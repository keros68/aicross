# TASK-09b 审查

## 1. 需求完整性

契约与实现 1–6 逐条核对：

- **契约（resolved_id）**：`src/resources.rs:108-109` 字段带 `#[serde(default)]`；`src/resources.rs:296-301` 别名表四条照任务书；`resolved_id_for`（:304）非别名原样返回。✅
- **旧库补齐只填空**：`ensure_resolved_id`（:331-345）只在空值时落表，非空不动；`load()`（:244）与 `ensure_agents()`（:512）两条入口都挂了，且命令层唯一读路径 `load_or_default`（:370）走 `load`，没有漏网入口。测试 `task09b_legacy_store_backfills_resolved_id`（:2768）钉住了「库里已有的真身优先于静态表」。✅
- **真身覆盖与失败不抹**：`apply_smoke_outcome`（:1511-1516）空真身不覆盖、失败分支（:1523-1534）完全不碰 `resolved_id`；`merge_detected_models`（:1219-1222）重探测不回退静态表；首派验证 `verify_on_first_use_at`（:1729）复用同一个 `apply_smoke_outcome`，覆盖一并生效。三条各有测试（:2800、:2860）。✅
- **实现 1–6**：图标 ✅、Claude 别名+真身 ✅（`types.ts:170` 相同返回空串）、分组/折叠/单点 ✅、字体密度 ✅、接入页同步 ✅（同一 `providerGroups`/`AgentBadge`）、Rust 四条 + TS 各条测试 ✅。

**发现一处口径缺口（中等偏低）**：

- `src/resources.rs:1561-1592` + `ResourcesOverlay.tsx:324,420,458` — **原生 claude 没有任何真身覆盖路径**。原生模型不给「验证」按钮、首派验证跳过 native（`verification != unverified` 即 Skipped），而 `apply_smoke_outcome` 只在冒烟里被调。结果是：claude 原生订阅下 `fable→claude-fable-5-1` 这张静态表是永久事实源，Anthropic 改名后界面会一直显示过时真身，且没有观察纠正的手段。PRODUCT.md:236 写「冒烟 / 首次派发核对到响应体 model 后以实测值覆盖」，对原生 claude 实际不可达——文档口径与可达路径不符。**为什么**：别名表自己也承认「会漂移」（resources.rs:293-295），漂了却修不回来。**建议**：本期至少在 PRODUCT.md/MANUAL 把这句收窄为「第三方端点（含挂 `ANTHROPIC_BASE_URL` 的 claude）核对后覆盖；原生端点的真身以静态表为准」；后续期可给原生行留一个手动「核对」动作。

## 2. 逻辑正确性

- **分组键与排序**：`types.ts:205-217` 按 provider 名 `localeCompare` 排序、组内保留输入序，输入序由 `modelsByAgent`（:144-156，验证档→provider→id）决定，纯函数、可复现。稳定 ✅。（`localeCompare` 不带 locale 参数依赖运行环境 ICU，跨机器顺序可能不同——单机桌面工具，不算问题，不记。）
- **折叠状态不串**：`types.ts:241 groupKey = agentId/provider`，`ResourcesTab.tsx:42,160` 状态键与折叠键一致；store 重载（RESOURCES_CHANGED_EVENT）不动 `openUnverified`。同名 provider 挂两个 agent 有纯函数测试（resources.test.ts:156）。✅
- **单 provider 不出组头**：`types.ts:211-213` `groups.size <= 1` 返回无名组，`ResourcesTab.tsx:198` 与 `ResourcesOverlay.tsx:339` 都按 `provider !== ''` 抑制组头，有测试。✅
- **@ 引用带 provider**：`ResourcesTab.tsx:267` 传 `model.provider` → `mention.ts:14` 补 `#provider` → `parseSend` 切回，`layout.test.ts:213-227` 两头都钉了。✅
- **resolved_id == id 不重复**：`modelSecondaryId`（types.ts:170-173）返回空串不渲染，左右两栏一致，测试在 resources.test.ts:189 与 tab-09.test.tsx:165。✅
- **pi 真身取半段的边界**：`resolved_id_from_identity`（resources.rs:318-327）`strip_prefix` 是**大小写敏感**的——真身 `Zai/glm-4.7`、provider `zai` 时剥不动，`resolved_id` 落成整串 `Zai/glm-4.7`（含斜杠），与组内「不带前缀」的显示口径不一致；真身不带前缀时同理保留整串。不丢数据、只在悬停/次文本显示，**低**。建议：strip 前两边都 `eq_ignore_ascii_case` 化（或先 `to_ascii_lowercase` 比对再切原串），并给这个函数补直测（现在只经 `apply_smoke_outcome` 间接测到 happy path）。

未发现其它逻辑问题。

## 3. 安全与许可

- **来源与许可**：`brand-marks.ts:1-16` 文件头写明 simple-icons、CC0-1.0、逐条 tag 与读取日期，OpenAI 一条把 #13944 撤出事件写清了，并留了「删一条即退回字母」的退路。做法规范。✅
- **无脚本、无运行时依赖**：路径数据是纯 `d` 字符串常量，React 渲染为 `<path d>`，无 `dangerouslySetInnerHTML`、无 `<script>` 可能；`package.json` 未动。✅
- **OpenAI 标志风险评估**：CC0 解决的是路径数据的**版权**，#13944 撤下的是**商标许可**问题。本产品是私用桌面工具，标志只作「这一行是哪家 CLI」的指称性使用，不做背书、不做自身品牌——指称性使用成立，实际风险≈0。条件：**一旦对外分发安装包**（MANUAL.md 已有安装包命名），性质就从私用指称滑向「分发他人商标的界面资产」，OpenAI 又是明确拒绝收录的那家。**建议：现在保留**（代码已留一行删除即兜底的退路，`brand-marks.ts:13-14`），把「对外分发前删 `BRAND_MARKS.codex` 或换文字标签」记进发布前检查；其它三家（Anthropic、Kimi 在 master 上）无此问题。

## 4. 视觉纪律与可访问性

- **对比度**：浅色主题四家压暗色齐（resources.css:46-60），报告给了实算比值；绿点浅色主题有专用 `--verify-green #1f7a3d`（:68-70）。**发现（低，报告已自曝）**：深色主题 kimi `#6E56CF` 对 `#202020` ≈3.0:1 贴线。18px 装饰性标志、旁边就是 agent 名，1.4.11 不强制，可接受；真机看暗就把 `resources.css` 深色那一套也压一档。
- **焦点环**：agent 行、模型行（resources.css:200-204）、折叠行（:341-344）三处 `:focus-visible` 2px 不透明强调色，有 CSS 断言。✅
- **aria**：状态点 `role="img"` + `aria-label` + `title`（VerificationMark.tsx:27-33）；悬停文本同时挂 `aria-describedby` 指向 sr-only 节点（ResourcesTab.tsx:254-276）补 WebView2 title 不响应键盘的洞；折叠按钮带 `aria-expanded`。✅ **两个小问题**：
  - ResourcesTab.tsx:254-257 — `hintId` 由 agent/provider/id 消毒拼接，消毒会把不同 id 撞成同一 ID（如含空格与含连字符的同形 id），产生重复 DOM ID。无害但无效 HTML。建议：加一个每行递增序号或用 `useId`。
  - 模型行 `disabled`（无对话图时）整行退出 tab 序，键盘用户连提示都读不到——现有交互下可接受，不记为缺陷。
- **字号/间距**：最小 11px（组头、真身、折叠行），CSS 断言封了 9px；左栏行 `4px 8px`/`gap 8px`/`line-height 16px`，4 的倍数断言扩到了新选择器。✅
- **胶囊只给异常**：`.verify-pill` 只在 failed 且 showLabel 时渲染（VerificationMark.tsx:35-36），左栏零胶囊。✅

## 5. 代码质量与契约

- **只追加**：Rust 侧报告为 209 insertions / 0 deletions（我无法在本环境跑 git 复核，但通读 `src/resources.rs` 改动段均为新增函数/字段，未见删改）；旧字段全部保留，serde default 兼容旧库。✅
- **死代码清理**：`ambiguousModelIds` / `modelRowLabel` 全仓无残留引用（grep 证实），随测试一并删净。✅
- **2–8 期路径**：model-picker 逻辑未动；DispatchOverlay 未动；resources.css 共享部分只有新增规则与 `.res-btn` 度量（R1 轮既改），无回归迹象。`ResourcesTab.tsx:68` 的 `react-hooks/set-state-in-effect` 是 R1 前既有 lint error，与本轮无关（报告如实记录）。✅
- **测试真实且断言目标行为**：Rust 四条（:2751、:2767、:2799、:2859）逐条打在「映射/补齐不覆盖/覆盖且失败不抹/重探测不回退」上；TS 369 条中本轮新增的纯函数断言与快照式 markup 断言（tab-09.test.tsx:133-195、overlay-09b.test.tsx）都是行为级，不是同义反复。✅

**最该补的 3 个测试**：
1. `add_model` 手动录入 claude 别名落 `resolved_id`（resources.rs:1286 这条路径零测试；误删该行现有测试全绿）。
2. `resolved_id_from_identity` 直测边界：provider 大小写不一致、真身无 provider 前缀（保留整串的现行为钉下来，或修复后钉新行为）。
3. 组件级折叠隔离：两个 agent 挂同名 provider，展开一个的未验证、断言另一个的折叠行文案与隐藏行不串（现在只有 `groupKey` 纯函数断言，没有走一遍渲染路径）。

---

## 是否可交付

**可交付**——功能、契约、测试、许可注明全部到位，OpenAI 标志按私用口径保留（已留删除退路）；缺项均不阻塞本分支合入 main。缺项按严重度：

1. 原生 claude 无真身覆盖路径，静态别名表漂移后无法被观察纠正；文档「冒烟/首派覆盖」口径对原生不可达——先改文档口径（中低）。
2. `resolved_id_from_identity` 对 provider 大小写不一致/无前缀真身保留整串，显示口径不一致；缺直测（低）。
3. OpenAI 商标：对外分发安装包前须删 `BRAND_MARKS.codex` 或换标签——记入发布前检查（低，条件性）。
4. kimi 徽标深色 3.0:1 贴线，真机看暗再压（低）。
5. `hintId` 消毒可撞出重复 DOM ID；接入页 md 徽标 20px 与任务书「18px」的偏差；`BrandMark.title` 字段注释称进 aria-label 实则从未使用（三者皆极低，顺手修）。