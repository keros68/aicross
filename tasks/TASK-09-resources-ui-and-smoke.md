# TASK-09 资源页美化与冒烟口径（09-15，用户提出）

依据 `docs/PRODUCT.md`「资源冒烟口径（09-15 定案）」。一个工作树，一位实现者（Claude 子代理，带 web-design-guidelines / design-taste-frontend 两个 skill），Rust 与前端都改，但 Rust 只动 `src/resources.rs`、`src/dispatch.rs` 里冒烟相关的少量行、`src/server.rs` 注册。

## 契约（只追加）

- `ResourceModel` 追加 `verification: "native" | "unverified" | "verified" | "failed"`（Rust `#[serde(default)]`，旧库缺字段按 agent 推断：claude/codex/kimi → native，其余 → 按旧 `status` 映射：已接入 → verified、冒烟失败 → failed、待冒烟 → unverified）。旧字段 `status`/`smoked_at`/`smoke_model`/`smoke_failure` 保留不删。
- 引擎：`run_one` 在派发前对 `verification == "unverified"` 的目标先调 `resources::verify_on_first_use(agent, provider, model)`（真发短请求，比对真身，沿用现有 `smoke_resource_model` 的判据），写回资源库；失败 → 该节点直接 `status: error`，`error` 写「首次验证失败：…」，`runner_note` 记「首次派发自动验证」；通过 → 继续派发并在 `runner_note` 记「首次派发已验证真身」。
- Tauri 命令：现有 `smoke_resource_model` 保留（手动重验），`list_dispatchable_models`/资源读取返回新增字段。

## 实现

1. **Rust**：`verification` 字段与推断；`verify_on_first_use`（复用冒烟函数，只对第三方生效，原生直接返回 native）；`run_one` 接线；测试：旧库映射三态 + 原生不冒烟 + 第三方首次派发失败即 error（stub 端点）+ 通过后第二次不再验证。
2. **左栏资源 tab**（`ResourcesTab.tsx` + `resources.css`）：列全部探测模型；agent 行 = 图标徽标（`src-ui/src/components/resources/AgentBadge.tsx`：圆角方形、1–2 个字母、各家色相：Claude 暖橙、Codex 石墨/青、Kimi 靛蓝、pi 苔绿，深浅主题各一套）+ 名称 + 弱化版本号；模型行 = 模型名 + 右侧状态点/胶囊（native 无点；unverified 灰点「未验证」；verified 绿点；failed 用现有异常胶囊「验证失败」）；来源与时间挪到 `title` 悬停；底部链接改「管理接入 →」。视觉纪律见 PRODUCT.md「视觉纪律」：胶囊只给异常状态。
3. **接入页**（`ResourcesOverlay.tsx`）：同一套徽标；原生模型不再显示「冒烟」按钮，第三方显示「验证」（手动）与三态；「N 个待冒烟」统计改成「N 个未验证（首次派发时自动验证）」。
4. **派发下拉与 G1**：模型清单后缀按三态；unverified 可选并在 G1 提示「首次派发会先验证真身」。
5. **测试**：前端纯函数 `verificationOf(model)`/分组排序/徽标映射各一条；快照式字符串断言 ResourcesTab 不再渲染来源路径正文。
6. `docs/MANUAL.md` 资源一节改口径。

验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。不跑 `cargo fmt`、保持文件原有换行（`server.rs` CRLF、其余 Rust LF；前端多为 CRLF，`tauri.ts` 混排用字节级改）；不动 `src/channels/`；不起窗口、不发真机请求、不 commit、不 push。报告写 `tasks/REPORT-09.md`。
