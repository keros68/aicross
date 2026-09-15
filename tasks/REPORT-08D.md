# REPORT-08D 蜂群按声明范围合并

范围：按 `TASK-08D-merge-by-range.md` 1→5 修改 `src/dispatch.rs` 的合并循环与测试，并更新 `tasks/TASK-08-swarm.md` 契约节。未运行 `cargo fmt`，未修改 `src/channels/`，未启动窗口，未发真机请求，未 commit、未 push。

## 逐条交付

### 1. 合并源改为声明范围内的 diff

已做：`SwarmMergeItem` 追加 `files`（`src/dispatch.rs:678`）；每件用本批项目初始 HEAD 与 worktree HEAD 的 `git merge-base` 定基线，收集已提交和未提交改动，只暂存声明范围，生成 `--binary --full-index` 补丁，再在项目中执行 `git apply --3way --index` 并创建一件一个提交（`src/dispatch.rs:814`、`src/dispatch.rs:892`、`src/dispatch.rs:998`、`src/dispatch.rs:1063`、`src/dispatch.rs:1099`）。缺少 `files` 的旧请求安全 `skipped`，不退化为全仓合并（`src/dispatch.rs:996`）。

已补：`task08d_disjoint_ranges_sharing_tracked_pyc_both_merge`；`task08d_merge_wire_defaults_missing_declared_files_to_safe_skip`。

### 2. 范围外改动与垃圾目录

已做：按路径任一分量复用快照排除清单；声明范围外或命中排除目录的改动不进补丁，只在 note 写出路径（`src/dispatch.rs:786`、`src/dispatch.rs:1025`）。

已补：`task08d_out_of_range_and_excluded_files_are_not_applied`；`task08d_disjoint_ranges_sharing_tracked_pyc_both_merge`。后者在两个真实 worktree 同时改动同一个被跟踪 pyc，并各改一件互不相交的声明文件，断言两件都为 `merged`、pyc 保持基线内容。

### 3. 验证后清理

已做：每件验证后读取 `git status --porcelain -z`；只有排除目录内的被跟踪文件变脏时逐路径 checkout，未跟踪文件不动；存在排除目录外改动时写「验证命令改动了项目文件：…」，标为 `test_failed` 并回退本件（`src/dispatch.rs:858`、`src/dispatch.rs:1121`、`src/dispatch.rs:1137`）。

已补：`task08d_validation_cleans_tracked_excluded_changes`；`task08d_validation_project_change_fails_and_rolls_back_item`。

### 4. 保留 R2 分级语义

已做：补丁 apply 超时或启动失败为 `skipped`；范围内补丁非零为 `conflict`，note 列冲突文件和 worktree；冲突后恢复工作区和索引，恢复失败或验证清理失败则停止后续并给人工恢复指引；`swarm_limit`、项目写锁、worktree 登记校验、留痕与对账行为未改（`src/dispatch.rs:429`、`src/dispatch.rs:504`、`src/dispatch.rs:1080`、`src/dispatch.rs:1090`、`src/dispatch.rs:1174`）。

已补：`task08d_apply_timeout_is_skipped_and_later_item_continues`；`task08d_restore_failure_stops_batch_with_manual_recovery_guidance`；`task08d_in_range_conflict_restores_clean_project_and_keeps_first_item`。原 R2 的验证失败回退、回退失败停批、件数上限、登记校验和留痕测试继续通过。

### 5. 契约改为按声明范围取 diff

已做：合并请求契约追加 `files`，把 cherry-pick 口径替换为按声明范围生成并应用 diff，并写明排除被跟踪 pyc/构建产物、验证后清理和恢复失败分级（`tasks/TASK-08-swarm.md:10`）。

已补：`task08d_merge_wire_defaults_missing_declared_files_to_safe_skip`，并在既有 `task08a_tauri_swarm_contract_serializes_camel_case_and_merge_results` 中断言 `files` 的 camelCase wire 值。

## 没做与边界

