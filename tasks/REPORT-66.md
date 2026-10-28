# REPORT-66 探测说清原因、指定路径、模式说明

## 改了什么

- 未安装说明：`resources::agent_state_note` 写「没找到 <命令>；安装后重新探测，或指定路径」，指定的路径不存在时写「指定的路径不存在：<路径>」。
- 资源库里还有模型的 agent 不折叠进「未安装」（`config-view.ts` 的 `isListed`）。
- 指定路径：`ResourceAgent.customPath`；`resources::custom_program` 供 `agent_catalog::resolve_program` 与 `channels::resolve_binary` 先查，文件不存在时退回原有查找。命令行 `aicross resources set-path <agent> [<路径>]`，窗口命令 `resources_set_path`；资源页在未安装的 agent 上给「指定路径」，已指定时显示路径与「修改」「清除」，保存后重新探测。
- 「全部测试」：有第三方模型就显示，没有待测模型或未安装时置灰并在悬停提示里写原因（`newuser-view.ts` 的 `batchSmokeButton`）。
- 模式说明：`mode-kind.ts` 的 `modeGuide`。内置模式卡片名称下一句「适合……」，名称旁标「只读」或「改项目文件」，原介绍放进悬停提示；自定义模式显示用户写的介绍。
- 文档：`docs/MANUAL.md` 模式表补齐六个模式（适合 / 怎么分工 / 改文件），补指定路径说明与术语；`docs/CONSOLE-CHANNEL.md` 补 `set-path` 与 `custom_path`。

## 验证

```
cargo test                 → 701 passed, 10 ignored
src-ui: npx vitest run     → 252 passed
npx tsc -b                 → 无输出
```

真机（隔离的 `AICROSS_HOME`）：
- 把 pi 的指定路径设到一个 PATH 以外的包装脚本，重新探测得到版本 `9.9.9-custom` 与 34 个模型；清除后探测回到 1.3.0。
- cursor 未安装时说明为「没找到 cursor-agent；安装后重新探测，或指定路径」。
- 开发窗口：模式页的「适合……」与只读 / 改项目文件标记显示正常；资源页 pi 卡片有「全部测试」，未安装行有「指定路径」。

## 没做 / 不确定

- 指定路径的编辑行与置灰的「全部测试」按钮没有在窗口里看过（视觉检查中途停止），只有单元测试覆盖。
- pi 程序本身报错时，`detect_pi` 在 stdout 为空时会读 stderr，可能把报错行当成模型。这是原有行为，本期没改。
