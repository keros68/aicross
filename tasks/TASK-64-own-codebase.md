# TASK-64 去掉上游代码，改 MIT；ai-cross skill 并入本仓

用户 10-25 定：aicross 与 Coffee CLI 不再有代码关系；ai-cross skill 从 xiaoyu-skill 移到本仓。

## 一、去掉上游代码

逐文件对比 `D:\work\coffeecli`，以下仍含上游代码，全部自己重写或删除：

| 文件 | 处理 |
|---|---|
| `port/` | 删除（旧代码，不参与编译） |
| `src/git.rs` | 删除；派发只需要「项目有几处未提交改动」，用 `git status --porcelain` 自写 |
| `src/tool_config.rs` | 重写：只读 `tools.json` 里手动 agent 的 `command` |
| `src/windows_path.rs`、`main.rs` 启动时的 PATH / GDK / 文件句柄处理 | 重写为 `src/startup_env.rs` |
| `src/server.rs` 窗口命令与 `start_ui` 的窗口外壳部分 | 重写 |
| `src-ui` 标题栏、缩放边条、`lib/platform.ts`、`main.tsx`、`tauri.ts` 头部、`index.html`、`vite.config.ts`、`global.css` 中上游段 | 重写 |
| `installer.nsi` | 删除；改用 Tauri 默认模板加 `installerHooks`，只放 PATH 的加入与移除 |
| `scripts/rebuild-icons.py`、`src-ui/resize_icon.js` | 删除（上游工具，不再使用） |
| 注释与 `.gitignore`、`Cargo.toml` 里的上游字样 | 清掉 |
| `LICENSE` / `NOTICE` / `TRADEMARKS.md` | LICENSE 换 MIT，删另两个；`Cargo.toml`、`package.json` 的 license 字段同步 |

Vite / Tauri 官方模板生成的配置（tsconfig、eslint 配置等）不属于上游作品，保留。

行为不变：窗口无边框、自绘标题栏可拖动、双击最大化、四边与四角可缩放、单实例（第二次启动聚焦已开窗口并转发 `open <run_id>`）、首帧后显示、3 秒兜底显示、Windows 直角无阴影、macOS 关窗隐藏；GUI 启动时补齐 PATH；安装加入 PATH、卸载移除且不展开 `%VAR%`。

## 二、skill 并入

- `git subtree split` 取 xiaoyu-skill 中 `skills/ai-cross` 的历史，`git subtree add` 到本仓 `skills/ai-cross`。
- `scripts/sync-skill.py` 默认源改为本仓 `skills/ai-cross`。
- xiaoyu-skill 删除该目录，README 一行指向新位置并给迁移命令。
- 更新 skills-manager 的源与全局 AGENTS.md 中的源仓路径。

## 完成条件

- 对比脚本在编译代码中不再命中上游行（模板配置除外）；全仓无 Coffee 字样（`tasks/`、`docs/history/` 的历史记录除外）。
- `npx vitest run`、`npm run build`、`npx eslint src`、`cargo test` 通过。
- 真机：开发窗口拖动、缩放、最小化、最大化、关闭、单实例转发可用；NSIS 安装包装上后 PATH 有安装目录，卸载后移除。
- `npx skills add <本仓路径> --list` 能列出 ai-cross。
