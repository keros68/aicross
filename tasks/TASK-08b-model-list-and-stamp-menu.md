# TASK-08b 节点面板模型清单 + 盖预设菜单被压盖

工作树 `D:\work\aicross-ui`，分支 `ui/polish`（已并入 main，继续在这条分支上改）。约束同 TASK-08：不动 `src/dispatch.rs`、`src/channels/`、`src-ui/src/graph/` 的派发逻辑；不 commit、不 push、不起窗口。报告追加到 `tasks/REPORT-08.md`「TASK-08b」一节，验证 `cd src-ui && npx vitest run`、`npm run build`、`cargo test resources`，命令与输出原文贴进去。

## 用户 09-14 指出的两处（截图）

1. **右栏节点面板「模型」下拉只有 4 项**（claude/fable、codex/gpt-5.6-luna、kimi/kimi-code/k3-256k、pi/glm-5.3-flash）。这是因为 `src/resources.rs` `list_dispatchable_models` 只返回「已接入」（冒烟通过）的模型；本机 pi 一家就有 3 个 provider 共 31 个模型，用户要的是**把探测到的都列出来**。改法：
   - `list_dispatchable_models` 返回所有已探测到的模型（`待冒烟`、`已接入`、`冒烟失败` 都给，cc-switch 仍排除），`DispatchableModel` 追加 `status: String` 字段（`src-ui/src/graph/types.ts` 同步）。
   - 排序：已接入在前，然后待冒烟，冒烟失败最后；同状态内按 agent → provider → id。
   - 前端 `NodePanel.tsx` 的 `<optgroup>` 标签改为 `agent` 或 `agent · provider`（pi 这种多 provider 的 agent 按 provider 分组），选项文本用 `id` 并在未冒烟/失败的后面加「（待冒烟）」「（冒烟失败）」后缀；`@` 弹层（GraphTab 里用同一份 `models` 的地方）同样处理。左栏资源 tab 保持只显示已接入（那是「可派发清单」的语义，别动）。
   - 选择未冒烟模型可以派（派发时通道自己会核真身），但 G1 决策行的理由里要能看出「该模型未冒烟」——`DispatchOverlay.tsx` 用 `status` 显示一条提示即可，不改派发逻辑。
   - 补 Rust 测试：`list_dispatchable_models` 对「一已接入一待冒烟一 cc-switch 已接入」的 store 返回前两条且顺序正确；vitest 测分组标签函数。

2. **「盖预设」下拉菜单被下方列面板压住**（截图：点节点下的「盖预设」后弹出的 `graph-stamp-menu` 只露出上半截，下半被列区域盖掉）。`GraphTab.css` 里 `.graph-stamp-menu { position:absolute; z-index:6 }` 挂在 `.graph-canvas`（`overflow:auto`）内，被裁掉/压在下方面板之下。修法任选其一并说明：把菜单渲染到 portal（`document.body`）并按触发按钮的 `getBoundingClientRect` 定位；或把菜单改为向上弹/贴节点右侧且给 `.graph-canvas` 所在层足够的 z-index 与 `overflow: visible`（注意画布本身要保留滚动）。同类的其它绝对定位弹层（`@` 弹层、节点菜单）顺手检查一遍是否有同样的裁剪。
