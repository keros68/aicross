# REPORT-80：manifest 加锁，kimi 动过项目文件时报出来

按 `tasks/TASK-80-manifest-lock-and-kimi-changes.md` 实施。只改了 skill；skill 版本 1.30.2 → 1.30.3。

## 改了什么

- `references/inventory.py`：`manifest_lock`（`fcntl` / `msvcrt` 文件锁，等至多 30 秒）、`update_manifest`；`save`、`set`、`diff` 在锁内运行。
- `references/dispatch.py`：`review`、`run`、`smoke` 收尾的 manifest 更新改用 `update_manifest`；`project_state`、`changed_files`；选中 kimi 时把有变化的文件写进「注意」。
- `SKILL.md`、`references/channels.md`：对应说明。
- 测试新增 2 条：`update_manifest` 写前重读、派发期间变化的文件被列出。
- `skill-bundle/ai-cross/` 重新生成。

## 验证

```
$ cd skills/ai-cross && python3 -m unittest discover -s tests
Ran 106 tests in 1.982s

OK (skipped=1)
```

真机（macOS）：

1. 临时 `AICROSS_HOME` 下 12 个进程同时调用 `update_manifest`，各追加一条备注：`notes kept: 12 of 12`。
2. 让 kimi（kimi-for-coding-highspeed）在 `~/.aicross/scratch/` 下新建文件：回复「已写」，文件存在，内容是要求的那串字。
3. 让 kimi 在一个临时 git 项目里新建 `made-by-kimi.txt`：输出里有「注意：派发期间项目里这些文件有变化：made-by-kimi.txt。kimi 没有只读档，核对是不是它改的。」

## 没做什么

- Windows 上的 `msvcrt` 锁没有运行过。
- 加锁之前没有先复现丢更新。
- 盲审（`review`）里选中 kimi 时的提示只由代码路径覆盖，没有真机跑。

## 不确定处

- 派发期间用户自己或其他进程改了项目文件，也会被列进提示，提示的措辞是「核对是不是它改的」。
