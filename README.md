# aicross

多厂商 agent 的派发引擎与结果查看器。用户在自己常用的宿主（Claude Code /
Codex / Kimi Code / pi agent 等，装 ai-cross skill）里说话，模型提议派发、
用户同意；aicross 以命令行拉起各家 CLI、隔离盲审、核对真身、留痕，并把各路
结果并排交给用户裁决。窗口只有首页「最近的派发」与查看页两页。

**本项目 fork 自 [Coffee CLI](https://github.com/edison7009/Coffee-CLI) by
edison7009。** 桌面外壳（自绘标题栏、无边框窗口、单实例转发）来自上游；无窗口
派发引擎、资源探测与冒烟核验、查看页是 aicross 新增或改造的部分。上游致谢见
`NOTICE`，商标政策见 `TRADEMARKS.md`。

许可：AGPL-3.0-or-later，与上游一致。

## 用法

```powershell
aicross dispatch --project D:\work\project --preset economy --text-file D:\tmp\task.txt
aicross status <run_id>
aicross open <run_id>
```

派发在宿主对话里由 ai-cross skill 发起；全部子命令、退出码与数据文件见
`docs/MANUAL.md` 与 `docs/CONSOLE-CHANNEL.md`。

## 开发

```bash
cd src-ui && npm install && npm run build
cargo check && cargo test
npm exec @tauri-apps/cli@2 -- tauri dev
```

进度见 `tasks/README.md`。
