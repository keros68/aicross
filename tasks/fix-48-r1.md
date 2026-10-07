# fix-48-r1 修正清单

来源：Claude 子代理与 Antigravity（Gemini 3.8 Flash）两份盲审。编排者已核对代码的条目注明「已核实」。

## 要改

1. **主目标查找不看 provider（两家共同，中，已核实）**：`plan_dispatch` 里「未发现」检查用 `strict`（只按 agent + id 取第一条）。本机实情：`pi/deepseek-v4-flash` 同时挂在已撤掉的 `deepseek` 与正常的 `opencode-go` 下，选 `opencode-go` 那条会被误拒；顺序反过来则漏拦。改法：新增一个共用查找函数（如 `find_target_model(models, agent, provider, id)`）：provider 非空时只按 agent + provider + id 精确匹配；provider 为空时按 agent + id 取第一条。「未发现」的主目标检查、备选检查、`console_cli` 的 `resource_state_of` 都改用它，删掉各处手写的两级查找。`resource_state_of` 在给了 provider 却没精确匹配时返回 `不在资源库`（Gemini 第 3 条）。「验证失败」「额度用尽」两处原有检查不在本期范围，不动。补测试：同一 id 挂两个 provider、只有一个带标记，选正常那条能派出、选带标记那条被拒；`effective.state` 同口径。
2. **只检查第一阶段（Claude 独有，中，已核实）**：「未发现」的主目标拒绝与丢备选只遍历 `first`。盲审 / 经济模式后面阶段的人选若未发现，派发会被接受，跑到那一阶段才失败。改法：这两项遍历本次 `created` 里所有带 target 的节点。补测试：第二阶段人选未发现 → 派发前即被拒；第二阶段备选未发现 → 被丢弃并有说明。
3. **全部跳过时冒烟输出空资源库（Gemini 独有，高，已核实）**：`smoke_agent_models_filtered_at` 的 `latest` 初始化为 `ResourceStore::default()`，可冒烟的模型为空时直接返回空库，命令行与窗口输出丢失该 agent。改法：`latest` 用第一次加锁读到的 store 初始化。补测试：某 agent 下第三方模型全部未发现 → 输出仍含该 agent 的完整条目与跳过说明。
4. **生产入口写 `detectedAt` 没有测试（Claude 独有，中）**：测试走的是 `#[cfg(test)]` 的函数，删掉 `detect_all_resources_at` 里那一行测试仍通过。改法：把盖时间戳抽成一个函数，两处都调用；在 console_cli 用可注入的 `detect_all` 走真实写回逻辑，断言输出的 `detected_at` 非 null。
5. **按 agent 冒烟的过滤与跳过说明没有测到生产代码（两家，低）**：两个测试各自抄了闭包或整段替换。改法：把过滤与跳过说明提成具名函数，生产与测试共用；补一条经 `smoke_agent_third_party_at`（冒烟函数可注入）的测试。
6. **丢掉备选写回了图（Claude 独有，低）**：每次派发一张图，影响只在本次 run 的重试里。改法：说明文字改为「备选 <agent>/<model> 在最近一次探测中未发现，已从该节点移除」，文档同步。
7. **判法不一致（Claude 独有，低）**：命令行按状态判（agent 未安装时状态为「未安装」，不算未发现），引擎直接读标记。改法：命令行的选人选与冒烟也直接读标记（`is_missing_from_detection`），与引擎一致；文档写明「未发现」按最近一次探测的标记判断。
8. **文档 `notes` 描述不一致（Gemini 独有，低）**：响应里 `notes` 总是给出（空时 `[]`），run 文件里为空时不写出。按实现改文档。

## 不改

无。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前。改完在 `tasks/REPORT-48.md` 末尾追加「修正轮 r1」一节：逐条改法、新增测试名、验证结果。不 commit，不 push。
