# REPORT-76：派发在宿主对话里只留要做决定的内容

按 `tasks/TASK-76-quiet-host-conversation.md` 实施。只改了 skill 与文档，引擎 `src/` 未动；skill 版本 1.28.0 → 1.29.0，应用版本未改。

## 改了什么

- `references/dispatch.py`
  - 开场：一次派发一行（`route_line`、`who`、`knobs`、`size_text`）。选人理由只在宿主厂商不确定时进「注意」；操作提示合成一行。
  - `review --go`：`split_answer` 按标题行拆出开头段、「会出错」、「建议」；每路只打印前两部分，「建议」报字数。认不出两节或带 `--full` 时整篇打印。
  - 小结骨架：要拍板、未验证、一行脚注（`brief`）。去掉路由回显、独立性单列、账单（`bill_line` 已删）。各路都没有结果时只说明原因去向，不给骨架。
  - `run`：开场与骨架同样压缩；回答仍整篇打印。
  - `runs_root`：旧记录迁移每个进程每个项目只做一次。
  - 一路超时、出错、真身不符时调用 `inventory.mark_failure`。
- `references/inventory.py`：`mark_failure`、`recently_failed`；`choose_reviewers` 把一天内没结果的条目排到后面并加一条说明；`mark_smoke` 成功时清除 `last_failure`。
- `SKILL.md`：「开场与收尾」改为新格式；写明何时不等回话直接发、何时先等同意；后台等待只说一次；「建议」默认不进对话。
- `references/rules.md`「路由公示」与闭环规则第 7 条、`references/channels.md` 脚本一节与引擎一节、`README.md`、`docs/PRODUCT.md`「机制」第 2 条与闸门表：与上面一致。
- `skill-bundle/ai-cross/`：`python scripts/sync-skill.py` 重新生成。

## 现有测试的改动

| 测试 | 原断言 | 现断言 | 原因 |
|---|---|---|---|
| `test_without_go_sends_nothing` | 含 `pi:zai-coding-cn / glm-5.3（Zhipu）` | 含 `盲审 → Zhipu glm-5.3（经 pi） ｜ 高档 · thinking=高`，且不含「新模型」 | 开场格式变了；提醒不进派发 |
| `test_go_runs_blind_and_leaves_trace` | 含 `独立性：` | 含 `- 审查：Zhipu glm-5.3（经 pi，独立，`，且不含「账单」 | 独立性并入脚注；账单不打印 |
| `test_two_vendors_and_partial_failure` | 含 `状态 error` | 含 `｜ error ｜` | 每路标题行格式变了 |
| `test_named_entries_same_vendor_flagged` | 含 `codex / gpt-6.1-sol（OpenAI）` 与「用户点名」 | 含 `OpenAI gpt-6.1-sol（经 codex）`，不含「用户点名」 | 用户自己点的名不再回显理由 |
| `test_plan_then_go_sends_task_verbatim` | 含 `执行 → pi:zai-coding-cn / glm-5.3-flash（Zhipu）` | 含 `执行 → Zhipu glm-5.3-flash（经 pi）` | 开场格式变了 |
| `test_all_failed_exits_1` | — | 另加：不含「要你拍板」 | 全部失败不给骨架 |

新增 4 条：`test_go_shows_only_the_will_break_section`、`test_answer_without_the_two_sections_is_shown_whole`、`test_failed_route_goes_to_the_back_until_it_succeeds`、`test_legacy_migration_runs_once_per_project`。

## 验证

本机 macOS，Python 3.11.15。没有装 pytest，测试是 unittest 写的，用 unittest 跑：

```
$ cd skills/ai-cross && python3 -m unittest discover -s tests
Ran 90 tests in 1.647s

OK (skipped=1)
```

在 2026-11-13、11-14 三份真实审查记录上跑 `split_answer`（只读记录，不发请求）：

```
pi-deepseek-v4-pro-验证.md: 全文 3008 字 → 显示 1598 字（开头 75，会出错 1523，建议 1399 未显示）
pi-glm-5.3-验证.md: 全文 3484 字 → 显示 1384 字（开头 403，会出错 981，建议 2089 未显示）
pi-qwen3.7-plus-验证.md: 全文 2176 字 → 显示 810 字（开头 6，会出错 804，建议 1355 未显示）
```

真机派发（临时目录，材料是一段 8 行的示例代码，低档、思考低）：

1. 默认选人、两家：开场输出为

   ```
   - [ai-cross] 盲审 → DeepSeek deepseek-v4-flash（经 pi）、Zhipu glm-5.3-flash（经 pi） ｜ 低档 · thinking=低 ｜ 材料 avg.py，1.3 KB
   ```

   加 `--go` 后两路都没有结果（18 秒返回，退出码 1）：DeepSeek 返回 400「Model deepseek-v4-flash isn't included in OpenCode Go」，Zhipu 返回 429「OpenCode Go usage limit reached (monthly, resets in 2d 22h)」。两个条目被记下 `last_failure`，随后 `inventory.py pick` 改选 `pi-xai.xai`，说明里有「上次派发没有结果、已排到后面：pi-opencode-go.deepseek、pi-opencode-go.zhipu」。
2. 点名 `pi-xai.xai` 与 `kimi.moonshot`：18 秒返回，退出码 0。两路各只显示开头一句与「会出错」一节，「建议」分别报 245 字、344 字未显示；脚注为

   ```
   - 审查：xAI grok-4.6（经 pi）（独立，15s）、Moonshot kimi-code/kimi-for-coding-highspeed（经 kimi）（非独立：隔离不完整，18s） ｜ 另有 2 路的建议没有展开 ｜ 记录 .aicross/runs/20261115-170141/
   ```

   这次之后把脚注里并排的两对括号合成一对（`（经 pi，独立，15s）`），并让全部失败时不再输出骨架；这两处改动只由单元测试覆盖，没有再真机派发。

## 没做什么

- 审查默认思考强度没改，原因见任务书「不做」。
- `cargo check`、`cargo test` 没跑：本机没有 cargo，且没有改 Rust 代码。
- `SKILL.md` 的新规则（直接发的条件、等待只说一次、建议不进对话）没有做宿主行为测试，模型是否稳定照做未验证。
- 引擎路径（`aicross dispatch`）的输出没有动；本机没装引擎，`channels.md` 里引擎一节的同意规则只改了文字。

## 不确定处

- 「建议」一节不进对话后，审查者因写不出出错场景而放进「建议」的实质性疑虑，宿主默认看不到，只看到字数。三份真实记录里「建议」占全文四到六成。宿主读记录文件可以取回。
- `split_answer` 靠标题行识别两节。标题写法离模板太远时退回整篇打印，不会丢内容，但也不省。
- 本机 manifest 里 `pi-opencode-go` 的低档 `deepseek-v4-flash` 已不在 OpenCode Go 套餐内，这是真机派发时发现的，manifest 未改。
- OpenCode Go 月额度用尽时，同一订阅下的 `pi-opencode-go.qwen` 也会失败，但它没有被连带排后。
