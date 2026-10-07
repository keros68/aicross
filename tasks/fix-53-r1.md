# fix-53-r1 斜杠命令修正清单（引擎）

来源：Claude 子代理（直接读两侧代码）与 Antigravity（Gemini 3.8 Flash，审 slash.rs）。两家都判「修改后可以合并」。

## 要改

1. **（两家共同，中 / 高）配置与文件不一致**：`slash_remove_at` 先存 modes.json 再删文件，删文件失败（Windows 文件被占用）后定义已没、文件成孤儿。改为：先删文件（只删带标记的；kimi 只删我们写的 `SKILL.md`，目录为空才删目录），成功后再存定义；删文件失败直接报错、不改配置。`slash_add_at` 先写文件再存配置，存配置失败要回滚（新建的删掉，更新的恢复原内容）。另外 remove 在没有定义但文件带标记时也能清掉文件。
2. **（Claude，中）保留名**：拒绝 `ai-cross`（与 skill 本身冲突：claude 的 `/ai-cross`、kimi 的 `skills/ai-cross`）。kimi 写入前，若 `~/.agents/skills/<name>` 或 `~/.kimi-code/skills/<name>` 已存在且不是我们写的，视为被占用。同时拒绝 Windows 保留名（`con`、`nul`、`aux`、`prn`、`com1`–`com9`、`lpt1`–`lpt9`）。
3. **（Claude，中）frontmatter 未转义**：`description`（及 kimi 的 `name`、正文里的模式名）写成双引号字符串并做 JSON 风格转义；模式名、note 里的换行与控制字符替换为空格。
4. **（两家，低）标记判定**：只认首行 `---` 开始、找到闭合 `---` 的 frontmatter 内、不缩进的 `aicross-managed: true`（允许行尾 `# 注释`）；开头的 BOM 先去掉再判；未闭合的 frontmatter 不认。
5. **（两家，低）名称与 host 校验**：`slash_remove_at`、`list` 对名称再跑 `validate_name`（list 对不合法的名称跳过并在输出里标 `state: "名称无效"`）；三个入口校验 host 只能是 claude / codex / pi / kimi，去掉 `_` 通配落到 kimi 的写法。
6. **（Claude，低）`--note` 以 `-` 开头被当缺值**：接受 `--note=<文本>` 写法，文档写明。
7. **测试（两家）**：每家按完整文件内容精确比对（含 kimi 的 `name:`、仅 claude / codex 有 `argument-hint`）；kimi 目录里放一个别的文件，remove 后它和目录都还在；`has_managed_marker` 单元测试（标记在 frontmatter 外、未闭合、缩进、BOM、CRLF、行尾注释）；remove 删文件失败不改配置、add 存配置失败回滚（失败可注入）；保留名与 Windows 保留名；非法 host。

## 不改

- Gemini「正文少了『模式』二字」：有意改为「按「<模式名>」处理」，避免读成「盲审模式模式」；任务书措辞以代码为准（编排者已同步）。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前；文档同步。改完在 `tasks/REPORT-53.md` 末尾追加「修正轮 r1」。测试一律用临时目录，绝不写真实家目录。不 commit，不 push。
