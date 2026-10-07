# TASK-50 主 agent、skill 安装、一键推荐（引擎侧，10-06）

**只改 `src/**`、`Cargo.toml`（只加下文允许的依赖）、`capabilities/**`、`docs/CONSOLE-CHANNEL.md`、`scripts/`（新增同步脚本）、`skill-bundle/`（新增），以及新建 `tasks/REPORT-50.md`。不改前端。不 commit，不 push。** 在 TASK-46 / 48 的未提交改动之上继续（同一工作树）。

设计依据：`docs/PRODUCT.md`「主 agent」「skill 安装」「一键推荐」三节（先完整读）。界面是 TASK-51，按本文第 5 节的窗口命令契约并行开发。

## 1. 主 agent

- 可作主 agent 的 agent：`claude`、`codex`、`kimi`、`pi`（资源库里 `detected` 为 true 的才列出）。
- **实际厂商推定**（只读配置，不读密钥）：
  - claude：`~/.claude/settings.json` 的 `env.ANTHROPIC_BASE_URL` 为空 → Anthropic；非空 → 按 URL 主机名匹配已知厂商（如 `bigmodel.cn` → 智谱，`moonshot` → 月之暗面，`deepseek` → DeepSeek，`dashscope`/`aliyun` → 阿里），匹配不上为「未知」。进程环境变量 `ANTHROPIC_BASE_URL` 同理（与 `inventory.py` 的 `host_vendor_of` 口径一致）。
  - codex：`~/.codex/config.toml` 没有 `model_provider` 或为 `openai` → OpenAI；其它 provider 按名称匹配，匹配不上「未知」。
  - kimi：`~/.kimi-code/config.toml` 的 `provider` 以 `managed:kimi-code` 开头 → 月之暗面；其它按名称匹配。
  - pi：`~/.pi/agent/settings.json` 的 `defaultProvider` + `defaultModel`，在资源库里找到这条模型取其 `vendor`；找不到或该模型「未发现」→「未知」，依据里写明（本机实情：默认 provider `qwen-token-plan-cn` 已不在 pi 里）。
  - 每个结果带 `vendorSource`：`推定` / `端点` / `用户设定` / `未知`，以及一行 `vendorBasis`（如「pi 默认模型 qwen-token-plan-cn/qwen3.8-max 未发现」）。
- 用户可覆盖：`modes.json` 每个主 agent 的 `vendorOverride`（空为不覆盖）。

## 2. 配置按主 agent 分开存

`modes.json` 改为：

```json
{ "version": 2,
  "hosts": {
    "claude": { "vendorOverride": "", "defaultMode": "builtin-blind",
                "seats": { "builtin-blind": [[ { …人选…, "origin": "manual" }, null, … ]] } }
  } }
```

- 人选对象在 TASK-46 基础上追加 `origin`：`manual`（窗口或命令行手选）/ `recommended`（一键推荐填的）；缺省视为 `manual`。
- `version` 1 的文件不做迁移（尚未发布过）：宽松读取当作空配置，严格读取报「modes.json 是旧格式，删除后重新设置」并退出 1、不覆盖。
- 补人规则（TASK-46 第 2 节）不变，只是从对应主 agent 的那份里取。
- `--host` 规则（所有读写配置的命令与派发）：给了就用；没给时，`modes.json` 里只有一个主 agent 的配置就用它，一个都没有则按无配置处理（派发走原有 `roles.json` 行为），多于一个则退出 2「需要 --host，可选：…」。`--host` 的值必须是第 1 节里检测到的 agent，否则退出 2。
- 派发记录（run 文件）追加 `host` 字段。

## 3. 一键推荐

- `aicross config recommend --host <h> [--mode <模式>] [--all]`：
  - 不带 `--all`：只填空位置与 `origin` 为 `recommended` 的位置，`manual` 不动；带 `--all`：全部重填（窗口里需用户确认后才传）。
  - 不带 `--mode`：对该主 agent 的全部模式做；带了只做这一个。
  - 输出 `{ defaultMode, modes, notes }`（与 `config show` 同结构的 `modes`，加 `notes` 说明凑不出的位置）。
- **候选池**：资源状态为 `可用`（原生）或 `已验证` 的模型；`未验证`、`未发现`、`验证失败`、`未登录`、`额度用尽` 都不进。
- **档位表**：新增 `src/data/model-tiers.json`（随版本发布的小表），按 `agent/provider/model` 前缀或模型名关键字给 `高` / `中` / `低`，以及厂商的计费方式（订阅 / 按次 / 未知，如 DeepSeek 官方 API 为按次）。表里没有的模型档位为「未知」，排在已知档位之后。
- **规则**（复用 `inventory.py` 选审查者的思路）：
  - 出方案、拆分：优先高档；同档内订阅优先、按次排最后。
  - 执行：优先中、低档；订阅优先。
  - 验收、验证：厂商必须不同于主 agent 的实际厂商（主 agent 厂商「未知」时不设这条限制，并在 notes 里说明「主 agent 厂商未知，不能保证跨厂商」）；同一阶段的验收 / 验证位置之间厂商互不相同；尽量也不同于该模式里执行位置的厂商；优先已验证的第三方与订阅。
  - 凑不出满足条件的人选时该位置留空，notes 写明原因，不降级硬填。
  - 结果确定（同输入同输出），候选同分时按资源库顺序。
- 推荐过的人选变成未发现等状态后，下次推荐会替换它（因为它是 `recommended`）；`manual` 的只在 `effective.state` 里体现失效，不替换。

## 4. skill 安装

