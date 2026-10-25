# TASK-65 skill 升级、软件自动更新、扩充主 agent

用户 10-25 反馈：开始页「旧于内置 1.23.0」看不懂，应直接提示升级；要有 skill 与软件的升级提醒和自动拉取；主 agent 要加主流的几家（OpenCode、Antigravity / Gemini、Hermes、ZCode、CodeBuddy、WorkBuddy、Qoder 等）；主 agent 菜单里的「未知」没必要。自动更新用 Tauri 官方更新器（用户选定）。

## 要做

1. skill 升级
   - 别的软件装的、主 agent 读得到的旧版：标「需要升级到 X」，给「升级」；确认后原目录移到同级配置目录下的 `.aicross-skill-backup/ai-cross-<时间>`（链接只删链接），再写入内置版。skills-manager 管理的不接管，提示到 skills-manager 里更新。命令行 `aicross skill upgrade --host --path`。
   - aicross 自己装在全局写入位置的旧版：启动时自动更新。
   - 一键配好包含升级（先确认）。
2. 开始页缺人提示：只说审与干活两个模式缺几个、为什么、怎么补（去验证 / 手选人选），文字不溢出卡片。
3. 软件自动更新：tauri-plugin-updater，读 `releases/latest/download/latest.json`；签名私钥与密码只在本机 `~/.tauri/`；`scripts/build-release.ps1` 签名并生成 latest.json。
4. 主 agent 扩充：探测时按配置目录或命令记入资源库 `extraHosts`；各家 skill 全局目录按官方文档或本机实测；只装全局、无短命令；Gemini CLI 厂商为 Google，其余不猜。
5. 主 agent 菜单厂商认不出时不显示。

## 完成条件

- 单元测试覆盖升级（备份、原地替换、非本家位置报错）、自动更新判定、缺人提示、扩展主 agent 的检测与安装位置、短命令列表。
- 真机：安装版 0.3.0 收到本机发布的 0.3.1 并装上、重开；扩展主 agent 被探测出来；真实界面升级一份旧版 skill。
