未运行构建、测试或真实 CLI，也未联网；以下是静态审查结论。已检查指定路径的改动和相关调用链，未逐行审阅全部未改动代码及所有历史报告。`skill_install.rs`、`slash.rs` 不属于 B 块，本次不能对它们覆盖外部文件的安全性作结论。未修改任何文件。

未发现 P0。问题按严重程度排序：

1. **P1 — 资源库跨进程写入会丢更新。**  
   位置：[src/resources.rs:656](/D:/work/aicross/src/resources.rs:656)、[src/resources.rs:2885](/D:/work/aicross/src/resources.rs:2885)。  
   `store_lock` 是进程内互斥锁，窗口、命令行和后台 `__run` 各持一份。单模型冒烟先读取整库，最长等待 180 秒，再整库写回。  
   **触发条件：**窗口冒烟期间，后台派发写入另一模型的额度标记或验证结果；冒烟结束会用旧库覆盖这些更新。探测、清理也存在跨进程读改写竞争。这是基线已有的锁缺口，本批窗口入口继续沿用。  
   **建议：**所有资源写入共用跨进程锁；冒烟在锁外运行，写回时重新读取最新库，只合并目标结果。现有并发测试只覆盖同进程线程，应补双进程写入测试。

2. **P1 — 已验证失败的模型仍能经后续阶段或备选实际派出。**  
   位置：[src/engine/mod.rs:2505](/D:/work/aicross/src/engine/mod.rs:2505)、[src/resources.rs:2314](/D:/work/aicross/src/resources.rs:2314)。  
   引擎只检查第一阶段的 `failed`；共同验证入口把已经失败的模型返回为 `Skipped`，派发层随后继续执行真实请求。  
   **触发条件：**经济模式第二阶段配置了验证失败的审查模型；或者只读节点失败后切到一个已经验证失败的备选。两条路径均绕过“验证失败直接拒绝”的产品规则。此缺口在基线中已存在。  
   **建议：**共同派发入口明确拒绝 `failed`，并在派发前检查全部新建目标及备选；保留 `timeout` 放行规则。补后续阶段及备选失败模型“不进入通道”的测试。

3. **P1 — TASK-48 的“未发现”拦截漏掉资源库中的有序备选。**  
   位置：[src/dispatch.rs:2498](/D:/work/aicross/src/dispatch.rs:2498)、[src/dispatch.rs:2273](/D:/work/aicross/src/dispatch.rs:2273)。  
   引擎会移除 `Target.fallback` 中未发现的备选，但 `ResourceModel.fallback_order` 解析出的候选只检查登录和额度，没有检查“未发现”。  
   **触发条件：**主模型额度用尽，资源库备选指向一个带 `MISSING_MARKER`、此前已验证的模型；它会跳过首次验证并实际派出。  
   **建议：**在共同尝试入口检查缺失标记，同时过滤资源库备选并记录跳过原因。补“主模型额度用尽＋资源库备选未发现”的测试。

4. **P2 — 验证失败检查不匹配 provider，可能误拒或漏拦。**  
   位置：[src/engine/mod.rs:2465](/D:/work/aicross/src/engine/mod.rs:2465)。  
   `strict` 只按 `agent + model` 取第一条，而配置器可以明确选择 provider。  
   **触发条件：**同一模型挂在两个 pi provider 下，一条失败、一条已验证。选择正常 provider 可能被误拒；顺序反过来，选择失败 provider 又可能被放行。`fix-48-r1` 明确留下了这段基线问题。  
   **建议：**验证失败检查改用 provider 精确匹配的共同查找函数；补两个 provider、两种排列顺序的测试。

5. **P2 — “如实汇报”只保证请求对象带后缀，没有保证实际外发带后缀。**  
   位置：[src/engine/mod.rs:1505](/D:/work/aicross/src/engine/mod.rs:1505)、[src/dispatch.rs:2713](/D:/work/aicross/src/dispatch.rs:2713)。  
   **触发条件：**共享节点续接会话且 `current_message` 非空时，实际发送的是当前消息，替换掉含后缀的完整 prompt；盲材料追加和 Antigravity 内联也会把材料放到后缀之后。与 `fix-57-r1` 的“每个节点提示词末尾固定追加”不符。  
   **建议：**在确定最终外发文本后统一追加后缀，覆盖续接、ACP 回退和材料内联。现有测试只断言 `node.prompt`，应检查最终 stdin／ACP 消息。

6. **P2 — Codex 模型全部被隐藏时，旧隐藏模型仍保持可选。**  
   位置：[src/resources.rs:1024](/D:/work/aicross/src/resources.rs:1024)、[src/resources.rs:1069](/D:/work/aicross/src/resources.rs:1069)。  
   **触发条件：**有效缓存中的模型全部变成 `visibility: "hide"`。过滤后空列表被当作解析失败；应用层保留旧模型，不合并空清单，因此旧条目没有“未发现”标记，仍可推荐、配置和派发。  
   **建议：**区分缓存解析失败与成功过滤后为空；后者应合并空清单。现有隐藏模型测试混有可见模型，需补“全部隐藏”的状态迁移测试。

另外，**Kimi 一次性通道仍把多行 prompt 放进 argv**：[src/channels/mod.rs:392](/D:/work/aicross/src/channels/mod.rs:392)。这是基线遗留，违反当前仓库的传参约束；整段提示词会进入子进程命令行。未实测当前 Kimi 版本是否发生换行截断，不能认定已发生。应核实并改用 stdin、文件或 ACP。

正常输入下的探测并行、pi 事实源优先顺序、Antigravity 的 NDJSON 输入、空回答失败判定和非盲位置拦截，未发现额外问题。

**结论：修后发布。**
