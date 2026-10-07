# TASK-59 评审视角可选、短命令只由用户调用、外部安装来源（引擎）

接在 TASK-57 之后，同一工作树。界面侧为 TASK-60，契约以本文为准。

## 1. 评审视角可选

评审模式（`builtin-jury`）每个验证位置的视角可以按主 agent 改。

- `modes.json` 的 host 段增加 `focus: { "<presetId>": ["安全", "性能", ...] }`，目前只接受 `builtin-jury`，其它模式退出 2。缺省或某个下标缺失时按默认表「安全 → 性能 → 需求覆盖」依次取。
- 内置视角与派发文本（一张表，`engine::preset_node_texts` 与 `config show` 的 `focus` 都从这里取，不抄两份）：
  - 安全：现有文本
  - 性能：现有文本
  - 需求覆盖：现有文本
  - 规范：「规范评审：只从项目规范视角审查这批材料：对照仓库已有的编码规范与约定（如 AGENTS.md、CONTRIBUTING.md、lint 配置），指出违反之处与修改建议。」
  - 正确性：「正确性评审：只从正确性视角审查这批材料，指出逻辑错误、遗漏的边界条件与反例。」
  - 可维护性：「可维护性评审：只从可维护性视角审查这批材料，指出重复、命名、耦合与可以简化之处。」
  - 其它自定义文字 X：「X评审：只从X视角审查这批材料，指出问题与改进建议。」
- 自定义文字校验：去首尾空白后 1–20 字，拒绝换行、制表与其它控制字符、ANSI 转义（退出 2）。
- 命令行：`aicross config set-focus --host <h> --mode builtin-jury --index <n> --focus <文字>`；`n` 从 1 起，是该模式里带视角的位置按阶段先后的序号。`--focus ""` 恢复该位置的默认。下标超出当前数量退出 2。在 `modes_write_lock` 内读改写。窗口命令 `config_set_focus(host, mode, index, focus)`，返回同 `config show` 里该模式的完整条目（id、name、count、stages 等）。
- `config show`：位置的 `focus` 取实际生效值；评审模式节点增加 `focusOptions: ["安全","性能","需求覆盖","规范","正确性","可维护性"]`。
- 派发：视角经与数量（counts）同一条路径到达派发（`shaped_preset` 带 host 段），派发出去的节点文本用上表。补测试：改成「规范」后 `plan_dispatch` / 盖章的节点文本是规范文本；自定义文字套模板；恢复默认；数量从 3 调到 2 后多余的 focus 不影响。
- 数量调小时不删 `focus` 里多出的项（再调大时恢复）。

## 2. 短命令只由用户调用

- claude（`~/.claude/commands/<名>.md`）与 kimi（`~/.kimi-code/skills/<名>/SKILL.md`）生成的 frontmatter 增加 `disable-model-invocation: true`，模型不会自己触发。codex prompts 与 pi prompts 本来只能由用户输入，不加。
- `slash list` 每条已登记命令增加 `stale`：文件带我们的标记、但内容与现在会生成的内容不同（如旧版没有这一行）时为 true。`slash add` 同名即重写，界面据此给「更新」。补测试。

## 3. 外部安装的来源

`skill status` 每个外部（external）位置增加 `manager`：
- `"skills.sh"`：全局位置在 skills.sh 全局锁文件里登记了 `ai-cross`（`$XDG_STATE_HOME/skills/.skill-lock.json`，未设时 `~/.agents/.skill-lock.json`，读 `skills` 对象的键）；项目位置在项目根的 `skills-lock.json` 里登记了 `ai-cross`。
- `"skills-manager"`：位置是链接且目标在 `~/.skills-manager/` 下。
- 其余为空串。
锁文件读不了或格式不对时当作未登记，不报错。家目录与项目目录都由调用方注入，测试用临时目录造锁文件与链接（Windows 用 junction，建不了时跳过该用例并注明）。

## 范围与验证

只改 `src/**`、`docs/CONSOLE-CHANNEL.md`、`tasks/REPORT-59.md`。不改 `src-ui/**`。不加依赖，不 commit，不 push，不联网。`src/server.rs`、`src/main.rs` 是 CRLF，其它 Rust 文件 LF。测试一律用临时目录，绝不写真实家目录或真实数据目录。

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

完成后写 `tasks/REPORT-59.md`：逐条改法、输出字段变化、新增测试名、验证结果摘要。
