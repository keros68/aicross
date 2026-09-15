# TASK-08A 修正轮 R2

先读 `tasks/fix-08a-r1.md`、`tasks/REPORT-08A.md` 的「修正轮 R1」一节，再读 `tasks/review-08a-r1-ds.md`（与 `tasks/review-08a-r1-glm.md`，若存在）。逐条处理，每条在 `tasks/REPORT-08A.md` 末尾「修正轮 R2」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文（含 build）贴进报告。不跑 `cargo fmt`、保持文件原有换行；不动 `src/channels/`；不起窗口、不发真机请求、不 commit、不 push。

## 必修（按严重度）

1. **cherry-pick 失败分级重做**（DS 1，R1 第 13 条）：只有 git 退出码非零且输出含冲突标志（`CONFLICT`/`could not apply`）才是 `conflict` → `cherry-pick --abort`；启动失败/超时/其它非零 → 该件 `skipped` + note（超时写明），不做 abort、不停整批；`abort` 本身失败才停整批并把后续件 `skipped`，note 写「项目可能处于冲突态，请手动 `git cherry-pick --abort`」。补测试：超时件 → skipped 且后续继续；abort 失败 → 后续 skipped + 指引。
2. **失败件重试闭环测试**（DS 2，R1 第 12/7 条）：端到端：第一次验证红 → `test_failed` 并回退本件全部提交 → 修好验证命令 → 第二次 `merged`；多提交件按序 pick；ahead 路径验证红时全部回退。
3. **提交信息 title 断言**（DS 3）：合并后 `git log -1 --format=%s` 含 `<title>`。
4. **登记项与 HEAD 的边界**（DS 4、5）：`git worktree list` 里单条登记 canonicalize 失败只跳过该条，不整批 skipped；`project_head` 取不到时该件 `skipped` + note，不误报「没有改动」。补测试。
5. **回退失败不吞**（DS 6）：`reset --hard` 失败进 note 并把该件标 `skipped`，整批停止（项目状态不确定）。补测试。
6. **check 侧小项**（DS 7、8）：超限 note；非法 glob 只让该条进 `unmatched`/note，不整条命令 Err；写盘开关路径用 `dispatch_data_root()`。
7. **报告与范围**（DS 9）：既有夹具超时 1s→10s 的改动在报告里写明理由（防 flake）；补 build 输出原文；报告自相矛盾处更正。