- **内置包**：新增 `skill-bundle/ai-cross/`，内容是 skill 源仓（`D:\work\skill\xiaoyu-skill\skills\ai-cross`）去掉 `tests/`、`.git*` 之后的副本；新增 `scripts/sync-skill.py`（参数为源目录，默认上面这个路径）负责同步。用 `include_dir` crate 把它编进可执行文件（允许新增这一个依赖；在报告里写明增加的体积）。内置版本号取 `SKILL.md` frontmatter 的 `metadata.version`。
- **位置表**（按 `docs/PRODUCT.md` 已核实的表）：每个主 agent 会读取的位置——
  - claude：全局 `~/.claude/skills`；项目 `.claude/skills`
  - codex：全局 `~/.agents/skills`，另扫描旧位置 `~/.codex/skills`；项目 `.agents/skills`
  - kimi：全局 `~/.kimi-code/skills`、`~/.agents/skills`；项目 `.kimi-code/skills`、`.agents/skills`
  - pi：全局 `~/.pi/agent/skills`、`~/.agents/skills`；项目 `.pi/skills`、`.agents/skills`
  - **我们安装时写入的位置**：claude 写 `.claude/skills/ai-cross`；codex、kimi、pi 写 `.agents/skills/ai-cross`（全局在家目录下，项目在项目目录下）。
- **判定每个位置**：
  - `none`：不存在。
  - `ours`：普通目录且含 `.aicross-install.json`（`{ version, installedAt, source }`）。
  - `external`：是链接（symlink / junction，记下指向的目标）或没有我们的标记文件。
  - 都读出 `SKILL.md` 的版本号（读不出为空）。
- `aicross skill status [--project <dir>]`：输出 `{ bundledVersion, hosts: [{ host, locations: [{ scope, path, kind, version, linkTarget, readByThisHost: true }], duplicate: bool, updateAvailable: bool }] }`。同一主 agent 能读到两处以上 ai-cross 时 `duplicate` 为 true。
- `aicross skill install --host <h> --scope global|project [--project <dir>]`：写入「我们安装时写入的位置」。目标为 `external` 时退出 1「由其他软件管理：<路径>」，不覆盖；为 `ours` 时先删后写（即更新）；为 `none` 时新建。写完输出该 host 的 status 项。项目范围必须给 `--project`，目录不存在退出 2。
- `aicross skill uninstall --host <h> --scope global|project [--project <dir>]`：只删 `ours`；`external` 退出 1 不动。
- 家目录与项目目录都要能在测试里注入（测试一律用临时目录，**绝不写真实家目录**）。

## 5. 窗口命令（server.rs，全部 async + spawn_blocking，返回与命令行 stdout 同结构）

| 命令 | 对应 |
|---|---|
| `hosts_list()` | `aicross hosts`：`[{ id, name, vendor, vendorSource, vendorBasis }]` |
| `host_set_vendor(host, vendor)` | `aicross config set-host-vendor <host> <vendor|auto>` |
| `config_snapshot(host)` | `{ resources, config }`，`config` 为该主 agent 的 `config show` |
| `config_set_default_mode(host, mode)` / `config_set_seat(host, mode, stage, index, seat)` / `config_clear_seat(host, mode, stage?, index?)` | 在原命令上加 `host` |
| `config_recommend(host, mode?, all)` | `aicross config recommend` |
| `skill_status(project?)` / `skill_install(host, scope, project?)` / `skill_uninstall(host, scope, project?)` | `aicross skill …` |
| `pick_folder()` | 弹出系统选目录对话框，返回路径或 null（允许新增 `tauri-plugin-dialog`，只在 Rust 侧调用，前端不装 npm 包；capabilities 按需放行） |

`config_set_seat` 写入的人选 `origin` 一律为 `manual`。

## 6. 测试

- 厂商推定：四家各自的正常、第三方端点、未知情形（配置文件内容用临时目录注入）；`vendorOverride` 覆盖。
- `modes.json` v2 读写；v1 文件宽松读为空、严格读退出 1 不覆盖；`--host` 三种缺省情形与非法 host。
- 推荐：只填空位与 recommended、不动 manual；`--all` 全填；验收 / 验证避开主 agent 厂商且同阶段互不相同；主 agent 厂商未知时不加限制并写 notes；候选不足留空写 notes；按次计费排后；结果确定。
- skill：none / ours / external（链接与无标记两种）判定；install 新建、更新、遇 external 拒绝；uninstall 只删 ours；duplicate 判定（如 codex 同时有 `~/.codex/skills/ai-cross` 与 `~/.agents/skills/ai-cross`）；内置版本号读取。
- 窗口命令与命令行输出一致（至少 `config_recommend`、`skill_status`）。

## 7. 文档

`docs/CONSOLE-CHANNEL.md`：`--host` 规则、`aicross hosts`、`config set-host-vendor`、`config recommend`、`aicross skill status / install / uninstall`、`modes.json` v2 格式与 `origin`、run 文件 `host` 字段。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行：`src/server.rs`、`src/main.rs` CRLF，其余 LF；新文件 LF。写 `tasks/REPORT-50.md`：改动与新增文件、新增依赖与体积影响、测试名、验证结果、未做与理由。

## 编排者真机核（实现者不做）

`aicross hosts` 看四家的厂商推定（本机 pi 应为未知）；`skill status` 应识别出 skillmanager 的 junction 为 external；`skill install --scope project` 装进一个临时项目目录再 uninstall；`config recommend --host claude` 看推荐结果与 notes。**不在真实家目录做 install。**
