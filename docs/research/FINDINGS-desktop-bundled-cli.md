# 桌面端自带的命令行能否当派发通道（2026-11-02 本机实测）

问题：只装了桌面端、没装 CLI 的用户，aicross 能不能直接用桌面端里打包的程序派发。

| 桌面端 | 打包的程序 | 沿用桌面端登录 | 无头调用 | 点名模型 | 结论 |
|---|---|---|---|---|---|
| Codex（商店包 26.1027） | `<包目录>\app\resources\codex.exe` | 是 | 能 | 能 | 已支持（`agent_catalog::desktop_bundled`） |
| WorkBuddy 5.8.3 | `D:\workbuddy\resources\app.asar.unpacked\cli\dist\codebuddy-lite-wb.mjs`（codebuddy 2.161.0），用 `~/.workbuddy/binaries/node/.../node.exe` 运行；`cli/bin/codebuddy` 缺 `../dist/codebuddy`，直接跑会报 `MODULE_NOT_FOUND` | 否：`-p` 报 `Authentication required. Please use /login`；这一版没有交互界面，无参数启动打印一段 `fetch config failed` 后直接退出，`/login` 无处可输 | 参数齐全：`-p`、`--output-format json`、`--tools ""`、`--no-session-persistence`、`--setting-sources`、`--acp` | `--model`：auto、glm-5v-turbo、glm-5.1、glm-5.0-turbo、glm-5.0、glm-4.7、kimi-k2.5、minimax-m2.7、deepseek-v3-2-volc | 这是桌面端内部用的精简版，凭据由桌面端注入（程序里有 `CODEBUDDY_AUTH_TOKEN`、`CODEBUDDY_GATEWAY_AUTH`、`CODEBUDDY_API_KEY` 等变量）。不读桌面端凭据就用不了 |
| ZCode 0.17.0 | `C:\Program Files\ZCode\resources\glm\zcode.cjs`（`~/.local/bin/zcode.cmd`） | 是（共用 Z.AI 登录） | 半通：默认找不到内置供应商配置，要设 `ZCODE_BUILTIN_PROVIDER_CONFIG_FILE=<安装目录>\resources\config\provider\zcode-builtin.json`；之后报 `Select a model before sending a message` | 否：`-p` 没有选模型参数，`--model` 报 `Unknown option` | 暂不可用；`zcode app-server`（自有协议）未查 |
| Qoder CN 0.4.11 | 无。桌面端内部跑 worker；入口脚本 `~/.qoder-cn/entry/qodercn.cmd` 找 `qoderclicn`（`~/.qoder-cn/bin/qoderclicn/qoderclicn.exe`），本机未安装 | — | — | — | 需另装 Qoder CLI |

## 另一条路：桌面端本机网关

WorkBuddy 运行时在 127.0.0.1 开 ACP HTTP + SSE 网关，端口写在 `~/.workbuddy/sessions/*.json`；社区项目 github.com/jie023/workbuddy-acp-bridge 借它把任务交给已登录的桌面端。README 写明这不是对第三方保证稳定的接口，升级可能失效。桌面端会加载用户的记忆与连接器，盲审隔离与真身核对都做不到。

## 结论

- 只有 Codex 的桌面包能直接当通道。
- WorkBuddy 要么让用户另装官方 CodeBuddy Code CLI 并登录，要么走非公开网关；不读取、不转用桌面端的内部凭据（AGENTS.md 密钥纪律）。
- ZCode 等官方在 `-p` 里开放选模型，或核实 `app-server` 后再看。
