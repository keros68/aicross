# aicross

多厂商 agent CLI 的桌面派发控制台。在一个窗口里接入本机的 agent CLI（Claude Code / Codex / Kimi Code CLI / pi 等），看到调用方案，手动编排，@ 各家模型干活。

**本项目 fork 自 [Coffee CLI](https://github.com/edison7009/Coffee-CLI) by edison7009。** 桌面外壳、终端、工作区、会话与修改记录来自上游；多厂商资源盘点、事实源探测、模型冒烟核验与派发编排是 aicross 新增的部分。上游致谢见 `NOTICE`，商标政策见 `TRADEMARKS.md`。

许可：AGPL-3.0-or-later，与上游一致。

## 运行

```bash
cd src-ui && npm install && npm run build
cargo check
npm run tauri dev --prefix src-ui
```

状态：fork 基座就位，改造进行中。进度见 `tasks/README.md`。
