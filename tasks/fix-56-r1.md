# fix-56-r1 修正清单（TASK-54 到 56，引擎）

来源：Claude 子代理（读三期代码）与 Antigravity（Gemini 3.8 Flash，审 preset.rs 与数量可调部分）。两家均「修改后可以合并」。

## 要改

1. **（两家共同，中 / 高）`preset remove` 级联顺序**：先在锁内逐个删带标记的短命令文件，全部成功后再写 modes.json（seats、默认模式、slash 定义），最后删预设。任一步失败停止，错误里列出已完成与剩下的条目（host / name）。用现成的 `FAIL_REMOVE_FILE` 注入补中途失败用例。
2. **（Claude，中）级联前校验**：对 modes.json 里每条短命令定义先过 `validate_host` 与 `validate_name`；不合法的只从定义里移除，不碰磁盘，在结果里标出。不能触发 `unreachable!`。
3. **（Claude，中）一键生成撞名**：`slash_add_all_at` 生成前查 `<前缀><短名>` 是否已被别的模式的定义使用；是则记入 `skipped`，原因「名称已被 <模式名> 使用」，不改写。补测试。
4. **（Claude，中）更新自定义模式后人选错位**：`preset save` 更新已有预设时，若阶段数、每阶段位置数或任一位置的角色变了，清掉各主 agent 下该模式的 seats（slashCommands 与默认模式保留），输出里列出受影响的主 agent。
5. **（Gemini，低）更新时 id 不存在**：显式给了 `id` 但不存在于用户预设时退出 2「模式不存在：<id>」，不静默新建。
6. **（Claude，低）id 与别名冲突**：拒绝 `shared`、`economy`、`blind` 及其它 `--preset` 别名作为用户预设 id。
7. **（Gemini，中）介绍校验**：`description` ≤200 字；`name` 与 `description` 里的换行、制表以外的控制字符与 ANSI 转义拒绝（退出 2）。
8. **（Claude，低）set-seat / clear-seat 的形状在锁外取**：在 `modes_write_lock` 内用严格读到的 modes 重新取 `shaped_preset` 并重新校验位置范围。
9. **（Gemini，低）可调位置的模板与顺序**：模板固定从 `builtin_presets()` 的原始预设取；增减改为「多则从末尾去掉、少则在该角色最后一个节点之后追加」，不打乱阶段内顺序。
10. **测试（两家）**：
    - 自定义预设真正走一次 `start_dispatch_with` / `plan_dispatch` 并检查节点 target。
    - 裁剪测试各位置填不同模型；盲审 4 → 1 时验收人选保留；经济 1 → 0 → 2 恢复。
    - 数量调整后的位置文本序列断言（评审按「安全 → 性能 → 需求覆盖」）。
    - 上面 1–7 各自的用例。

## 不改

- Gemini「数量调到 0 后再调大模板丢失」：现有调用每次都从原始内置预设计算形状，实际不会发生；第 9 条改为固定从原始预设取模板，作为加固。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前；文档同步。改完在 `tasks/REPORT-56.md` 末尾追加「修正轮 r1」。测试一律用临时目录，绝不写真实家目录或真实数据目录。不 commit，不 push。
