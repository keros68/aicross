# TASK-39 修正轮 R1（10-01）

依据：`tasks/review-39-claude.md`（Claude 子代理，C）、`tasks/review-39-agy.md`（Antigravity Gemini，G）、编排者真机核（O）。标「已核」的条目编排者已对照代码确认。范围仍按 `tasks/TASK-39-resources-config-cli.md`。不 commit，不 push。完成后在 `tasks/REPORT-39.md` 末尾追加「R1」一节：逐条写改了什么、对应测试名。

## 必修

**R1-1 带值选项写了却没给值，一律退出 2**（G 高-1、C 低-2，已核）
`unknown_option_error` 碰到认识的选项直接跳两格，不检查后面有没有值；`option_value` 取不到值就当没写。后果最重的是 `aicross resources smoke pi --model`（漏写模型名）会静默改成对该 agent 全部第三方模型批量冒烟，真实消耗额度。改为：带值选项出现但后面没有值（到末尾，或下一个参数以 `--` 开头）时退出 2，提示「缺少 <选项> 的值」。适用于 `resources smoke` 与 `config set-role` 的全部带值选项。测试：`smoke pi --model`、`smoke pi --model glm --provider`、`set-role execution --agent codex --model x --thinking` 各退出 2 且不调用冒烟、不写盘。

**R1-2 `set-role` 遇到损坏或版本不认的 `roles.json` 不得覆盖**（C 中-1、G 中-3，已核）
`roles_from` 解析失败或 `version != 1` 时静默返回默认值，`set-role` 改一个角色后整份写回，另外两个角色丢失。改为：`set-role` 严格读取——文件存在但解析失败或版本不是 1 时退出 1，stderr 提示「roles.json 无法解析，为避免覆盖已停止；请先修复或删除该文件」，不写盘。文件不存在时照常按默认值起步。`config show` 遇到同样情况在 stderr 提示一行（stdout 仍输出默认值）。参照同文件 `save_premises_in`、`save_commander_in` 遇损坏拒写的做法。测试：损坏与版本不符各一例，断言退出 1 且文件字节不变。

**R1-3 原生模型的额度用尽要显示出来**（C 中-2、G 中-2，已核）
`model_state` 先判原生就返回「可用」，额度检查走不到；而引擎派发前会拒掉额度未恢复的目标，不分原生。改为先判未过期的额度，再判原生。补测试：原生模型 + 未过期额度 → 「额度用尽」；原生 + 已过期 → 「可用」。

## 应修

| 编号 | 问题 | 来源 |
|---|---|---|
| R1-4 | agent 未安装时，它下面的模型不得显示「可用」：模型状态跟随 agent，显示「未安装」，note 与 agent 一致 | C 低-1 |
| R1-5 | `config set <key> <value>` 的值按原样取第 4 个参数，允许负数与以 `-` 开头的文本（负数按夹取规则夹到下限） | C 低-3 |
| R1-6 | 小数被取整时提示「已取整为 N」，与「越界，已夹到 N」分开 | C 低-4 |
| R1-7 | 批量冒烟时 agent 下没有任何模型，提示「该 agent 下没有模型」，退出 0 | C 低-5、G 低-4 |
| R1-8 | 「未登录」的 note 末尾补「登录后运行 aicross resources detect 刷新」 | C 低-6 |
| R1-9 | 文档补：`config set` 遇损坏文件退出 1、`set-role` 遇损坏 `roles.json` 退出 1、模型状态冲突时的优先级（未安装 → 额度用尽 → 原生可用 → 未登录 → 验证结果） | C §4 |

## 不在本轮（进积压）

- 资源库的锁只在进程内有效，窗口与命令行同时探测或冒烟会互相覆盖（C 低-7）：改文件锁，另起。
- 冒烟固定 60 秒超时会把首包慢的模型误判为「验证失败」，进而在派发前检查里被拒（O：真机对 `zai-coding-cn/glm-5.3` 冒烟 60 秒超时，而同一模型当天作实现者正常）：接入页原有逻辑，另起。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
```

R1-1、R1-2、R1-3 的新测试须能在修复临时改回时变红（编排者会抽查）。
