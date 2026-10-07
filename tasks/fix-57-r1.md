# fix-57-r1 修正清单（TASK-57，引擎）

来源：Claude 子代理盲审（读快照代码），结论「可以合并」，9 条均为低。

## 要改

1. **探测线程 panic**（resources.rs，并行探测的 join 处）：某个 agent 的探测线程 panic 时，不再整次作废；转成该 agent 的一条错误「探测 <agent> 失败：线程异常」，其它 agent 照常合并、盖时间、写盘。补测试（注入会 panic 的假探测）。
2. **进度事件顺序**：计数加一与 `on_done` 回调放进同一把锁，保证事件里的 `done` 严格递增。测试改为不排序直接断言 1..=total。
3. **kimi 的 `existing`**（slash.rs）：与 `slash add` 用同一套占用判定——`~/.agents/skills/<名>` 已存在，或 `~/.kimi-code/skills/<名>/` 目录存在但没有带标记的 SKILL.md，都给 `{ state: "被占用" }`。抽出共用函数，不抄两份。补测试。
4. **项目路径斜杠**（skill_install.rs）：Windows 上对传入的项目根路径先统一成反斜杠再拼接；`linkTarget` 输出同样统一。补测试（`D:/x` 形式的输入）。
5. **`unverified` 输出测试**：在 `recommend_at_home` 现有测试里断言输出的 `unverified` 结构，含空时为 `[]`。
6. 删掉 console_cli.rs 里无作用的 `.map_err(|message| message)`。
8. **如实汇报（新增）**：引擎派出去的每个节点提示词末尾固定追加一段（所有角色、所有模式、ACP 与一次性 CLI 两条通道都走同一处）：
   `Be honest in your response. 没做完、没运行、失败或与预期不符的结果要写明，放在回答开头。`
   常量只定义一处，旁边注明出处（Language Models Are "Insecure" Reporters，2026-09：英文这句让负面结果披露从 2/200 升到 190/200；中文句是补充，未单独实测）。不做开关。补测试：盖章 / 派发构造出的提示词以这段结尾，用户原文不被改动；golden 测试相应更新。
7. recommend.rs：同阶段已选清单合并了执行与规划角色，在旁边注明这是有意合并及其影响（只影响同一阶段混有规划位和执行位的自定义模式）。

## 不改

- ACP 缓存在命令行装完适配器后，窗口最多 60 秒才更新：在 REPORT 里注明这个上限即可。
- 缓存过期不补测试。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前。测试一律用临时目录。不 commit，不 push。改完在 `tasks/REPORT-57.md` 末尾追加「修正轮 r1」。
