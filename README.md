# aicross

多 agent 协作的入门面板。平时用 Claude Code、Codex、Kimi Code 还是 pi agent
都可以：打开面板点两下，本机已有的各家模型就分好了谁出方案、谁执行、谁审查，
回到原来的软件里说一句话就开始协作。

本仓库同时包含面板（`src/`、`src-ui/`）与 ai-cross skill 源码
（[`skills/ai-cross`](skills/ai-cross/)）。skill 不装面板也能用；装了面板后，
派给谁在面板里设，不用在对话里另做盘点。

## 亮点

- **新人快速上手**：「开始」页三步完成：探测本机装了哪些 agent 和模型，选平时用的
  软件后「一键配好」（装 skill、按推荐分好人选），复制示例回去说一句。不用写配置
  文件，不用记命令；之后改人选也在面板里点选。
- **不挑主 agent**：Claude Code、Codex、Kimi Code、pi agent 都能当主 agent，换着用时
  各自一份配置，互不影响。
- **多种协作方式**：六种模式可选，也能自建：

  | 模式 | 做法 |
  |---|---|
  | 共享模式 | 单个模型执行，可读取完整对话上下文；只读 |
  | 经济模式 | 高档模型出方案，低档模型执行，其他厂商的模型验收 |
  | 盲审模式 | 同一份材料由多个厂商的模型独立审查，结果互不可见 |
  | 擂台模式 | 多个模型分别完成同一任务，从结果中选定一份采纳 |
  | 评审模式 | 多个模型分别从安全、性能、需求覆盖角度审查 |
  | 蜂群模式 | 任务拆分为互不重叠的子任务，由多个模型并行完成后合并 |

  审查请求默认用盲审模式，干活默认用经济模式；可在面板里改默认模式。

可靠性方面：

- **跨厂商交叉验证**：审查者默认避开被审产出的厂商，Claude 写的交给 GPT、Kimi、
  Gemini 等审。
- **盲审**：每一路在单独的目录里运行，只收到原始材料和中性问题，看不到我方结论，
  也看不到其他路的结果。
- **核对模型真身**：第三方端点可能把不认识的模型名静默换成默认模型；首次派发前
  发一个短请求，比对响应里的模型名，不符的记「验证失败」，不派发。
- **分层省额度**：经济模式由高档模型出方案、低档模型执行、其他厂商验收；额度用尽
  时按备选顺序改派，结果里写明。
- **不做多数表决**：各路结果在查看页并排，由用户选定采纳；派发前先说明发给谁、
  发什么，同意后才发送。
- **可追溯**：每次派发记录任务原文、应答模型和原始输出；引擎中途退出后可用
  `aicross recover` 核对，并按快照恢复改了一半的项目文件。

## 使用前提

本机装好并登录至少两家 agent 的命令行工具（Claude Code、Codex、Kimi Code、
pi agent 等）。只有一家时，无法交给其他厂商审查。

## 安装

在 [Releases](https://github.com/keros68/aicross/releases) 下载 Windows 安装包，
`aicross_<版本>_x64-setup.exe` 与 `.msi` 装一个即可。安装后命令行里可用
`aicross`。程序数据在 `~/.aicross/`，卸载不删。macOS 与 Linux 目前只保证能编译，
没有安装包。

面板的「开始」页会装好 skill。只用 skill、不装面板时：

```bash
npx skills add keros68/aicross --skill ai-cross -g
```

用 `-a claude-code -a codex -a kimi-code-cli -a pi` 指定 agent。之前从
`keros68/xiaoyu-skill` 装的，先 `npx skills remove ai-cross -g` 再按上面重装。

## 第一次使用

打开 aicross，进「开始」页：

1. 探测：查出本机装了哪些 agent、各有哪些可用模型。
2. 选在哪个软件里用，点「一键配好」：装好 ai-cross skill，按推荐填好审查和干活
   各派给哪些模型。手选的人选和其他软件管理的 skill 不改动。
3. 回到该软件，输入页面给出的示例，例如 `/ai-cross 让别家审一下 main.py`。

要换人选，到「模式」页改。

## 面板

- 开始：探测、一键配好、示例输入。
- 资源：本机各 agent 与模型的状态，第三方端点在首次派发前核对模型真身。
- skill：各软件里 ai-cross skill 的安装位置与版本。
- 模式：每个模式里每个位置派给哪个模型。
- 派发记录：最近的派发，点一条看各路结果并采纳。

## 命令行

skill 通过命令行调用 aicross，一般不需要手动执行：

```powershell
aicross config show --host claude
aicross dispatch --project D:\work\project --preset blind --text-file task.txt
aicross status <run_id> --wait
aicross open <run_id>
```

全部子命令与退出码见 [docs/MANUAL.md](docs/MANUAL.md)，命令行契约见
[docs/CONSOLE-CHANNEL.md](docs/CONSOLE-CHANNEL.md)。

## 密钥

密钥只经子进程环境变量传给各家命令行工具，不写入 aicross 自己的文件，不进模型
上下文。发出的任务文本默认脱敏，认得 PEM 私钥、Bearer 令牌、password / token /
api_key 赋值等常见形态；裸写的 `sk-…` 之类认不出来，规则见 docs/MANUAL.md
「密钥与脱敏」。

## 开发

Tauri 2（Rust）+ React 19 + TypeScript + Vite。

```bash
cd src-ui && npm install && npm run build
cargo check && cargo test
npx --yes -p @tauri-apps/cli@2 tauri dev
```

## 许可

[MIT](LICENSE)。
