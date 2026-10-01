# TASK-38 每次派发新建一张图（10-01）

**只改 `src/**` 与 `docs/CONSOLE-CHANNEL.md`。不 commit，不 push。**

## 背景

`docs/PRODUCT.md`「待用户裁决」第一条（10-01 用户同意按建议做）：现在 `aicross dispatch` 续写同一项目最新的那张图，后一次派发的执行者会读到前一次的材料与回复，run 摘要的遗留会混入别的 run 的（积压 103），图标题只是第一次派发的任务。新形态下对话与上下文在宿主里，改为**每次 `aicross dispatch` 新建一张图**，派发之间不共享上下文。

## 要求

1. `src/engine/mod.rs` 的 `start_dispatch`：不再查找同项目的已有图（现在经 `latest_project_graph`），总是新建图。`plan_dispatch` 接受已有图的能力保留不动——金标准重放（`src/engine/golden.rs`）照常把夹具里的已有图喂给它，31 个夹具必须继续全绿，不改夹具与 `src-ui/`。
2. 因此不再被生产路径用到的函数（如 `latest_project_graph`、`pick_project_graph`）删掉，连同只为它们存在的测试；仍被别处使用的（如路径归一 `normalized`）保留。删之前用编译器确认调用方。
3. 补测试：同一项目连续两次 `start_dispatch`（注入的 spawn 不真起进程），得到两张不同的图，第二张图里没有第一次的节点；第二次 run 摘要的遗留不含第一次的。
4. `docs/CONSOLE-CHANNEL.md`：写明每次派发新建一张图、派发之间不共享上下文；追问同一件事时由调用方把前情写进任务文本。删掉或改正「多个命令行后台 run 交替提交同一张图」等已不成立的说法（落图合并仍保留，用于 `adopt` 与后台提交并发）。
5. 在 `tasks/fix-04-r5-backlog.md` 把第 103 条划掉，注明「已修（TASK-38，每次派发新建图）」。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
```

`src/engine/mod.rs` 等 Rust 文件保持 LF。完成后写 `tasks/REPORT-38.md`：改了哪些行、删了哪些函数与测试、新增测试名、三条命令的实际输出摘要。
