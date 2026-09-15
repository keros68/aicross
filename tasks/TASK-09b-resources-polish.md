# TASK-09b 资源页第二轮（09-15 用户反馈：官方图标、Claude 模型 ID、长列表整理）

依据 `docs/PRODUCT.md`「视觉纪律」「资源冒烟口径」。在 main（含 TASK-09）之上做，前端为主；Rust 只允许改 `src/resources.rs` 里 Claude 别名→模型 ID 的映射与 `ResourceModel` 追加字段。

## 契约（只追加）

- `ResourceModel` 追加 `resolved_id: string`（Rust `#[serde(default)]`）：别名对应的真实模型 ID。Claude 按已知表填（`fable→claude-fable-5-1`、`opus→claude-opus-5`、`sonnet→claude-sonnet-5`、`haiku→claude-haiku-4-5-20251001`），冒烟/派发拿到 `modelUsage` 键后以真身覆盖；非别名模型 `resolved_id == id`。

## 实现

1. **官方图标**：`AgentBadge` 改为品牌标志 SVG（内联、单色 currentColor 或官方色，深浅主题各一套）：Claude → Anthropic 标志；Codex → OpenAI 标志；Kimi → Moonshot/Kimi 标志；pi → π 字形。SVG 路径用 simple-icons 的开源路径数据（在文件头注明来源与许可），尺寸 18px，与名称基线对齐；徽标不再用字母。
2. **Claude 模型行显示真实 ID**：行内主文本别名（`fable`），次文本弱化显示 `resolved_id`；别名与 ID 相同的模型只显示一次；悬停仍给来源。
3. **长列表整理**：pi 这类多 provider 的 agent 下按 provider 分小组（小组标题 = provider 名 + 计数），组内模型名不再带 `· provider` 后缀；每个小组内「未验证」的模型默认折叠成一行「未验证 N 个 · 展开」，已验证/失败的常显；原生 agent 无小组。状态只留一个点（verified 绿、failed 红、unverified 灰），不再跟文字，文字进悬停/`aria-label`。
4. **字体与密度**：模型名用界面正文字体（不用等宽）；行高与间距按 4 的倍数；agent 行版本号弱化保持。
5. **接入页同步**：同一套品牌图标与分组；Claude 卡片显示别名 + 真实 ID。
6. **测试**：`resolved_id` 映射与覆盖（Rust）；分组/折叠/状态点纯函数与快照式字符串断言（TS）。

验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行；不动 `src/channels/`、`src/dispatch.rs`；不起窗口、不发真机请求、不 commit、不 push。报告写 `tasks/REPORT-09b.md`。
