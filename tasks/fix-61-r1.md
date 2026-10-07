# fix-61-r1 修正清单（TASK-61，引擎）

来源：Codex（gpt-6.1-sol）只读盲审，结论「修后合并」；窗口复测（集成树）补一条。范围、验证与规矩同 TASK-61。

## 要改

1. **agy 拿不到文件材料（P1）**：agy 只看得到 stdin 里的文字。
   - 派发给 agy 时，节点需要的材料（原始材料、上游 diff、ACCEPT.md 等）全部以文本内联进 prompt，设一个总长上限（如 200 KB，常量旁注明上限与升级条件）。
   - 超过上限、有二进制或必须读项目副本 / 跑验证命令的节点（如盲验收要看改后的项目副本），派发前直接失败并说明原因，不发出。
   - 推荐：agy 只填纯审查位（盲审模式、评审模式等只读材料的验证位），不填需要项目副本或验证命令的验收位。补测试。
2. **角色默认绕过限制（P2）**：`config set-role` 把只接盲的 agent 设为执行等非盲角色时，同样拒绝（退出 2）；`config show` 对最终生效人选不合法的组合标出原因；一键推荐（含 `--all`）在找不到合规候选时，不得退回到被硬规则排除的角色默认。补测试。
3. **agy 冒烟目录（P2）**：`resources smoke agy` 用本次调用专属的空目录（建在 `$AICROSS_HOME/scratch/` 下，与派发一致），成功、失败、超时后都清理。原四家行为不变。
4. **agy 程序路径一致（P2）**：探测、冒烟、派发用同一个解析结果 / 同一优先级找 agy。
5. **模型自带档位与思考设置（P2）**：模型 id 以 `-low/-medium/-high` 结尾时，实际档位由模型决定。留痕与 run 摘要里的思考强度记实际生效值（如「由模型决定：low」），不记用户设的值；`config show` 的 effective 同样反映。补测试。
6. **未知厂商只当执行位（P2）**：出方案 / 拆分 / 顾问等规划角色也排除「未知」厂商；修正 `recommend.rs` 里断言相反行为的测试，补一条自定义模式含规划位的测试。
7. **cursor 解析失败（P2）**：`cursor-agent models` 解析失败时，该 agent 已有模型标「未在事实源中」（与其它事实源消失的处理一致），不再可选；写错误。原四家的失败处理不变。
8. **官网**：`codebuddy` 的 homepage 改为 `https://www.workbuddy.ai`（CLI 文档在该站 `/docs/cli`）。

## 不改

- 四家未实测通道继续「派发即失败」，等真机或文档核实后另起一期。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

不需要再调真机 agy（第 1 条的内联用单元测试覆盖）。改完在 `tasks/REPORT-61.md` 末尾追加「修正轮 r1」：逐条改法、新增测试名、验证结果。
