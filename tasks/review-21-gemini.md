# TASK-21 盲审 · Antigravity / Gemini 3.8 Flash (High)

2026-09-16。材料全部内联（任务书 + 实现报告 + 完整 diff，约 3.6 万字符），prompt 明写不使用任何工具。
用量：65,692 token / 166.7 秒 / 订阅额度。**流在末尾被打断过一次**（result.error = stream was interrupted），四条问题与核过要点都完整。

---

总判：**可交付但有未闭合项**

---

### 问题清单（按严重度排序）

#### 1. 【未闭合】重判留痕写入 `note`，但在悬停与详情构造函数中被完全丢弃，导致翻案留痕 UI 不可见且旧备注丢失
- **文件:行**：`src/resources.rs:560-563`、`src-ui/src/resources/types.ts:425-444` (`modelDetailLines`)、`src-ui/src/components/resources/ResourcesOverlay.tsx:336-347` (`AgentCardModelMeta`)
- **问题**：任务书第 4 条明确要求：“命中已知路由别名的，翻成已验证……并在来源/备注里留一句「按已知路由别名重判（日期）」”。Rust 端将其写入了 `model.note`。但在前端，`modelDetailLines` 为对齐第 2 条的固定可选行，未将 `model.note` 纳入，同时删除了原 `AgentCardModelMeta` 中的 `{model.note}` 和 `{model.meta}`；测试甚至将 `expect(markup).not.toContain('备注：手动加的')` 作为断言。这导致：
  1. 翻案留痕在界面（左栏悬停及接入页详情）彻底不可见，用户无法感知翻案依据；
  2. 存量模型的人工备注和未在事实源中的丢失标记（`MISSING_MARKER`）被静默隐藏。
- **复现条件**：加载带旧失败记录的库，命中重判后，在左栏或接入页查看该模型悬停详情，看不到任何重判备注信息。
- **建议改法**：二选一：
  1. 在 `modelDetailLines` 可选行末尾恢复可选的 `备注：${model.note}`；
  2. 若详情行严格不允许出现“备注”，则依据任务书第 4 条的“来源/备注”选项，在 Rust 重判时将「按已知路由别名重判（日期）」追加到 `model.source` 中。

#### 2. 【缺陷】`normalizeModelSource` 缺乏幂等性保护，二次调用或空值会产生重复前缀与日期
- **文件:行**：`src-ui/src/resources/types.ts:243-261` (`normalizeModelSource`)
- **问题**：当输入为空时返回 `'来源：未记录 · 日期未知'`。因为 `'日期未知'` 不匹配 `current` 正则的 `\d{4}-\d{2}-\d{2}`，若将此返回值再次传入（或库中已存在该值），会直接穿透至末尾兜底分支，返回 `'来源：来源：未记录 · 日期未知 · 日期未知'`。对任何以 `'来源：'` 开头但无标准日期的字符串均存在此重复拼接问题，不满足纯函数幂等性。
- **复现条件**：调用 `normalizeModelSource(normalizeModelSource(""))`，输出为 `'来源：来源：未记录 · 日期未知 · 日期未知'`。
- **建议改法**：在函数头部或兜底逻辑前判断 `value.startsWith('来源：')`，对已包含该前缀的字符串直接剥离前缀处理或原样返回。

#### 3. 【缺陷】Rust 端 `shorten_home_path` 强制要求尾随斜杠，与前端缩写逻辑不一致
- **文件:行**：`src/resources.rs:1532-1543` (`shorten_home_path`)、`src-ui/src/resources/types.ts:235-239` (`shortenHomePath`)
- **问题**：前端正则通过 `(?=[\\/]|$)` 同时支持目录后带斜杠或位于字符串末尾；Rust 端正则 `r"(?i)[A-Z]:[\\/]Users[\\/][^\\/]+([\\/])"` 强制捕获后续的斜杠 `([\\/])`。若传入无尾随斜杠的用户主目录路径，Rust 端将无法替换。
- **复现条件**：在 Rust 端执行 `shorten_home_path(r"C:\Users\alice")`，返回仍为原串而非 `~`。
- **建议改法**：将 Rust 的正则调整为兼容行尾 `$ ` 或保持与前端断言一致。

#### 4. 【遗漏】左栏 `ResourcesTab` 模型名去同义前缀缺少组件级接线测试
- **文件:行**：`src-ui/src/components/resources/ResourcesTab.tsx:440`、`src-ui/src/resources/__tests__/resources-tab-09.test.tsx`
- **问题**：`ResourcesTab.tsx` 行内模型名已接线为 `{modelDisplayName(model)}`，但在 `resources-tab-09.test.tsx` 中仅测试了版本号提取，未补充断言左栏遇到类似 `kimi-code/k3-256k` 时渲染为 `k3-256k` 的接线测试（该接线目前仅在 overlay 测试中覆盖）。
- **复现条件**：检查 `resources-tab-09.test.tsx` 的 diff，缺乏左栏同义前缀剥除的 DOM 断言。
- **建议改法**：在 `resources-tab-09.test.tsx` 中增加一个带厂商同义前缀的模型 fixture，断言左栏渲染结果不含前缀。

---

### 核过没问题的要点

1. **零真实模型请求约束**：`rejudge_stored_smoke_failures` 仅在 `load` 阶段基于内存正则和静态路由表匹配，无进程启动、无 CLI 探测、无网络请求，符合少用计费模型的硬约束。
2. **重判条件严格**：严格比对 `requested_provider`、`requested_model`、`actual_provider`、`actual_model` 及 `model.provider` 和 `model.id`，未命中路由表的记录严格保持在 `failed`，未放宽翻案条件。
3. **状态分类彻底解耦**：未命中重判的记录和新冒烟失败均明确记为 `failed` / `冒烟失败`；`merge_detected_models` 补充了 `model.verification.clone_from(&existing.verification)`，避免重新探测时重判结果被重置为 `unverified`；`unverifiedCount` 口径保持不变。
4. **版本号纯函数与回退机制**：`cliVersionNumber` 覆盖 Claude Code、Codex、Kimi、Pi 四家真实输出，未命中时原样回退；左栏与接入页均显示精简版本号，并通过 `title` 属性完整保留原始输出证据。
5. **前四行固定与多处统一**：`modelDetailLines` 作为左栏与卡片详情的唯一构造器，强制固定输出名字、模型 ID（即使与名字相同）、状态、来源；行内模型名通过 `modelDisplayName` 通用去除同义厂商前缀，未硬编码厂商名。
6. **三种老来源串显示层归一**：`normalizeModelSource` 覆盖了 `读:<绝对路径> (RFC3339)`、`claude --help 别名 (日期)` 和 `申报(日期)`，主目录均缩短为 `~`；Rust 探测合并逻辑同步兼容新老申报前缀，未破坏存量未探测标记。
