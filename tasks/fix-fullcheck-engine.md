# fix-fullcheck-engine（10-08 全面检查，引擎）

来源：10-08 全面检查。Codex（gpt-6.1-sol）分 A / B 两块只读审查，结论都是「修后发布」；另有窗口与真机派发走查的一条。审查原文：`tasks/fullcheck-A.md`、`tasks/fullcheck-B.md`。

**只改 `src/**`、`fixtures/**`、`docs/CONSOLE-CHANNEL.md`，新建 `tasks/REPORT-fullcheck-engine.md`。不改 `src-ui/**`。不 commit，不 push，不加依赖。**

**每条先核实再改**：先读代码、能写失败测试的先写出失败测试，确认问题成立再修；核实后不成立的，在报告里写明证据（代码位置与理由），不改。修 bug 要找共同路径修根因，查清被改函数的所有调用方。

## 要改

### skill 安装与斜杠命令（A 块）

1. **（A1，P1）查询状态会删安装备份**：`skill status` / 窗口 `skill_status` 只读，不做任何删除。残留的临时目录与备份只在安装 / 更新 / 卸载时、确认属于已结束的本次操作后清理；目标缺失而备份在时，下一次安装先恢复备份。安装 / 更新 / 卸载加跨进程互斥（不加依赖：用 `create_new` 锁文件 + 超时与过期锁处理，常量旁注明上限）。补中断恢复与「查询不删」测试。
2. **（A2，P1）斜杠命令沿链接写删外部文件**：写入与删除共用一个归属检查，识别命令目录与文件本身是 symlink / junction（含 Kimi 的 `~/.kimi-code/skills/<名>/` 目录）；链接一律视为外部、拒绝修改；元数据读不到（除确实不存在）按占用处理。补 junction / symlink / 悬空链接测试（建不了链接的平台跳过并注明）。
3. **（A4，P2）预设在锁外读取**：`set-seat`、`clear-seat`、`set-default-mode`、一键推荐等写 modes.json 的路径，在同一把锁内重新读取预设与 modes 再校验和写入。
4. **（A5，P2）推荐不采用的非法角色默认仍会生效**：`fill_seats` 等共同补人路径在用角色默认补位时，也按位置硬规则（`seat_violation` 与「只看得到文字的 agent 不填验收位」等）校验；不合规的不补，`config show` 显示未指定并给原因。补测试：保存后的 `config show` 与派发计划里实际人选。
5. **（A6，P2）跨卷暂存**：skill 安装的临时目录与备份建在目标实际所在目录（父目录被 junction 重定向时按解析后的真实路径）的同卷位置，且不被宿主当成 skill 扫描（如隐藏前缀）。补测试（能建 junction 时）。

### 资源与派发（B 块）

6. **（B1，P1）资源库跨进程写入丢更新**：资源库所有写入（探测、冒烟、清理、额度标记、验证结果）走跨进程锁（同第 1 条的锁文件做法）；冒烟在锁外运行，写回时重读最新库、只合并目标模型的结果。补双进程或模拟交错写入的测试。
7. **（B2，P1）验证失败的模型仍能经后续阶段或备选派出**：在共同派发入口拒绝 `failed`，覆盖后续阶段与备选；`timeout` 照旧放行。补测试。
8. **（B3，P1）资源库 `fallback_order` 备选没查「未发现」**：共同入口检查缺失标记，过滤资源库备选并记录跳过原因。补测试。
9. **（B4，P2）验证失败检查不匹配 provider**：改用 provider 精确匹配的共同查找。补两 provider、两种顺序的测试。
10. **（B5，P2）如实汇报后缀没保证在最终外发文本里**：在确定最终外发文本（续接时的 `current_message`、ACP、材料内联之后）统一追加，只出现一次；agy 的「不要使用任何工具」句同理。测试断言最终 stdin / ACP 消息。
11. **（B6，P2）Codex 模型全部隐藏时旧条目仍可选**：区分「缓存解析失败」与「过滤后为空」；后者合并空清单，旧条目标未发现。补测试。

### 真机走查

12. **盲审 / 评审模式单独派发时用错了模板**：没有上游回答（第一阶段就是审查位）时，审查者收到的是「审查别人的产物」模板与固定关注点（契约是否变、回退修复后测试是否变红），真机三家都花篇幅说明「没有被审产物」。改为：没有上游产物的审查位用「独立审查这份原始材料」的模板，固定关注点只在有上游产物时附上；评审模式的视角句照常。补测试，golden 若受影响按新行为更新并在报告写明。

## 不改

- Kimi 一次性通道 prompt 走 argv（B 块附注）：kimi 没有 stdin 入口，已知限制，已有 30KB 截断；不改。
- skill 尚未读 aicross 配置（A3）：另起一期在 skill 源仓做，本轮不动内置包。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行：`src/server.rs`、`src/main.rs` 是 CRLF，其它 Rust 文件 LF；源码改动用编辑工具，不用 shell heredoc。测试一律用临时目录，绝不写真实家目录、真实 `~/.claude`、`~/.agents` 或 `D:\aicross-data`。报告写：每条「成立 / 不成立（证据）」、改法、新增测试名、验证结果；没做完或失败的放最前。