- 没改 `src-ui/`：任务限定只动 `src/dispatch.rs`、其测试和契约节。当前前端 `swarmMergeItems` 尚未把节点的 `swarm.files` 放入合并请求，因此现有界面发出的旧形状请求会被后端安全跳过并提示「未声明文件范围」；要从界面端实际启用 08D，还需后续在前端 wire 类型和组包处追加 `files`。本报告不把前端端到端接通写成已完成。
- 没按任务文字机械使用一次 `git checkout -- . && git reset -q` 就结束：真实 Git 的 unmerged 路径会让普通 checkout 失败，且强制 checkout 后仍会留下冲突标记。实现采用受检查的 `git checkout -f -- .`、`git reset -q`、再 `git checkout -- .`，真实冲突测试证明最终项目已恢复干净。

## 验证命令与输出原文

### `cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.26s
```

输出中没有 warning。

### `cargo test --quiet`

```text
running 334 tests
i........................................................iiiii......................... 87/334
....................................................................................... 174/334
....................................................iii................................ 261/334
....................ii...................................................
test result: ok. 323 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 11.15s
```

此前还按原命令运行过 `cargo test`，结果同为 323 passed、0 failed、11 ignored；`--quiet` 复跑用于在报告中保留完整且不省略的原始输出。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

复跑前仅清除当前进程继承的 `FORCE_COLOR`，命令本身不变，原始输出如下：

```text

 RUN  v5.0.0 D:/work/aicross-p4/src-ui


 Test Files  26 passed (26)
      Tests  360 passed (360)
   Start at  11:19:07
   Duration  621ms (transform 64%, import 17%, tests 16%, worker 3%)

  Transform  transforming modules took 3.22s · 64% of tracked time, re-done on every run
             persist transforms across runs with fsModuleCache: true
             learn more: https://vitest.dev/guide/improving-performance#caching-between-reruns
```

### `cd src-ui && npm run build`

下方只把 ANSI 控制字符去掉，文字内容未改写；构建输出中的资产清单完整保留。

```text

> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:36292) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
transforming...✓ 327 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                                 3.46 kB │ gzip:   1.32 kB
dist/assets/onig-Du5pRr7Y.wasm                                466.61 kB │ gzip: 159.69 kB
dist/assets/xterm-sLQPUA_C.css                                  4.00 kB │ gzip:   1.02 kB
dist/assets/model-picker-Dx7sti2H.css                            4.75 kB │ gzip:   1.40 kB
dist/assets/resources-D3DO20Wr.css                               8.33 kB │ gzip:   1.83 kB
dist/assets/SettingsModal-BlDZGgPL.css                          14.64 kB │ gzip:   2.88 kB
dist/assets/main-BktGH3Ox.css                                  127.10 kB │ gzip:  22.36 kB
dist/assets/dist-js-C9A_UucY.js                                  0.06 kB │ gzip:   0.07 kB
dist/assets/event-CuZFo1Eh.js                                    0.06 kB │ gzip:   0.07 kB
dist/assets/github-dark-default-DgYFsgBk.js                      0.06 kB │ gzip:   0.07 kB
dist/assets/github-light-default-BCy-jVco.js                     0.06 kB │ gzip:   0.07 kB
dist/assets/app-CgFyQySp.js                                      0.06 kB │ gzip:   0.08 kB
dist/assets/window-BUySKeMh.js                                   0.07 kB │ gzip:   0.08 kB
dist/assets/webview-D2RcBque.js                                  0.07 kB │ gzip:   0.08 kB
dist/assets/c-BhhWwbyw.js                                        0.09 kB │ gzip:   0.10 kB
dist/assets/go-Cto0HdKH.js                                       0.09 kB │ gzip:   0.10 kB
dist/assets/cpp-kvbzRYwQ.js                                      0.09 kB │ gzip:   0.10 kB
dist/assets/css-DzgGK70y.js                                      0.09 kB │ gzip:   0.10 kB
dist/assets/jsx-DWh0IJuu.js                                      0.09 kB │ gzip:   0.10 kB
dist/assets/mdx-mc07Ne2a.js                                      0.09 kB │ gzip:   0.10 kB
dist/assets/php-Du8jd4kT.js                                      0.09 kB │ gzip:   0.10 kB
dist/assets/sql-DhQaC_ST.js                                      0.09 kB │ gzip:   0.10 kB
dist/assets/tsx-C_EyipRR.js                                      0.09 kB │ gzip:   0.10 kB
dist/assets/vue-CjUPUJpD.js                                      0.09 kB │ gzip:   0.10 kB
dist/assets/xml-C3E8aJkh.js                                      0.09 kB │ gzip:   0.10 kB
dist/assets/html-ZycoJ1lK.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/java-Bl8wispM.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/json-BZdaHLul.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/less-DiZKyOVk.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/ruby-HTANXuLd.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/rust-R7X5em5q.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/sass-X9LXKHST.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/scss-jLYg7q0r.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/toml-XHBGgTos.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/yaml-Ss_NIIhz.js                                     0.09 kB │ gzip:   0.10 kB
dist/assets/swift-BuhqnOB_.js                                    0.09 kB │ gzip:   0.10 kB
dist/assets/csharp-BR5aFhBe.js                                   0.09 kB │ gzip:   0.10 kB
dist/assets/docker-BXqrIeZV.js                                   0.09 kB │ gzip:   0.10 kB
dist/assets/kotlin-Cq_-4jec.js                                   0.09 kB │ gzip:   0.10 kB
dist/assets/python-CWm9lwjy.js                                   0.09 kB │ gzip:   0.10 kB
dist/assets/svelte-BwryZbIn.js                                   0.09 kB │ gzip:   0.10 kB
dist/assets/markdown-CfaaU-by.js                                 0.09 kB │ gzip:   0.10 kB
dist/assets/javascript-DBe5iz3I.js                               0.10 kB │ gzip:   0.11 kB
dist/assets/typescript-DgNyJxh1.js                               0.10 kB │ gzip:   0.11 kB
dist/assets/bash-BMT7fr4i.js                                     0.10 kB │ gzip:   0.11 kB
dist/assets/engine-oniguruma-CUPo-3vi.js                         0.17 kB │ gzip:   0.14 kB
dist/assets/lang-vue-interpolations-DSV92BsH.js                  0.40 kB │ gzip:   0.24 kB
dist/assets/lang-vue-directives-BpYOJEc5.js                      0.42 kB │ gzip:   0.27 kB
dist/assets/rolldown-runtime-km5iIlDX.js                         0.68 kB │ gzip:   0.41 kB
dist/assets/defineProperty-CzYKe_pn.js                           0.71 kB │ gzip:   0.39 kB
dist/assets/lang-markdown-vue-CBO-3uK2.js                        0.75 kB │ gzip:   0.38 kB
dist/assets/lang-html-derivative-CoJpQrZm.js                     0.83 kB │ gzip:   0.46 kB
dist/assets/lang-vue-sfc-style-variable-injection-CGHzv4ky.js    0.94 kB │ gzip:   0.43 kB
dist/assets/core-CX3XEUCs.js                                     1.33 kB │ gzip:   0.62 kB
dist/assets/lang-docker-_OIk34xS.js                              1.73 kB │ gzip:   0.59 kB
dist/assets/resources-C2LrC12F.js                                1.81 kB │ gzip:   0.90 kB
dist/assets/lang-json-CwMwdRMR.js                                2.81 kB │ gzip:   0.77 kB
dist/assets/dispatch-settings-D5E7c-vx.js                        3.55 kB │ gzip:   1.68 kB
dist/assets/lang-glsl-dAjUAX10.js                                3.63 kB │ gzip:   1.41 kB
dist/assets/lang-xml-BXuWBzo8.js                                 5.38 kB │ gzip:   1.21 kB
dist/assets/lang-postcss-uUyjoxlZ.js                             6.40 kB │ gzip:   1.90 kB
dist/assets/lang-toml-DdlhiNcx.js                                6.41 kB │ gzip:   1.28 kB
dist/assets/tauri-BmIJNC_2.js                                    6.66 kB │ gzip:   2.60 kB
dist/assets/lang-regexp-PkKp9FJr.js                              7.98 kB │ gzip:   1.41 kB
dist/assets/lang-haml-BSPUMTuO.js                                8.27 kB │ gzip:   1.82 kB
dist/assets/lang-kotlin-sWY7ArP7.js                              8.77 kB │ gzip:   2.13 kB
dist/assets/SkillsPanel-i8FVrQoE.js                              9.18 kB │ gzip:   3.38 kB
dist/assets/lang-sass-BauQOl7Q.js                                9.28 kB │ gzip:   2.49 kB
dist/assets/app-state-D49IauP-.js                               10.43 kB │ gzip:   3.26 kB
dist/assets/lang-yaml-CuAmHpom.js                               10.49 kB │ gzip:   2.26 kB
dist/assets/ResourcesOverlay-CCxro0Q3.js                        11.24 kB │ gzip:   3.66 kB
dist/assets/lang-rust-BVU9p_Ey.js                               15.05 kB │ gzip:   2.71 kB
dist/assets/lang-lua-B42IYw08.js                                15.53 kB │ gzip:   3.15 kB
dist/assets/lang-graphql-Cleegyn5.js                            18.03 kB │ gzip:   2.50 kB
dist/assets/lang-svelte-DpdpUH4Q.js                             18.26 kB │ gzip:   3.14 kB
dist/assets/lang-vue-6b42Py3N.js                                22.35 kB │ gzip:   2.54 kB
dist/assets/lang-sql-Dkaro8Br.js                                23.39 kB │ gzip:   7.43 kB
dist/assets/lang-scss-DHs9xDPW.js                               27.20 kB │ gzip:   4.19 kB
dist/assets/lang-java-BmwQdcBP.js                               27.21 kB │ gzip:   4.25 kB
dist/assets/markdown-B96puryu.js                                30.02 kB │ gzip:  10.85 kB
dist/assets/SettingsModal-B3V4XH5N.js                           30.50 kB │ gzip:   9.20 kB
dist/assets/lang-shellscript-ClIYzLfq.js                        41.47 kB │ gzip:   6.07 kB
dist/assets/lang-ruby-Bc0Yd8v4.js                               46.65 kB │ gzip:   5.71 kB
dist/assets/lang-go-CtQjvmW3.js                                 46.81 kB │ gzip:   5.15 kB
dist/assets/lang-css-BZYgv9eh.js                                49.02 kB │ gzip:  11.90 kB
dist/assets/lang-html-BZHVI7eq.js                               57.25 kB │ gzip:  11.74 kB
dist/assets/lang-markdown-BQDdzsCg.js                           59.32 kB │ gzip:   5.66 kB
dist/assets/lang-python-DRoGfMiU.js                             69.94 kB │ gzip:   9.10 kB
dist/assets/lang-c-CghvNN5f.js                                  72.10 kB │ gzip:  10.50 kB
dist/assets/lang-swift-ty8Frjwc.js                              86.67 kB │ gzip:  14.69 kB
dist/assets/lang-csharp-B402LPno.js                             89.67 kB │ gzip:  10.64 kB
dist/assets/lang-less-CWcMZZEH.js                               97.62 kB │ gzip:  14.84 kB
dist/assets/lang-php-DJHB53HJ.js                               111.33 kB │ gzip:  28.66 kB
dist/assets/model-picker-Dh0_Wt2b.js                           130.88 kB │ gzip:  43.38 kB
dist/assets/lang-mdx-C6frHttG.js                               136.10 kB │ gzip:  23.54 kB
dist/assets/vendor-2oHXUfc3.js                                 148.73 kB │ gzip:  40.40 kB
dist/assets/lang-javascript-HQTRwLWZ.js                        174.81 kB │ gzip:  16.62 kB
dist/assets/lang-tsx-DrUye_4e.js                               175.52 kB │ gzip:  16.62 kB
dist/assets/lang-jsx-B0Mso9yp.js                               177.78 kB │ gzip:  16.71 kB
dist/assets/lang-typescript-CUZHi_ve.js                        181.07 kB │ gzip:  16.24 kB
dist/assets/react-CWnP4iQq.js                                  190.94 kB │ gzip:  60.60 kB
dist/assets/lang-cpp-macro-CgwvTOCX.js                         224.24 kB │ gzip:  18.05 kB
dist/assets/lang-cpp-BAHFRj0j.js                               402.02 kB │ gzip:  31.11 kB
dist/assets/main-Bdfp6vop.js                                   441.99 kB │ gzip: 215.62 kB
dist/assets/xterm-C7jn2Fqg.js                                  465.79 kB │ gzip: 123.12 kB

✓ built in 1.97s
```

### 换行与范围核对

```text
src/server.rs LF=4620 CRLF=4620
src/dispatch.rs LF=6856 CRLF=0
tasks/TASK-08-swarm.md LF=33 CRLF=30
 M src/dispatch.rs
 M tasks/TASK-08-swarm.md
```

写报告后 `tasks/REPORT-08D.md` 另为新增文件；`server.rs` 未修改且仍为纯 CRLF，修改的 Rust 文件 `dispatch.rs` 为纯 LF。`git diff --check` 无输出。
