# TASK-83 引擎：只读位置的改动提示、带序号的派发目录、配置损坏的退出码

## 背景

`tasks/REPORT-81.md` 在 Windows 上核对 skill 时记下了几处引擎一侧的问题。那次用的是已安装的 0.3.7，源码是 0.3.10：动手前先用当前源码构建的程序把每一项重跑一遍，已不存在的划掉。

## 范围

1. **kimi 只读位置改了项目文件且没有提示**（`REPORT-81` 第 5 项后半）。`acpEnabled: false` 时引擎走 `kimi -p`，它没有只读档，能读目录外的文件、能改项目文件。只读位置派发前后各取一次项目目录的 git 状态，有变化就在该路的备注与 run 摘要里列出文件，文案与 skill 一致：「派发期间项目里这些文件有变化：…。kimi 没有只读档，核对是不是它改的。」项目不在 git 里时不检查、不报错。
2. **`adopt --verdict-file` 在带序号的派发目录上落错位置**（第 7 项后半）。`src/project_dir.rs` 的 `find_run_dir` 按 `short_id` 取前 8 位匹配后缀，`20261115-201451-2` 这样的目录找不到，裁决写到了 `$AICROSS_HOME/runs/`。改成先按目录名完全相等找，再走原来的后缀规则。
3. **`modes.json` 损坏时退出码仍为 0**（`REPORT-81` 另记）。读不出来时以非 0 退出并在 stderr 写明文件路径与原因。
4. **`aicross dispatch --help`** 现在报「缺少 --project」、退出码 2。`dispatch` 及其他子命令认 `--help` / `-h`，打印用法，退出码 0。

## 不做

- 不把 skill 自己的派发登记进引擎的索引，查看器仍只显示引擎的派发。
- 不改 `--preset`、`--seat`、`--read-only-task` 的名字。
- 不拦 kimi 写文件，不加系统级沙箱。
- 不改应用版本号，不发 Release（由用户定）。

## 验证

```bash
cargo check && cargo test
cd skills/ai-cross && python -m unittest discover -s tests
```

每项补单元测试。真机：第 1 项让 kimi 在只读位置上新建一个项目文件，核对备注；第 2 项对一个 `<时间>-2` 目录跑 `adopt --verdict-file`，核对 `裁决.md` 落在项目的 `.aicross/runs/<时间>-2/`；第 3 项写坏 `modes.json` 后看退出码（用临时 `AICROSS_HOME`，不动真实配置）；第 4 项看 `aicross dispatch --help` 的退出码与输出。

## 交付

`tasks/REPORT-83.md`，没做完、不符合的项写在最前面。Conventional Commits 英文提交，不 push。
