## 逐条复核（review-00-glm-r2.md §3 的 5 条 vs 修正轮 R2）

1. **配置损坏启动 panic — 已核实修复**：settings/recent 解析失败分别回退默认与空列表并产出警告（config.rs:63-73、config.rs:80-96），tools.json 失败回退内置注册表（config.rs:106-125），`write_json` 改 UUID 临时文件+sync+rename+失败清理（config.rs:35-58），警告经 lib.rs:291 聚合入初始状态并在 App.tsx:190 顶部渲染，测试 config.rs:184 覆盖。

2. **close_terminal 主线程阻塞 — 已核实修复**：lib.rs:211-217 改 async 命令，`taskkill /T /F`+kill+wait 全部经 `run_blocking("关闭终端")` 移出主线程。

3. **Changes 切项目残留 — 已核实修复**：Changes.tsx:14-21 在 `projectPath` 变化时清空 changes/diff/busy/error 并递增 `projectVersion`，三个异步操作（Changes.tsx:23-66）回写前全部带版本守卫、`finally` 的 busy 也不豁免。

4. **关 tab 竞态/幽灵终端 — 已核实修复**：前端仅在 `api.closeTerminal` 成功后移除 tab、失败保留并报错（App.tsx:149-160）；后端用同一把 `PtyState` 锁管 `opening/records/cancelled`（pty.rs:37-39），close 先到时插 cancelled（pty.rs:202），open 首段命中 cancelled 直接返回不 spawn（pty.rs:67-69），prepare 完成后二次检查 cancelled、命中即杀掉新进程且不入 records（pty.rs:96-102）——三种时序（先关/启动中关/启动后关）都闭合。

5. **浅色终端配色 — 已核实修复**：light 分支改为 #f5f6f8 背景/#171a21 前景/#2c4a8c 光标（TerminalView.tsx:110-113），`.terminal-host` 背景改用 `--terminal-bg` 变量并在暗色媒体查询下切换（styles.css:23、47、174），与 xterm 主题监听同源同步。

## 本轮新引入问题扫描

未发现本轮新引入的功能性问题。两个非阻塞备注（均非本轮引入）：`write_terminal` 仍是同步命令且 `pty.write` 持全局 state 锁做 write_all+flush（lib.rs:196-206、pty.rs:159-176），PTY 背压时可卡全部 IPC——R1 即如此，本轮 close/open 改造未加剧（close 的进程终止在锁外，pty.rs:197-216）；TerminalView disposed 后 `await` 恢复处无二次 disposed 检查就 `terminal.focus()`（TerminalView.tsx:66-67），紧竞态下最多触发一次多余全局错误提示，理论性瑕疵。另外 `cancelled` 集合对“open 永不到达”的一次性 UUID 条目会残留（pty.rs:202），永不复用，仅内存尘埃。

**结论：5 条全部核实修复、无本轮新引入缺陷，代码层面可进入人工窗口 smoke。**