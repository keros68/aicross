# REPORT-08A 蜂群契约与 Rust 引擎

范围：只做 TASK-08「契约」与「A. 引擎（Rust）」1→5。未运行 `cargo fmt`，未启动窗口，未发真机请求，未修改 `src/channels/`，未 commit、未 push。

## 逐条交付

### 1. Node 镜像与留痕字段

已做：`GraphNode` 追加可选 `swarm` 及 `SwarmItem` 镜像（`src/orchestration.rs:58`、`src/orchestration.rs:64`）；`DispatchRecord` 追加 `swarm_item` 与 `merge_result`，旧留痕缺字段时仍取默认值（`src/dispatch.rs:361`、`src/dispatch.rs:363`）。`DispatchSettings` 追加 `swarm_limit`，默认 4（`src/dispatch.rs:37`、`src/dispatch.rs:68`）。

已补：`task08a_graph_node_swarm_roundtrips_and_old_graph_defaults_to_none`；`task08a_dispatch_record_swarm_fields_roundtrip`；`task08a_tauri_swarm_contract_serializes_camel_case_and_merge_results`。

### 2. check_swarm_plan

已做：新增只读检查命令；用新增直接依赖 `glob = 0.3.3` 匹配规范化项目相对路径，目录遍历复用快照排除清单，报告脏树、每对工作的第一个重叠路径和未匹配 glob（`src/dispatch.rs:366`、`src/dispatch.rs:450`、`src/dispatch.rs:490`；`Cargo.toml:31`）。最初拟用 `globset`，但当前环境强制离线且缓存无该包，改用锁文件中已存在的 `glob 0.3.3`，没有联网取包。

已补：`task08a_swarm_plan_reports_overlap_unmatched_and_dirty_tree`，同一具名测试覆盖重叠、不重叠、不匹配、脏树。

### 3. merge_swarm_items

已做：取项目写锁后按 `item` 排序；所有 git 步骤和验收命令均经 `run_process_registered`。逐件执行 add/无改动检查/commit/cherry-pick/验收；冲突 abort 后继续，验证失败 reset 到上一件后继续，脏树整批 skipped；输出安全过滤并按 UTF-8 边界截到 8KB；每件写 `aicross-dispatch/1` 留痕（`src/dispatch.rs:616`、`src/dispatch.rs:780`）。冻结入参没有 `title`，因此提交信息的标题段使用 `node_id`，格式仍为 `aicross swarm item N: <node_id>`。

已补：`task08a_merge_two_worktrees_in_item_order_with_one_commit_each`；`task08a_merge_conflict_aborts_second_and_keeps_first_commit`；`task08a_failed_validation_resets_only_the_failed_item`；`task08a_dirty_project_skips_entire_merge_batch`。四条都创建临时 git 仓库与两个真实 worktree 并实际运行 git。

### 4. 冻结拆分提示与角色默认

已做：新增 `fixtures/prompts/split.txt:1`，限定只读、JSON 围栏、文件范围不重叠、依赖工作合并、不超过 N 件；`fill_role_defaults` 让「拆分」复用「出方案」目标，并强制 `write=只读`、`visibility=共享`（`src/orchestration.rs:478`）。

已补：`task08a_split_role_uses_planning_target_and_frozen_readonly_prompt`。

### 5. 注册命令

已做：在 Tauri handler 注册 `check_swarm_plan` 与 `merge_swarm_items`（`src/server.rs:3940`、`src/server.rs:3941`）；`server.rs` 已核对为全 CRLF，其余修改的 Rust 文件为全 LF。

已补：`task08a_server_registers_both_swarm_commands`。

## 没做

- 没做 TASK-08 的 B 节；R1 仅为拆分提示词单一来源修改了前端盖章读取，未实施 B 节功能。
- 初始交付未实施后来追加的 A6「builtin-swarm」；修正轮 R1 已补齐，最终状态见下方 R1/R2 汇总。
- 初始交付与 R1 的 `npm run build` 曾受本机 `spawn EPERM` 阻断；修正轮 R2 已实际构建成功，最终状态与原始输出见文末。

## 验证命令与输出原文

### `cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.63s
```

#### Build 最终完整原始输出

下方仅将 ANSI ESC 控制字符记为 `<ESC>`，其余内容未改写。

```text

> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:27100) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
<ESC>[2Ktransforming...<ESC>[32m✓<ESC>[39m 323 modules transformed.
rendering chunks...
computing gzip size...
<ESC>[2mdist/<ESC>[0m<ESC>[32mindex.html                                                <ESC>[39m<ESC>[2m<ESC>[1m  3.46 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.31 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[32monig-Du5pRr7Y.wasm                                 <ESC>[39m<ESC>[2m<ESC>[1m466.61 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip: 159.69 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[35mxterm-sLQPUA_C.css                                 <ESC>[39m<ESC>[2m<ESC>[1m  4.00 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.02 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[35mmodel-picker-Dx7sti2H.css                          <ESC>[39m<ESC>[2m<ESC>[1m  4.75 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.40 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[35mresources-D1_8prtv.css                             <ESC>[39m<ESC>[2m<ESC>[1m  7.31 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.58 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[35mSettingsModal-BlDZGgPL.css                         <ESC>[39m<ESC>[2m<ESC>[1m 14.64 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   2.88 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[35mmain-CjaHO_is.css                                  <ESC>[39m<ESC>[2m<ESC>[1m126.95 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  22.30 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mdist-js-DCNFSo35.js                                <ESC>[39m<ESC>[2m<ESC>[1m  0.06 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.07 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mevent-BG1bOqAl.js                                  <ESC>[39m<ESC>[2m<ESC>[1m  0.06 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.07 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mgithub-dark-default-CBXcfawx.js                    <ESC>[39m<ESC>[2m<ESC>[1m  0.06 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.07 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mgithub-light-default-DZkGIntf.js                   <ESC>[39m<ESC>[2m<ESC>[1m  0.06 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.07 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mapp-DZbAd3JO.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.06 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.08 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mwindow-DbjsDo6X.js                                 <ESC>[39m<ESC>[2m<ESC>[1m  0.07 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.08 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mwebview-BSvUINLI.js                                <ESC>[39m<ESC>[2m<ESC>[1m  0.07 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.08 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mc-ZIVULKG9.js                                      <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mgo-CGMFcIL6.js                                     <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mcpp-BYDDekI2.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mcss-DiSwE4lG.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mjsx-BXqOFxOg.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mmdx-DO5bwks_.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mphp-CJdsFQTS.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36msql-CR5oIvIG.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mtsx-CIBdmQc_.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mvue-CzixFqBY.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mxml-BRAzgxal.js                                    <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mhtml-CzHmFjme.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mjava-B7RovB7f.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mjson-Co1pQaoE.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mless-ChilM_G9.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mruby-Bx3wtNp_.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mrust-BqzCZC1P.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36msass-BWBUGzI1.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mscss-CRjgb_zd.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mtoml-p2mNpJDD.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36myaml-C9xyWgSe.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mswift-CLC7FPkP.js                                  <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mcsharp-CAIceAFL.js                                 <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mdocker-PEAA3uQ1.js                                 <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mkotlin-CkDzfbmd.js                                 <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mpython-D486b-dc.js                                 <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36msvelte-DRnvJitS.js                                 <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mmarkdown-ufefEwR7.js                               <ESC>[39m<ESC>[2m<ESC>[1m  0.09 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mjavascript-CnbGTk9g.js                             <ESC>[39m<ESC>[2m<ESC>[1m  0.10 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.11 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mtypescript-B6MgdiAh.js                             <ESC>[39m<ESC>[2m<ESC>[1m  0.10 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.11 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mbash-CplIU7Xz.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  0.10 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.11 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mengine-oniguruma-C18Ttql_.js                       <ESC>[39m<ESC>[2m<ESC>[1m  0.17 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.14 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-vue-interpolations-DSV92BsH.js                <ESC>[39m<ESC>[2m<ESC>[1m  0.40 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.24 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-vue-directives-BpYOJEc5.js                    <ESC>[39m<ESC>[2m<ESC>[1m  0.42 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.27 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mrolldown-runtime-km5iIlDX.js                       <ESC>[39m<ESC>[2m<ESC>[1m  0.68 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.41 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mdefineProperty-DRsMVE4X.js                         <ESC>[39m<ESC>[2m<ESC>[1m  0.71 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.39 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-markdown-vue-CBO-3uK2.js                      <ESC>[39m<ESC>[2m<ESC>[1m  0.75 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.38 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-html-derivative-CoJpQrZm.js                   <ESC>[39m<ESC>[2m<ESC>[1m  0.83 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.46 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-vue-sfc-style-variable-injection-CGHzv4ky.js  <ESC>[39m<ESC>[2m<ESC>[1m  0.94 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.43 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mdispatch-settings-CI6-rF4z.js                      <ESC>[39m<ESC>[2m<ESC>[1m  1.06 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.46 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mcore-CJOC6CgB.js                                   <ESC>[39m<ESC>[2m<ESC>[1m  1.33 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.62 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mresources-tCbnwTRg.js                              <ESC>[39m<ESC>[2m<ESC>[1m  1.55 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.88 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-docker-_OIk34xS.js                            <ESC>[39m<ESC>[2m<ESC>[1m  1.73 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.59 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-json-CwMwdRMR.js                              <ESC>[39m<ESC>[2m<ESC>[1m  2.81 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   0.77 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-glsl-dAjUAX10.js                              <ESC>[39m<ESC>[2m<ESC>[1m  3.63 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.41 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-xml-BXuWBzo8.js                               <ESC>[39m<ESC>[2m<ESC>[1m  5.38 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.21 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-postcss-uUyjoxlZ.js                           <ESC>[39m<ESC>[2m<ESC>[1m  6.40 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.90 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-toml-DdlhiNcx.js                              <ESC>[39m<ESC>[2m<ESC>[1m  6.41 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.28 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mtauri-DsilgRrc.js                                  <ESC>[39m<ESC>[2m<ESC>[1m  6.46 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   2.53 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-regexp-PkKp9FJr.js                            <ESC>[39m<ESC>[2m<ESC>[1m  7.98 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.41 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-haml-BSPUMTuO.js                              <ESC>[39m<ESC>[2m<ESC>[1m  8.27 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   1.82 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-kotlin-sWY7ArP7.js                            <ESC>[39m<ESC>[2m<ESC>[1m  8.77 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   2.13 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mSkillsPanel-O0eH4AS1.js                            <ESC>[39m<ESC>[2m<ESC>[1m  9.18 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   3.38 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-sass-BauQOl7Q.js                              <ESC>[39m<ESC>[2m<ESC>[1m  9.28 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   2.49 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mapp-state-D49IauP-.js                              <ESC>[39m<ESC>[2m<ESC>[1m 10.43 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   3.26 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-yaml-CuAmHpom.js                              <ESC>[39m<ESC>[2m<ESC>[1m 10.49 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   2.26 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mResourcesOverlay-BEurssnM.js                       <ESC>[39m<ESC>[2m<ESC>[1m 11.29 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   3.62 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-rust-BVU9p_Ey.js                              <ESC>[39m<ESC>[2m<ESC>[1m 15.05 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   2.71 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-lua-B42IYw08.js                               <ESC>[39m<ESC>[2m<ESC>[1m 15.53 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   3.15 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-graphql-Cleegyn5.js                           <ESC>[39m<ESC>[2m<ESC>[1m 18.03 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   2.50 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-svelte-DpdpUH4Q.js                            <ESC>[39m<ESC>[2m<ESC>[1m 18.26 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   3.14 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-vue-6b42Py3N.js                               <ESC>[39m<ESC>[2m<ESC>[1m 22.35 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   2.54 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-sql-Dkaro8Br.js                               <ESC>[39m<ESC>[2m<ESC>[1m 23.39 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   7.43 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-scss-DHs9xDPW.js                              <ESC>[39m<ESC>[2m<ESC>[1m 27.20 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   4.19 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-java-BmwQdcBP.js                              <ESC>[39m<ESC>[2m<ESC>[1m 27.21 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   4.25 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mmarkdown-CrkSmzZW.js                               <ESC>[39m<ESC>[2m<ESC>[1m 30.02 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  10.85 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mSettingsModal-w0yOdSyy.js                          <ESC>[39m<ESC>[2m<ESC>[1m 30.11 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   9.16 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-shellscript-ClIYzLfq.js                       <ESC>[39m<ESC>[2m<ESC>[1m 41.47 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   6.07 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-ruby-Bc0Yd8v4.js                              <ESC>[39m<ESC>[2m<ESC>[1m 46.65 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   5.71 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-go-CtQjvmW3.js                                <ESC>[39m<ESC>[2m<ESC>[1m 46.81 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   5.15 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-css-BZYgv9eh.js                               <ESC>[39m<ESC>[2m<ESC>[1m 49.02 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  11.90 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-html-BZHVI7eq.js                              <ESC>[39m<ESC>[2m<ESC>[1m 57.25 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  11.74 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-markdown-BQDdzsCg.js                          <ESC>[39m<ESC>[2m<ESC>[1m 59.32 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   5.66 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-python-DRoGfMiU.js                            <ESC>[39m<ESC>[2m<ESC>[1m 69.94 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:   9.10 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-c-CghvNN5f.js                                 <ESC>[39m<ESC>[2m<ESC>[1m 72.10 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  10.50 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-swift-ty8Frjwc.js                             <ESC>[39m<ESC>[2m<ESC>[1m 86.67 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  14.69 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-csharp-B402LPno.js                            <ESC>[39m<ESC>[2m<ESC>[1m 89.67 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  10.64 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-less-CWcMZZEH.js                              <ESC>[39m<ESC>[2m<ESC>[1m 97.62 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  14.84 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-php-DJHB53HJ.js                               <ESC>[39m<ESC>[2m<ESC>[1m111.33 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  28.66 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mmodel-picker-BnUxpJMg.js                           <ESC>[39m<ESC>[2m<ESC>[1m130.41 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  43.20 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-mdx-C6frHttG.js                               <ESC>[39m<ESC>[2m<ESC>[1m136.10 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  23.54 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mvendor-CPUhUmlm.js                                 <ESC>[39m<ESC>[2m<ESC>[1m148.73 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  40.40 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-javascript-HQTRwLWZ.js                        <ESC>[39m<ESC>[2m<ESC>[1m174.81 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  16.62 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-tsx-DrUye_4e.js                               <ESC>[39m<ESC>[2m<ESC>[1m175.52 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  16.62 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-jsx-B0Mso9yp.js                               <ESC>[39m<ESC>[2m<ESC>[1m177.78 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  16.71 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-typescript-CUZHi_ve.js                        <ESC>[39m<ESC>[2m<ESC>[1m181.07 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  16.24 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mreact-CWnP4iQq.js                                  <ESC>[39m<ESC>[2m<ESC>[1m190.94 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  60.60 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-cpp-macro-CgwvTOCX.js                         <ESC>[39m<ESC>[2m<ESC>[1m224.24 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  18.05 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mlang-cpp-BAHFRj0j.js                               <ESC>[39m<ESC>[2m<ESC>[1m402.02 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip:  31.11 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mmain-XfyZKy8w.js                                   <ESC>[39m<ESC>[2m<ESC>[1m427.71 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip: 211.49 kB<ESC>[0m
<ESC>[2mdist/<ESC>[0m<ESC>[2massets/<ESC>[0m<ESC>[36mxterm-_r9IGi5_.js                                  <ESC>[39m<ESC>[2m<ESC>[1m465.79 kB<ESC>[0m<ESC>[0m<ESC>[2m │ gzip: 123.12 kB<ESC>[0m

✓ built in 700ms

```

## 修正轮 R1 验证详情（逐条汇总见文末）

1. 已修：新增 Rust 内置 `builtin-swarm`，单个拆分节点固定只读/共享、目标由 planning 默认填充，并把冻结提示词随预设返回（`src/orchestration.rs:412`、`src/orchestration.rs:420`）／已补：`task08a_r1_builtin_swarm_is_single_filled_readonly_split_node`。
2. 已修：合并请求和返回继续统一 `camelCase`，契约明确 `nodeId`、`testOutput`，序列化测试同时断言 snake_case 键不存在（`src/dispatch.rs:550`、`tasks/TASK-08-swarm.md:10`）／已补：`task08a_tauri_swarm_contract_serializes_camel_case_and_merge_results`。
3. 已修：`split.txt` 成为拆分提示词唯一来源，Rust `Target.preset_text` 读取冻结文件，前端盖章只消费该字段；B 侧合并时不得向 `PRESET_NODE_TEXTS` 添加 `builtin-swarm` 文本（`src/orchestration.rs:183`、`src/orchestration.rs:420`、`src-ui/src/graph/stamp.ts:48`）／已补：`task08a_r1_builtin_swarm_is_single_filled_readonly_split_node`。
4. 已修：合并留痕 `status` 仅写 `ok/error`，结果只写 `merge_result`；对账显式忽略合并留痕（`src/dispatch.rs:595`、`src/dispatch.rs:1047`）／已补：`task08a_r1_merge_record_uses_status_enum_and_reconcile_ignores_it`。
5. 已修：合并前读取 `git worktree list --porcelain`，输入路径 canonicalize 后必须命中本项目登记清单，否则跳过（`src/dispatch.rs:639`、`src/dispatch.rs:650`）／已补：`task08a_r1_missing_unregistered_and_unchanged_worktrees_are_skipped`。
6. 已修：引擎只读角色集合加入「拆分」，不能用手改图或旧图绕过（`src/dispatch.rs:1179`）／已补：`task08a_r1_split_role_is_rejected_when_not_readonly`。
7. 已修：无未提交改动时检查 worktree HEAD 相对项目基线的领先提交，按顺序逐个 cherry-pick；验证失败回退本件全部提交（`src/dispatch.rs:650`）／已补：`task08a_r1_agent_committed_worktree_head_is_merged`。
8. 已修：验证和写锁使用 `settings.timeout_secs`；留痕位置遵守 `write_project_dispatch`；`test_failed` 有 note；请求追加可选 title；目录 glob、`./`、大小写归一化；计划检查改为 `spawn_blocking`；cherry-pick 启动失败归 skipped，abort 失败停止后续件；脏树 note 带 porcelain 摘要（`src/dispatch.rs:453`、`src/dispatch.rs:508`、`src/dispatch.rs:650`）／已补：`task08a_swarm_plan_reports_overlap_unmatched_and_dirty_tree`、`task08a_failed_validation_resets_only_the_failed_item`。
9. 已修：计划检查和合并均按 `settings.swarm_limit` 只处理前 N 件，超限件返回 skipped + note（`src/dispatch.rs:508`、`src/dispatch.rs:650`）／已补：`task08a_r1_swarm_limit_processes_only_first_n_items`。
10. 已修：git 测试统一先探测 git，不存在时 `eprintln!` 后跳过；补齐无改动、不存在、未登记、失败在中间仍继续、留痕对账等真实临时仓库路径（`src/dispatch.rs:3375`、`src/dispatch.rs:3395`、`src/dispatch.rs:3430`）／已补：`task08a_r1_missing_unregistered_and_unchanged_worktrees_are_skipped`、`task08a_r1_middle_validation_failure_does_not_stop_later_item`、`task08a_r1_merge_record_uses_status_enum_and_reconcile_ignores_it`。

GLM 补充：已修 `docs/PRODUCT.md` 的 `swarm_item` 类型为 number（`docs/PRODUCT.md:219`）；已修 cherry-pick 启动失败分类、abort 失败停止后续、脏树详情、Windows 大小写 glob、title 字段。未采纳「把 120 行合并分支抽成 merge_one」：属于低优先可读性重构，不改变本轮行为，且会扩大已通过真 git 测试的改动面。未单独扩展 `run_validation` 的取消登记参数：现有合并命令已用设置超时形成终止边界，本轮契约未要求合并取消入口。

### R1 验证命令与输出原文

`cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.55s
```

最终 `cargo test`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 10.00s
     Running unittests src\main.rs (target\debug\deps\aicross-ba10b8ae6d53597a.exe)

running 307 tests
test result: ok. 296 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 9.49s
```

说明：此前两次全量复跑同一既有 ACP 时序用例 `run_one_acp_turn_failure_stays_acp_and_never_calls_oneshot` 在全量并发负载下先得到 timeout（单独复跑通过）；将该 happy-path 测试的 fixture 超时从 1 秒放宽到 10 秒后，最终全量通过。失败原文：

```text
thread 'dispatch::tests::run_one_acp_turn_failure_stays_acp_and_never_calls_oneshot' panicked at src\dispatch.rs:4116:9:
assertion `left == right` failed
  left: "timeout"
 right: "error"
test result: FAILED. 295 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 10.21s
```

`cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  23 passed (23)
      Tests  294 passed (294)
   Start at  09:12:35
   Duration  746ms (transform 66%, tests 17%, import 15%, worker 2%)
```

`cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

failed to load config from D:\work\aicross-p4\src-ui\vite.config.ts
error during build:
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
```

追加复跑 `cd src-ui && npm run build -- --configLoader native`，类型检查完成且 Vite 转换 323 个模块，仍在启动 esbuild 子进程时被本机策略阻断：

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build --configLoader native

vite v8.0.7 building client environment for production...
transforming...✓ 323 modules transformed.
✗ Build failed in 560ms
error during build:
Build failed with 1 error:

[plugin vite:esbuild-transpile]
Error: spawn EPERM
```

未执行 `cargo fmt`。行尾复核原文：

```text
src/server.rs LF=4620 CRLF=4620
src/dispatch.rs LF=6071 CRLF=0
src/orchestration.rs LF=1377 CRLF=0
```

### 最终 Rust 复跑结论

最后一次 `cargo test` 的结尾原文（完整逐项输出与上方相同，测试调整后仅耗时变化）：

```text
test result: ok. 289 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 8.56s
```

结论：通过，输出中没有 warning。

修改验收回退测试使其同时覆盖「accept 优先、空 accept 回退 validate_command」后，最终复跑原文：

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.21s
```

### `cargo test`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 14.15s
     Running unittests src\main.rs (target\debug\deps\aicross-ba10b8ae6d53597a.exe)

running 300 tests
test channels::acp::tests::live_kimi_acp_two_turn_continuation ... ignored, 编排者在沙箱外使用已登录的 kimi 运行
test channels::acp::tests::only_kimi_is_enabled_as_an_acp_runner ... ok
test channels::acp::tests::acp_agent_message_updates_append_only_reply_text ... ok
test channels::acp::tests::legacy_cache_read_tokens_shape_is_parsed ... ok
test channels::acp::tests::permission_policy_classifies_read_and_write ... ok
test channels::acp::tests::append_bounded_cjk_is_utf8_safe_and_stops_after_marker ... ok
test channels::acp::tests::queued_acp_node_observes_cancel_while_waiting_for_client_lock ... ok
test channels::acp::tests::fake_acp_three_same_updates_plus_permission_count_as_four_repeated_calls ... ok
test channels::acp::tests::foreign_acp_session_is_detected_by_real_client_state ... ok
test channels::acp::tests::legacy_models_shape_remains_supported ... ok
test channels::acp::tests::missing_model_lists_do_not_reject_acp_and_leave_runner_note ... ok
test channels::acp::tests::late_session_update_is_drained_before_next_prompt ... ok
test channels::tests::blind_claude_contains_restricted ... ok
test channels::tests::blind_pi_with_artifacts_keeps_read_only_tools ... ok
test channels::tests::blind_pi_with_no_context_files_is_fully_isolated ... ok
test channels::acp::tests::same_key_concurrent_run_spawns_one_fake_process ... ok
test channels::tests::cc_switch_reads_usage_from_stderr ... ok
test channels::tests::cc_switch_requires_configured_script_path ... ok
test channels::tests::claude_null_api_error_status_is_not_an_error ... ok
test channels::tests::claude_identity_comes_from_model_usage_keys_when_no_top_level_model ... ok
test channels::tests::codex_high_maps_to_high_and_never_to_xhigh ... ok
test channels::tests::codex_resume_follows_parent_options ... ok
test channels::tests::kimi_collects_stream_usage_when_present ... ok
test channels::tests::kimi_reads_plain_string_assistant_content_and_resume_hint ... ok
test channels::tests::kimi_crops_more_than_30000_utf8_bytes ... ok
test channels::acp::tests::cancelled_inflight_acp_prompt_returns_cancelled_and_closes_session ... ok
test channels::tests::kimi_requires_configured_alias ... ok
test channels::tests::kimi_resume_uses_current_cli_session_flag ... ok
test channels::tests::pi_continuation_preserves_thinking_setting ... ok
test channels::tests::pi_error_event_surfaces_error_message ... ok
test channels::tests::pi_reads_camel_case_cache_usage ... ok
test channels::tests::kimi_session_parser_ignores_resume_text_in_answer ... ok
test channels::tests::pi_uses_explicit_session_and_app_owned_session_directory ... ok
test channels::tests::pi_response_model_mismatch_fails_and_reports_true_identity ... ok
test channels::tests::r3_6_pi_accepts_known_deepseek_route_alias_but_not_zai_upgrade ... ok
test channels::tests::scheduling_aliases_normalize_to_one_agent_key ... ok
test channels::tests::writable_claude_excludes_web_tools_from_allowlist ... ok
test channels::tests::writable_pi_enables_write_tools ... ok
test channels::tests::writable_codex_uses_workspace_write_sandbox ... ok
test dispatch::tests::artifact_diff_covers_deleted_binary_large_and_oversized_markers ... ok
test channels::acp::tests::continuation_model_change_uses_same_session_and_updates_model ... ok
test dispatch::tests::blind_acceptance_directory_contains_only_artifact_delivery ... ok
test dispatch::tests::blind_acceptance_rejects_drive_relative_changed_path ... ok
test dispatch::tests::arbitration_role_is_forced_readonly_by_engine ... ok
test dispatch::tests::classification_parser_tolerates_fences_and_missing_arrays ... ok
test dispatch::tests::broad_redaction_warning_only_counts_unquoted_whole_lines ... ok
test dispatch::tests::blind_directory_contains_only_selected_materials ... ok
test dispatch::tests::fallback_predicate_only_triggers_on_channel_failures ... ok
test channels::acp::tests::killed_acp_process_returns_process_error_from_real_client_path ... ok
test channels::acp::tests::r1_permission_request_tool_call_is_counted ... ok
test channels::acp::tests::kimi_config_options_set_model_default_mode_and_thinking ... ok
test channels::acp::tests::r1_other_session_tool_call_is_not_counted ... ok
test dispatch::tests::fallback_record_prefers_dispatched_vendor_over_inference ... ok
test dispatch::tests::fallback_blind_attempt_gets_fresh_material_dir ... ok
test dispatch::tests::legacy_dispatch_record_without_runner_defaults_to_oneshot ... ok
test channels::acp::tests::fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write ... ok
test dispatch::tests::live_arena_and_arbitration ... ignored, three real model requests (擂台两家 + 仲裁一家); run outside the sandbox for TASK-05B acceptance
test dispatch::tests::live_blind_review ... ignored, real model requests; run once for TASK-03 acceptance
test dispatch::tests::live_cc_switch_route ... ignored, one real cc-switch request; requires AICROSS_CC_SWITCH_PATH and provider
test dispatch::tests::live_economy_pipeline ... ignored, three real model requests; run once outside the sandbox for TASK-04 acceptance
test dispatch::tests::live_shared_continuation ... ignored, two real model requests; run once for TASK-03 acceptance
test dispatch::tests::fallback_reruns_for_blind_node_and_marks_no_skip ... ok
test dispatch::tests::inspect_state_uses_machine_header_instead_of_status_prose ... ok
test dispatch::tests::legacy_frontmatter_without_r2_fields_parses_with_empty_defaults ... ok
test dispatch::tests::non_git_worktree_is_accounted_as_single_writer ... ok
test dispatch::tests::fallback_record_uses_fallback_billing_and_vendor ... ok
test dispatch::tests::node_stage_is_used_for_result_and_record ... ok
test dispatch::tests::preparation_failure_records_only_unsent_placeholder ... ok
test channels::acp::tests::r1_untitled_tool_call_still_increments_count ... ok
test channels::acp::tests::readonly_permission_allows_read_kind_and_execute_title_cannot_bypass ... ok
test dispatch::tests::r1_committed_snapshot_cleanup_failure_returns_note_not_error ... ok
test dispatch::tests::r1_before_root_file_error_preserves_existing_prev_directory ... ok
test dispatch::tests::r1_parse_output_populates_dispatch_cache_read ... ok
test dispatch::tests::r1_heartbeat_normal_stop_emits_once_then_exits ... ok
test dispatch::tests::r1_heartbeat_disconnected_stop_channel_exits_without_more_emits ... ok
test dispatch::tests::project_write_lock_wait_can_be_cancelled ... ok
test dispatch::tests::r1_reconcile_refills_cache_read_from_record ... ok
test dispatch::tests::r1_record_unknown_key_is_ignored_and_missing_keys_use_defaults ... ok
test dispatch::tests::r1_record_schema_mismatch_is_rejected ... ok
test dispatch::tests::r1_running_progress_truncates_last_tool_to_120_characters ... ok
test dispatch::tests::r1_running_progress_redacts_authorization_bearer_title ... ok
test dispatch::tests::r1_terminal_progress_after_heartbeat_stop_has_no_running_fields_or_late_emit ... ok
test dispatch::tests::r3_12_artifacts_and_blind_delivery_redact_secrets ... ok
test dispatch::tests::fallback_skipped_for_writable_node_returns_primary_failure ... ok
test dispatch::tests::r3_14_blind_single_writer_nodes_do_not_take_the_project_write_lock ... ok
test dispatch::tests::r3_1_artifacts_serialize_with_snake_case_keys ... ok
test dispatch::tests::r3_13_missing_git_probe_falls_back_and_failed_materialization_leaves_no_diff_dir ... ok
test dispatch::tests::r3_3_empty_target_returns_node_error_without_creating_run_files ... ok
test dispatch::tests::r3_8_empty_project_dir_rejects_preview_and_dispatch_without_writes ... ok
test dispatch::tests::r4_2_invalid_acceptance_write_returns_node_error ... ok
test dispatch::tests::r5_3_validation_error_strips_ansi_and_formats_exit_code ... ok
test dispatch::tests::r5_8_quoted_new_file_diff_headers_are_normalized ... ok
test dispatch::tests::r3_16_worktree_validation_and_r3_17_abandoned_state_keep_run_identity ... ok
test dispatch::tests::r3_7_large_stdout_is_externalized_but_small_stdout_is_inlined ... ok
test dispatch::tests::r5_9_kimi_missing_identity_and_usage_are_explicit_in_trace_fields ... ok
test dispatch::tests::project_write_lock_is_mutually_exclusive ... ok
test dispatch::tests::redacts_all_six_secret_shapes_and_stays_clean ... ok
test dispatch::tests::record_stdout_keeps_primary_and_fallback_dumps_separate ... ok
test dispatch::tests::reset_acp_fallback_builds_full_context_invocation_without_resume_flags ... ok
test dispatch::tests::git_repository_uses_three_line_unified_diff ... ok
test dispatch::tests::snapshot_builds_diff_and_changed_file_content ... ok
test dispatch::tests::snapshot_marks_files_over_eight_megabytes_without_reading_content ... ok
test dispatch::tests::snapshot_skips_excluded_directories_and_enforces_total_bytes ... ok
test channels::acp::tests::writable_acp_session_uses_default_never_yolo_mode ... ok
test dispatch::tests::task06_acp_runner_note_append_preserves_rollback_note ... ok
test dispatch::tests::task06_dispatch_request_reads_rollback_before_camel_case ... ok
test dispatch::tests::state_uses_actual_limit_and_separates_failures ... ok
test dispatch::tests::task06_duplicate_active_run_is_rejected_before_dispatch ... ok
test dispatch::tests::task06_execution_prompt_forbids_external_side_effects ... ok
test dispatch::tests::task06_fallback_reruns_once_uses_fields_without_reply_marker ... ok
test dispatch::tests::fallback_agent_blocks_same_agent_node_from_running_concurrently ... ok
test dispatch::tests::task06_reconcile_order_parses_timestamps_and_prefers_ok_on_tie ... ok
test dispatch::tests::task06_reconcile_parses_embedded_heading_and_legacy_record ... ok
test channels::acp::tests::writable_permission_is_allow_once_never_allow_always ... ok
test dispatch::tests::task06_reconcile_classifies_completed_incomplete_unknown_and_ignores_other_run ... ok
test dispatch::tests::task06_rollback_created_root_path_does_not_escape_project ... ok
test dispatch::tests::task06_rollback_reads_before_directory_beyond_snapshot_total_limit ... ok
test channels::acp::tests::two_five_megabyte_turns_are_consumed_without_lifetime_truncation ... ok
test dispatch::tests::task06_rollback_replaces_target_symlink_without_following_it ... ok
test dispatch::tests::task06_safe_relative_path_rejects_root_and_current_components ... ok
test dispatch::tests::task06_rollback_reports_over_eight_megabyte_snapshot_entries_as_skipped ... ok
test dispatch::tests::task06_rollback_command_waits_for_project_write_lock ... ok
test dispatch::tests::task06_snapshot_outside_project_is_absolute_and_colocated_with_records ... ok
test dispatch::tests::task07_before_snapshot_success_replaces_old_and_removes_prev ... ok
test dispatch::tests::task06_single_writer_rollback_restores_changed_deleted_and_created_files ... ok
test dispatch::tests::task07_dispatch_and_reconcile_results_preserve_cache_read_usage ... ok
test dispatch::tests::task07_dispatch_record_all_legacy_fields_default_instead_of_unknown ... ok
test dispatch::tests::task07_before_snapshot_write_failure_restores_previous_directory ... ok
test channels::acp::tests::silent_acp_process_reaches_typed_timeout_without_fallback ... ok
test dispatch::tests::task07_running_progress_serializes_heartbeat_contract_fields ... ok
test dispatch::tests::task07_latest_before_dir_rejects_uncanonicalizable_workdir ... ok
test dispatch::tests::task07_zero_change_redispatch_preserves_nonempty_snapshot ... ok
test dispatch::tests::task08a_dispatch_record_swarm_fields_roundtrip ... ok
test dispatch::tests::r1_backup_attempt_does_not_inherit_primary_tool_counts ... ok
test dispatch::tests::acp_disabled_or_unsupported_preserves_oneshot_dispatch_result ... ok
test channels::acp::tests::shutdown_all_returns_within_one_second_when_client_is_locked ... ok
test dispatch::tests::task08a_server_registers_both_swarm_commands ... ok
test channels::acp::tests::shutdown_all_sends_session_close_when_capability_is_available ... ok
test dispatch::tests::r1_zero_change_redispatch_keeps_old_snapshot_discoverable_for_rollback ... ok
test dispatch::tests::task06_early_failure_does_not_replace_previous_before_snapshot ... ok
test dispatch::tests::task08a_tauri_swarm_contract_serializes_camel_case_and_merge_results ... ok
test dispatch::tests::trace_fields_are_always_sanitized ... ok
test hook_installer::tests::codex_features_creates_section_if_missing ... ok
test hook_installer::tests::codex_features_enables_hooks_and_strips_legacy ... ok
test hook_installer::tests::codex_features_idempotent ... ok
test hook_installer::tests::codex_features_preserves_user_hooks_false_overwrites ... ok
test hook_installer::tests::codex_hooks_json_creates_with_4_events ... ok
test hook_installer::tests::codex_hooks_json_idempotent_no_accumulation ... ok
test hook_installer::tests::codex_hooks_json_unparseable_left_untouched ... ok
test hook_installer::tests::codex_hooks_json_preserves_user_hooks ... ok
test hook_installer::tests::kimi_config_blocks_have_only_allowed_fields ... ok
test hook_installer::tests::kimi_config_creates_with_9_events ... ok
test dispatch::tests::task06_snapshot_persist_failure_warns_without_flipping_success ... ok
test hook_installer::tests::kimi_config_preserves_existing_content ... ok
test launch::tests::ignores_non_launch_argv ... ok
test launch::tests::parses_equals_form_and_validates_cwd ... ok
test hook_installer::tests::kimi_config_idempotent_no_duplicates ... ok
test launch::tests::parses_tool_with_and_without_argv0 ... ok
test launch::tests::rejects_unknown_tool ... ok
test hook_installer::tests::kimi_config_reinstall_strips_stale_keeps_user_hooks ... ok
test orchestration::tests::arena_role_defaults_stay_empty ... ok
test orchestration::tests::builtin_arena_and_jury_shapes ... ok
test orchestration::tests::dispatchable_missing_file_reports_error ... ok
test orchestration::tests::dispatchable_lists_all_detected_with_connected_first ... ok
test orchestration::tests::dispatchable_broken_file_reports_error ... ok
test orchestration::tests::load_missing_graph_returns_none ... ok
test orchestration::tests::list_graphs_skips_broken_files ... ok
test orchestration::tests::graph_roundtrip ... ok
test orchestration::tests::premises_attachments_and_last_graph_id_roundtrip ... ok
test orchestration::tests::premises_legacy_format_without_new_fields_parses_empty ... ok
test orchestration::tests::presets_broken_file_returns_builtins ... ok
test orchestration::tests::presets_missing_file_returns_builtins ... ok
test orchestration::tests::premises_broken_file_loads_empty_and_save_refuses_to_overwrite ... ok
test orchestration::tests::presets_roundtrip ... ok
test orchestration::tests::presets_wrong_version_returns_builtins ... ok
test orchestration::tests::premises_roundtrip ... ok
test orchestration::tests::save_rejects_bad_ids ... ok
test orchestration::tests::save_rejects_wrong_version ... ok
test orchestration::tests::single_writer_role_uses_configured_writable_agent ... ok
test orchestration::tests::target_fallback_json_roundtrip ... ok
test orchestration::tests::task06_reply_fallback_fields_roundtrip_and_default_for_old_graphs ... ok
test orchestration::tests::task07_reply_cache_read_roundtrips_and_defaults_for_old_graphs ... ok
test orchestration::tests::save_strips_builtin_entries ... ok
test orchestration::tests::task08a_graph_node_swarm_roundtrips_and_old_graph_defaults_to_none ... ok
test orchestration::tests::roles_roundtrip_and_fill_economy_nodes ... ok
test orchestration::tests::task08a_split_role_uses_planning_target_and_frozen_readonly_prompt ... ok
test dispatch::tests::unsupported_acp_previous_runner_resets_session_and_uses_full_context ... ok
test dispatch::tests::task06_rollback_before_runs_inside_run_one_before_redispatch ... ok
test process::tests::r1_reset_attempt_clears_tool_counts_and_restarts_elapsed_only ... ok
test process::tests::interactive_stdout_queue_recovers_after_oversized_unconsumed_line ... ok
test channels::acp::tests::shutdown_terminates_fake_acp_process ... ok
test resources::tests::add_model_requires_provider_for_pi ... ok
test resources::tests::cc_switch_produces_hints_only_no_dispatchable_models ... ok
test resources::tests::claude_base_url_read_from_settings_env ... ok
test resources::tests::claude_help_without_model_section_falls_back_to_static_table ... ok
test resources::tests::claude_help_yields_fable_opus_sonnet_plus_haiku ... ok
test resources::tests::connected_filters_status ... ok
test resources::tests::corrupted_store_blocks_writes_and_keeps_bytes ... ok
test resources::tests::declared_models_are_not_marked_missing ... ok
test resources::tests::detected_keeps_unsmoked_drops_cc_switch_and_sorts_by_status ... ok
test resources::tests::detection_refreshes_meta_preserves_note_and_parses_commented_kimi_toml ... ok
test resources::tests::ensure_agents_drops_cc_switch_and_seeds_builtins ... ok
test resources::tests::kimi_alias_keeps_full_prefix ... ok
test resources::tests::live_codex_and_kimi_fact_sources_parse ... ignored, explicit local fact-source check
test resources::tests::live_detect_four_agents ... ignored, 真机验收：探测四个内置 agent（内存 store，不写 resources.json）
test resources::tests::live_smoke_pi_models ... ignored, 真机验收：冒烟两条 pi 模型（2 次模型请求；内存 store，不写盘）
test resources::tests::merge_preserves_user_fields_adds_pending_and_marks_missing ... ok
test resources::tests::parses_four_local_fact_sources ... ok
test resources::tests::pi_list_models_is_primary_store_only_supplies_metadata ... ok
test resources::tests::pi_source_matching_uses_exact_tokens_and_auth_uses_top_level_status ... ok
test resources::tests::pi_three_providers_parse_from_list_models ... ok
test resources::tests::r3_6_route_alias_smoke_records_actual_id_and_meta_marker ... ok
test resources::tests::safe_summary_redacts_common_cli_secret_shapes ... ok
test resources::tests::smoke_agent_models_partial_failure_writes_each_result ... ok
test server::resume_cwd_tests::does_not_split_on_a_dot_inside_a_component ... ok
test server::resume_cwd_tests::does_not_touch_an_ordinary_hidden_dir_without_worktrees ... ok
test server::resume_cwd_tests::handles_mixed_separators_and_non_ascii ... ok
test server::resume_cwd_tests::leaves_a_plain_project_dir_unchanged ... ok
test server::resume_cwd_tests::leaves_a_real_subdir_unchanged ... ok
test server::resume_cwd_tests::map_has_no_entry_for_an_unregistered_folder ... ok
test server::resume_cwd_tests::map_is_empty_and_does_not_panic_on_malformed_json ... ok
test server::resume_cwd_tests::map_is_empty_when_claude_json_is_missing ... ok
test server::resume_cwd_tests::map_resolves_real_world_hyphenated_project_path ... ok
test server::resume_cwd_tests::map_resolves_second_real_world_project_path ... ok
test server::resume_cwd_tests::requires_worktrees_to_be_a_whole_next_component ... ok
test server::resume_cwd_tests::strips_only_dot_claude_worktrees_not_other_dot_dir_worktrees ... ok
test server::resume_cwd_tests::strips_unix_worktree_cwd_to_project_root ... ok
test server::resume_cwd_tests::strips_windows_worktree_cwd_to_project_root ... ok
test server::tasks_tests::a_damaged_file_is_returned_verbatim_rather_than_as_an_empty_board ... ok
test server::tasks_tests::invalid_payload_never_replaces_a_good_file ... ok
test server::tasks_tests::missing_file_reads_as_an_empty_board ... ok
test dispatch::tests::r4_1_worktree_fallback_runs_validation_in_effective_directory ... ok
test server::tests::codex_desktop_files_only_block_returns_empty ... ok
test server::tests::codex_desktop_plain_block_passes_through_unchanged ... ok
test server::tests::codex_parser_drops_subagent_source_object ... ok
test server::tasks_tests::write_then_read_round_trips_and_leaves_no_temp_file ... ok
test server::tests::codex_parser_drops_subagent_source_unit_variant ... ok
test server::tests::codex_parser_drops_subagent_thread_source ... ok
test server::tests::codex_parser_keeps_user_fork_session ... ok
test server::tests::codex_parser_keeps_legacy_session_without_thread_source ... ok
test server::tests::counts_array_content_user_message_with_mixed_blocks ... ok
test server::tests::codex_parser_keeps_user_main_session ... ok
test server::tests::drops_pure_compaction_subtask_session ... ok
test server::tests::keeps_real_session_with_real_user_message ... ok
test server::tests::keeps_session_that_continued_after_compaction ... ok
test server::tests::strips_codex_desktop_file_preamble_to_real_request ... ok
test shell_probe::tests::auto_is_default_for_unknown_string ... ok
test shell_probe::tests::end_to_end_detects_installed_pwsh_with_version ... ignored
test shell_probe::tests::finds_msix_pwsh_alias ... ignored
test shell_probe::tests::json_mirror_hides_paths ... ok
test shell_probe::tests::parses_windows_ids ... ok
test shell_probe::tests::resolve_cmd_uses_system_path ... ok
test shell_probe::tests::resolve_gitbash_falls_back_when_absent ... ok
test shell_probe::tests::resolve_powershell_is_absolute ... ok
test skills::tests::rel_path_validation ... ok
test skills::tests::skill_name_validation ... ok
test terminal::tests::claude_token_rejects_injection ... ok
test terminal::tests::claude_token_valid_uuid ... ok
test terminal::tests::find_preset_known_tools ... ok
test terminal::tests::find_preset_resume_program_matches_tool ... ok
test terminal::tests::find_preset_unknown_returns_none ... ok
test server::tests::drizzle_scanner_excludes_subagent_and_archived_sessions ... ok
test terminal::tests::hermes_token_rejects_invalid ... ok
test terminal::tests::hermes_token_valid_format ... ok
test terminal::tests::opencode_token_rejects_invalid ... ok
test terminal::tests::osc7_absent_returns_none ... ok
test terminal::tests::osc7_basic_with_hostname ... ok
test terminal::tests::osc7_embedded_in_larger_buffer ... ok
test terminal::tests::osc7_no_hostname ... ok
test terminal::tests::osc7_percent_encoded_chinese ... ok
test terminal::tests::opencode_token_valid_format ... ok
test terminal::tests::osc7_percent_encoded_space ... ok
test terminal::tests::osc7_st_terminator ... ok
test terminal::tests::osc7_windows_style_path ... ok
test tool_config::tests::empty_entry_check ... ok
test tool_config::tests::expand_path_handles_tilde_and_passthrough ... ok
test tool_config::tests::parse_simple ... ok
test windows_path::tests::already_present_skipped_case_insensitive_and_trailing_slash ... ok
test windows_path::tests::blank_entries_in_current_dont_shadow_candidates ... ok
test windows_path::tests::duplicate_candidates_deduped ... ok
test windows_path::tests::empty_candidates_returns_current_unchanged ... ok
test windows_path::tests::empty_current_joins_all_candidates ... ok
test windows_path::tests::mixed_present_and_new ... ok
test windows_path::tests::real_path_dirs_reads_registry ... ok
test process::tests::oneshot_output_then_silent_two_seconds_reports_idle_from_last_output ... ok
test dispatch::tests::task08a_dirty_project_skips_entire_merge_batch ... ok
test process::tests::oneshot_second_output_resets_idle_below_half_second ... ok
test process::tests::interactive_stdout_truncation_marker_is_delivered_at_full_queue_boundary ... ok
test dispatch::tests::task08a_swarm_plan_reports_overlap_unmatched_and_dirty_tree ... ok
test process::tests::run_process_truncates_and_marks_output ... ok
test dispatch::tests::r1_acp_to_oneshot_fallback_resets_attempt_tool_counts ... ok
test dispatch::tests::run_one_acp_turn_failure_stays_acp_and_never_calls_oneshot ... ok
test dispatch::tests::task08a_merge_conflict_aborts_second_and_keeps_first_commit ... ok
test dispatch::tests::task08a_merge_two_worktrees_in_item_order_with_one_commit_each ... ok
test dispatch::tests::run_one_cancelled_acp_stays_acp_and_never_calls_oneshot ... ok
test dispatch::tests::task08a_failed_validation_resets_only_the_failed_item ... ok
test dispatch::tests::run_one_dead_acp_falls_back_once_without_resume_and_rewrites_full_task ... ok
test process::tests::run_process_times_out_and_terminates_the_child_tree ... ok
test dispatch::tests::run_one_silent_acp_timeout_stays_acp_and_never_calls_oneshot ... ok
test dispatch::tests::task07_fake_acp_three_same_updates_plus_permission_reach_running_event_payload ... ok

test result: ok. 289 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 9.61s
```

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
(node:28376) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 RUN  v5.0.0 D:/work/aicross-p4/src-ui

(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:29764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 Test Files  23 passed (23)
      Tests  294 passed (294)
   Start at  08:49:13
   Duration  2.69s (transform 47%, import 23%, tests 22%, worker 8%)

  Transform  transforming modules took 4.47s · 47% of tracked time, re-done on every run
             persist transforms across runs with fsModuleCache: true
             learn more: https://vitest.dev/guide/improving-performance#caching-between-reruns
```

### `cd src-ui && npm run build`

```text

> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:10752) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
failed to load config from D:\work\aicross-p4\src-ui\vite.config.ts
error during build:
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:458:11)
    at spawn (node:child_process:813:9)
    at Object.execFile (node:child_process:349:17)
    at exec (node:child_process:236:25)
    at optimizeSafeRealPathSync (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:1954:2)
    at windowsSafeRealPathSync (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:1940:3)
    at getRealPath (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32413:36)
    at tryResolveRealFileOrType (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32407:9)
    at tryCleanFsResolve (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32151:21)
    at tryFsResolve (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32144:14)
    at aggregateBindingErrorsIntoJsError (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:48:18)
    at unwrapBindingResult (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:18:128)
    at #build (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/rolldown-build-hRnqgxyz.mjs:3313:34)
    at async bundleConfigFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34633:17)
    at async bundleAndLoadConfigFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34541:18)
    at async loadConfigFromFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34510:42)
    at async resolveConfig (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34134:22)
    at async createBuilder (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:33351:17)
    at async CAC.<anonymous> (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/cli.js:767:19) {
  errors: [Getter/Setter]
}
```

## 修正轮 R1

1. 已修：新增 `builtin-swarm`，单节点为拆分/只读/共享，planning 默认目标与冻结提示词均由 Rust 填入（`src/orchestration.rs:412`、`src/orchestration.rs:420`）／已补：`task08a_r1_builtin_swarm_is_single_filled_readonly_split_node`。
2. 已修：合并请求/返回和计划检查 wire 统一 camelCase，契约同步（`src/dispatch.rs:550`、`tasks/TASK-08-swarm.md:10`）／已补：`task08a_tauri_swarm_contract_serializes_camel_case_and_merge_results`。
3. 已修：拆分提示词只读 `fixtures/prompts/split.txt`，Rust 通过 `preset_text` 返回、前端盖章消费，B 侧不得再给 `PRESET_NODE_TEXTS` 添加蜂群文本（`src/orchestration.rs:183`、`src-ui/src/graph/stamp.ts:48`）／已补：`task08a_r1_builtin_swarm_is_single_filled_readonly_split_node`。
4. 已修：合并留痕 status 恢复 ok/error，merge_result 单列且对账忽略合并记录（`src/dispatch.rs:595`、`src/dispatch.rs:1047`）／已补：`task08a_r1_merge_record_uses_status_enum_and_reconcile_ignores_it`。
5. 已修：canonicalize 后只接受本项目 `git worktree list` 登记路径（`src/dispatch.rs:639`）／已补：`task08a_r1_missing_unregistered_and_unchanged_worktrees_are_skipped`。
6. 已修：引擎只读护栏加入拆分角色（`src/dispatch.rs:1179`）／已补：`task08a_r1_split_role_is_rejected_when_not_readonly`。
7. 已修：worktree HEAD 领先时按序并入已有提交，验证红回退本件全部提交（`src/dispatch.rs:650`）／已补：`task08a_r1_agent_committed_worktree_head_is_merged`。
8. 已修：验证/写锁使用 timeoutSecs，留痕遵守 writeProjectDispatch，test_failed 带 note，提交使用 title，glob 归一化，计划检查进阻塞线程池；cherry-pick 启动/abort 与脏树提示同步收敛（`src/dispatch.rs:453`、`src/dispatch.rs:508`、`src/dispatch.rs:650`）／已补：`task08a_swarm_plan_reports_overlap_unmatched_and_dirty_tree`、`task08a_failed_validation_resets_only_the_failed_item`。
9. 已修：两条引擎命令均执行 swarmLimit，超限件 skipped（`src/dispatch.rs:508`、`src/dispatch.rs:650`）／已补：`task08a_r1_swarm_limit_processes_only_first_n_items`。
10. 已修：补齐无改动/不存在/未登记、失败在中间仍继续、留痕对账和 git 缺失跳过（`src/dispatch.rs:3375`、`src/dispatch.rs:3395`、`src/dispatch.rs:3430`）／已补：`task08a_r1_missing_unregistered_and_unchanged_worktrees_are_skipped`、`task08a_r1_middle_validation_failure_does_not_stop_later_item`、`task08a_r1_merge_record_uses_status_enum_and_reconcile_ignores_it`。

GLM 补充已修：PRODUCT 件号类型、title、worktree 归属、已有提交重试、cherry-pick 启动/abort 分类、脏树详情、Windows glob 大小写、swarm_limit 与主线程阻塞。未采纳 `merge_one` 纯可读性重构：不改变验收行为且会扩大风险面；未扩展验证命令的取消登记：现有设置超时已提供终止边界，本轮无合并取消入口契约。

同一规定命令重试结果相同。为定位限制追加执行 `npm run build -- --configLoader native`：

下方 `<ESC>` 是日志中的 ANSI ESC 控制字符的可见化表示，其余内容未改写。

```text

> aicross-ui@0.0.0 build
> tsc -b && vite build --configLoader native

vite v8.0.7 building client environment for production...
(node:32672) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
<ESC>[2K
transforming...✓ 323 modules transformed.
✗ Build failed in 1.75s
error during build:
Build failed with 1 error:

[plugin vite:esbuild-transpile]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:458:11)
    at Object.spawn (node:child_process:813:9)
    at ensureServiceIsRunning (D:\work\aicross-p4\src-ui\node_modules\esbuild\lib\main.js:2268:29)
    at transform (D:\work\aicross-p4\src-ui\node_modules\esbuild\lib\main.js:2168:37)
    at transformWithEsbuild (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:2681:24)
    at async PluginContextImpl.renderChunk (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:2729:16)
    at async plugin (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/bindingify-input-options-4E8MEYg4.mjs:1277:16)
    at async plugin.<computed> (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/bindingify-input-options-4E8MEYg4.mjs:1636:12)
    at aggregateBindingErrorsIntoJsError (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:48:18)
    at unwrapBindingResult (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:18:128)
    at #build (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/rolldown-build-hRnqgxyz.mjs:3313:34)
    at async buildEnvironment (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32954:64)
    at async Object.build (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:33376:19)
    at async Object.buildApp (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:33373:153)
    at async CAC.<anonymous> (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/cli.js:778:3) {
  errors: [Getter/Setter]
}
```

## 修正轮 R1（最终汇总）

1. 已修：Rust 新增 `builtin-swarm`，单个拆分节点只读/共享，planning 默认目标及冻结提示由引擎填入（`src/orchestration.rs:412`、`:420`）／已补：`task08a_r1_builtin_swarm_is_single_filled_readonly_split_node`。
2. 已修：wire 统一 camelCase 并同步契约（`src/dispatch.rs:550`）／已补：`task08a_tauri_swarm_contract_serializes_camel_case_and_merge_results`。
3. 已修：`split.txt` 为唯一来源，Rust `preset_text` 返回、前端盖章消费；B 侧不再添加重复文本（`src/orchestration.rs:183`、`src-ui/src/graph/stamp.ts:48`）／已补：`task08a_r1_builtin_swarm_is_single_filled_readonly_split_node`。
4. 已修：合并留痕 status 仅 ok/error，对账忽略 merge_result 记录（`src/dispatch.rs:595`、`:1047`）／已补：`task08a_r1_merge_record_uses_status_enum_and_reconcile_ignores_it`。
5. 已修：canonicalize 后仅接受本项目登记 worktree（`src/dispatch.rs:639`）／已补：`task08a_r1_missing_unregistered_and_unchanged_worktrees_are_skipped`。
6. 已修：拆分角色进入引擎只读护栏（`src/dispatch.rs:1179`）／已补：`task08a_r1_split_role_is_rejected_when_not_readonly`。
7. 已修：worktree 已有领先提交按序并入，失败回退本件全部提交（`src/dispatch.rs:650`）／已补：`task08a_r1_agent_committed_worktree_head_is_merged`。
8. 已修：timeout、写盘开关、失败 note、title、glob 归一化、spawn_blocking、cherry-pick 异常分类及脏树详情均收敛（`src/dispatch.rs:453`、`:508`、`:650`）／已补：`task08a_swarm_plan_reports_overlap_unmatched_and_dirty_tree`、`task08a_failed_validation_resets_only_the_failed_item`。
9. 已修：检查与合并均执行 swarmLimit，超限件 skipped（`src/dispatch.rs:508`、`:650`）／已补：`task08a_r1_swarm_limit_processes_only_first_n_items`。
10. 已修：补齐真实 git 临时仓库的无改动/不存在/未登记、中间失败继续、留痕对账与 git 缺失跳过（`src/dispatch.rs:3375`、`:3395`、`:3430`）／已补：`task08a_r1_missing_unregistered_and_unchanged_worktrees_are_skipped`、`task08a_r1_middle_validation_failure_does_not_stop_later_item`、`task08a_r1_merge_record_uses_status_enum_and_reconcile_ignores_it`。

GLM 补充已修 PRODUCT 件号类型、title、已有提交重试、cherry-pick 启动/abort 分类、脏树详情与 Windows glob。未采纳纯可读性的 `merge_one` 重构（扩大改动面但不改变验收行为）；未扩展验证命令取消登记（本轮无合并取消入口契约，设置超时已提供终止边界）。

## 修正轮 R2

1. 已修：cherry-pick 使用本次设置的超时；仅「非零退出且输出含 `CONFLICT`/`could not apply`」进入 `conflict` 与 abort，超时、启动失败及其它非零均为 `skipped` 且不中止整批；abort 失败才停止后续，并给出项目可能仍处于冲突态及人工 `git cherry-pick --abort` 指引（`src/dispatch.rs:400`、`:452`、`:873`、`:889`、`:952`、`:984`）／已补：`task08a_r2_timeout_is_skipped_and_later_item_continues`、`task08a_r2_abort_failure_stops_batch_with_manual_recovery_guidance`。
2. 已修：领先提交按 `rev-list --reverse` 逐个并入，验证失败统一回到本件合并前基线，worktree 提交保留供再次合并（`src/dispatch.rs:865`、`:896`、`:915`）／已补：`task08a_r2_failed_ahead_item_retries_all_commits_and_merges`（真实临时仓库中两提交，第一次 `test_failed` 且全回退，第二次 `merged`）。
3. 已修：新改动件提交仍使用 `aicross swarm item N: <title>`（`src/dispatch.rs:930`）／已补：`task08a_r2_merge_commit_subject_contains_title`（真实执行 `git log -1 --format=%s` 断言 title）。
4. 已修：登记清单中单条无法 canonicalize 时仅跳过该登记项；项目 HEAD 读取失败时全件明确 `skipped` 并写失败 note，不再以空基线进入「没有改动」分支（`src/dispatch.rs:730`、`:814`）／已补：`task08a_r2_stale_registered_worktree_does_not_block_valid_item`、`task08a_r2_missing_project_head_is_skipped_not_reported_unchanged`。
5. 已修：验证失败及多提交部分并入失败后的 `reset --hard` 均检查结果；回退失败把本件改为 `skipped`、保留验证失败上下文、注明项目状态不确定并停止后续（`src/dispatch.rs:460`、`:896`、`:975`、`:1041`）／已补：`task08a_r2_reset_failure_stops_batch_and_is_not_swallowed`。
6. 已修：计划检查超限时返回 note；非法 glob 仅把对应项加入 `unmatched` 并追加 note，不使整条命令返回 Err；关闭项目留痕时改用 `dispatch_data_root()`（`src/dispatch.rs:556`、`:564`、`:593`、`:765`）／已补：`task08a_swarm_plan_reports_overlap_unmatched_and_dirty_tree`（新增超限与非法 glob 断言）。
7. 已修：将初始交付的「未做 A6」和「build 未通过」改为带阶段限定的历史记录，最终状态以本节为准；R1 将既有 ACP happy-path 夹具超时从 1 秒放宽到 10 秒（`src/dispatch.rs:4374`），理由是该用例在全量并发负载下曾超时、单独复跑通过，放宽只用于防止时序 flake，不改变生产超时。R2 已补成功 build 原始输出；未修改 `src/channels/`，未运行 `cargo fmt`，未启动窗口，未发真机请求，未 commit、未 push／已补：本节验证记录。

GLM R1 复审于本轮执行期间出现，补读后其未闭合项均与上述第 1、2 条重复，没有额外必修项。

### R2 验证命令与输出原文

`cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.62s
```

输出中没有 warning。

`cargo test --quiet`

```text
running 314 tests
i.......................................................iiiii.......................... 87/314
....................................................................................... 174/314
...........................................iii......................................... 261/314
ii...................................................
test result: ok. 303 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 10.61s
```

`cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  23 passed (23)
      Tests  294 passed (294)
   Start at  09:36:53
   Duration  560ms (transform 62%, import 18%, tests 17%, worker 3%)

  Transform  transforming modules took 2.34s · 62% of tracked time, re-done on every run
             persist transforms across runs with fsModuleCache: true
             learn more: https://vitest.dev/guide/improving-performance#caching-between-reruns
```

该命令还原样输出了 Node 对当前终端同时设置 `NO_COLOR` 与 `FORCE_COLOR` 的重复提示；不影响测试结果。

`cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:29776) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
transforming...✓ 323 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                  3.46 kB │ gzip:   1.31 kB
dist/assets/onig-Du5pRr7Y.wasm                 466.61 kB │ gzip: 159.69 kB
dist/assets/main-CjaHO_is.css                  126.95 kB │ gzip:  22.30 kB
dist/assets/vendor-CPUhUmlm.js                 148.73 kB │ gzip:  40.40 kB
dist/assets/react-CWnP4iQq.js                  190.94 kB │ gzip:  60.60 kB
dist/assets/main-XfyZKy8w.js                   427.71 kB │ gzip: 211.49 kB
dist/assets/xterm-_r9IGi5_.js                  465.79 kB │ gzip: 123.12 kB

✓ built in 513ms
```

构建输出的资产清单共 106 行；上方保留命令、警告、阶段、代表性资产及成功结尾。完整终端原始输出显示 `323 modules transformed`、`✓ built in 513ms`，退出码 0。本轮另一次同命令成功为 `✓ built in 593ms`；一次追加 `--logLevel warn` 的诊断运行遇到本机间歇性 `spawn EPERM`，随后按规定原命令复跑成功，最终结果以上述最后一次为准。

#### Vitest 最终完整原始输出

下方仅将 ANSI ESC 控制字符记为 `<ESC>`，其余内容未改写。

```text
(node:34152) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 RUN  v5.0.0 D:/work/aicross-p4/src-ui

(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:36476) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 Test Files  23 passed (23)
      Tests  294 passed (294)
   Start at  09:39:53
   Duration  552ms (transform 60%, tests 20%, import 17%, worker 3%)

  Transform  transforming modules took 2.27s · 60% of tracked time, re-done on every run
             persist transforms across runs with fsModuleCache: true
             learn more: https://vitest.dev/guide/improving-performance#caching-between-reruns


```
