# TASK-04 收尾积压（09-14 窗口真派后的剩余小缺陷，待下一轮一起修）

来源：编排者 09-14 10:07–10:40 在合并后 main（`7e0567a`）窗口里真派三次经济流水线（两次 PASS 全通、一次用 `-k ZZZNOPE` 验证命令强制走验证失败修正轮 1+3 次后停下）。主链路已闭环，以下是过程中记下的小项，按严重度排。

1. **验证重试到顶后，从未派出的验收节点显示「失败」**（中）：应显示「已跳过」或保持「待派」并说明「上游验证未通过」。图卡、右栏节点链、结果屏三处一致。
2. **重试节点的路由行文案**（小）：现在是「验证命令未通过，请修正后重新运行。 验证命令失败 → codex/…」，理由被塞进角色位。应是「执行（修正 2/3）→ codex/… ｜ …」，理由另起一行。
3. **验证失败 error 文本带 ANSI 转义**（小）：`验证命令失败（exit=Some(1)）\e[33m…`，剥掉转义再写留痕与 Reply；`exit=Some(1)` 改成 `exit=1`。
4. **盖预设菜单（portal）不响应 Escape 与外点**（小）：只有再点「盖预设」或滚画布才收起。加 Escape + 外点关闭。
5. **新开 tab 不继承工作目录**（小）：`+` 新建 tab 后左栏要重新选目录；应继承上一个活动 tab 的 folderPath（或 localStorage `cc-folder`）。
6. **切 tab 后右栏仍显示上一张图的节点链**（小）：`graphFocus` 只在关 tab 时清；切到没有图的 tab 时也应清或按当前 tab 的图显示。
7. **根列直接发消息默认「未指定模型」**（小）：`appendTurn` 的 `defaultTurnTarget()` 空 agent；改为按 roles.json 的执行默认填。
8. **diff.patch 新增文件头 `+++ after/...`**（小）：git `--no-index` 对新文件输出的 `+++ "after/..."` 带引号没被 R3-21 的行头改写命中；补引号形态。
9. kimi 验收留痕 `identity`/usage 为空（已知，kimi 不报），留痕里明示「kimi 不报用量/真身」而不是空串。

已验证正常、不用动：角色默认盖预设即填；G1 写权限/验证命令显示；逐层推进遇未指定模型停在 G1；结果屏三栏/产物/小计/采纳；验证门（验证失败不派验收）；留痕 raw 外置；DeepSeek 路由别名；Claude fable 别名探测与冒烟真身。

## 09-14 追加（来自 docs/PRODUCT.md「重派与崩溃恢复的一致性」与 COMPETITIVE-LANDSCAPE.md 研究侧）

10. ~~已做（TASK-06 A4/B7）~~ 推进逻辑补测试：同一结果回调送达两次只派下一层一次（`nextStageStep` / `start()` 幂等）。
11. G2 加「疑似空转」提示：同一工具调用重复、N 分钟无文件变化而 token 在涨 → 提示 + 叫停（依据 2606.01365：预警后干预可省一半以上 token）。
12. ~~已做（TASK-06 A1/B2）~~ 单写者「重派」默认用派发前快照回滚（或执行角色默认 worktree）；worktree 基于 HEAD 看不到未提交改动，项目目录不干净时 G1 提示。
13. ~~已做（TASK-06 A2/B1）~~ 崩溃后「继续」先按留痕与会话文件对账（已完成 / 未完成 / 未知），已完成直接收回产物，未知交 G3，不自动重跑单写者节点。
14. ~~已做（TASK-06 A3/B12）~~ 执行者提示词与工具白名单禁掉不可回滚的外部动作（push、发布、装包、付费接口），留给用户在 G3 之后做。
15. 蜂群（第 3 期）按 CAID：拆分时声明文件范围，G1 显示重叠，逐个合并逐个跑测试，有先后依赖的任务不进蜂群。
16. ~~已做（TASK-06 B4，窗口核过）~~ G1 确认屏的「备选」下拉溢出卡片右边（05B 加了第四个控件后一行放不下，2026-09-14 窗口手测）：改成两行或按宽度折行。
17. ~~已做（TASK-06 A6/B3）~~ 「已切备选」靠回复正文首行标记识别（Reply 无字段），标记行会随 serializeContext 进下游 prompt 并随图落盘；模型正文恰好以「（已切备选：」开头会误判。根治要给 Reply 追加字段（05B 复审 DS 4 / GLM 2）。
18. ~~已做（TASK-06 B6）~~ 盲父节点 → 同目标共享子节点仍会继承 session/runner（continuationSession 只查子节点），靠池键含 blind + ForeignSession 兜住，无测试（05A R3 复审 DS 3）。
19. ~~已做（TASK-06 B5）~~ 前提附件手输按逗号分隔，含逗号的路径会被拆坏；校验只在设置页保存时做，手改 premises.json 不会在抽查前拦（05B R2 复审）。
20. ~~已做（TASK-06 A5）~~ ACP 队列恰满 8MB 时截断标记自身也进不了队列，丢行不留痕（process.rs 预留失败分支，极边缘）；取消/并发用例在池里留活进程属测试卫生（05A R3 复审 DS 4/5）。

## 08D-R1 复审剩余（09-15，两家都判可交付；低危，择期）

21. 验证窗口内用户写入项目的未跟踪文件会被回退定点删除（rollback 集 = 暂存 ∪ 验证后脏路径，分不清验证产物与用户文件）；至少在 note 明列被删路径，理想是移入隔离目录而非删除。（GLM 1）
22. `unmerged_swarm_paths` 查询失败/截断一律按空处理 → 真冲突可能降成 skipped 且丢冲突清单；区分「无 unmerged」与「无法判定」。（GLM 2）
23. 补丁新建文件回退后留下空目录。（GLM 3 / DS 3）
24. 验证脏路径 note 未走 20 条截断。（DS 4）
25. 临时补丁文件在系统 temp（含项目 diff），进程被强杀时残留；改落 `.dispatch/` 或加启动清理。（DS 5）
26. 入口脏检查不过滤排除目录，批中验证留下的 `__pycache__` 会让下一批入口整批 skipped；plan 侧 `check_swarm_plan` 的 status 也未带 `-uall`。（DS 6/9、GLM 4）
## TASK-10 R1 复审剩余（09-15，两家都判可交付；低危，择期）

27. 损坏的 commander.json 拒写后前端 `.catch(() => {})` 吞错，快照不消费，每次重启 tab 反复重试且无提示；给保存失败一个可见提示。（GLM 1）
28. 启动器目录选择器 `pickFolder` 不置 `seeded`，晚到的默认可能覆盖用户刚选的目录。（GLM 2 / DS 6 同类）
29. `graphViewTabs` 按 session id 索引，RESTART_TERMINAL 换 id 后标记丢失、关 tab 不清理；空白 tab 换目录会翻回启动器。（GLM 3 / DS 5）
30. 保存默认按闭包捕获的旧 id 消费，保存在途时换目录会把旧快照留在新 id 上（窗口几毫秒）。（DS 4）

## TASK-10 窗口实测（09-15 16:25）

31. 总指挥 PTY 继承了宿主环境里的 `CLAUDE_CODE_CHILD_SESSION` 标记（开发窗口从 Claude Code 会话里起时出现，Claude Code 提示 transcript saving off）：起总指挥时清掉 `CLAUDE_CODE_*` 之类宿主会话变量，用户从 Claude Code 里启动 aicross 时也不受影响。
32. 总指挥 tab 标题被 CLI 的 OSC 标题（「Claude Code」）盖掉，看不到 `agent · 模型 · 目录名`；改成 commanderTitle 优先、OSC 标题进悬停。

## TASK-14 R1 复审剩余（09-15，两家都判可交付；低危，择期）

33. 额度回滚失败只改返回前端的结果，该次尝试的派发记录 frontmatter/runner_note 里没有「回滚失败」。（GLM 2）
34. `resets \d` 仍会命中「server resets 3 connections」；无 reset/retry 语义时 ISO 兜底取正文第一个时间戳。（GLM 3）
35. `status 429`（空格无冒号）与 `HTTP/1.1 429` 不识别，方向是漏判。（GLM 4）
36. G1 收敛的「手动改过」集合不含「待核对」分区的勾选；provider 为空的目标写回额度会静默落空（与首次验证按 agent+id 命中不一致）。（DS 5/6）

## TASK-12 R1 复审剩余（09-15，两家都判可交付；低危，择期）

37. 结果屏「已跳过」行取全图所有带 skipped_reason 的节点，不按本轮过滤（与遗留卡同型，可随步状态收集本轮 id）。（GLM）
38. 同批既有验收 NEEDS_FIX 又有跨分支执行 validation_failed 时，修正轮早退跳过验证失败处理，等待验收停在 draft，只能手动重派执行节点；内置单链预设到不了。（GLM / DS F）
39. 验证重派轮不重生盲「验证」兄弟（修正轮会），重派轮产物没有 VALIDATION.txt；验证缓存键只有 run_id+项目，不含材料目录/命令；`run_one` 路径不清缓存；verify 通过后叫停仍会跑完验证命令。（GLM / DS E）
40. 第 2 条（未发送不附材料）缺 run_one 端到端负例：首次验证失败 + 盲 + review_material_validation 走 run_one，断言无 VALIDATION.txt、validation_attached=false、命令未执行。（DS C）

## TASK-18 复审剩余（09-15，两家都判可交付；低危，择期）

41. 右栏预设下拉的 listbox 角色混排（组标签 div、details/summary 在 listbox 内，规划项无 role=option），方向键/字母跳转不如原生 select。（DS 5 / GLM 3）
42. 规划中两项只在右栏下拉出现，盖预设菜单与讲解页没有；盖预设菜单的源码字符串断言偏弱。（DS 7 / GLM 5）
43. 没有用户预设时不出现「自定义模式」组；只有一个用户预设时组标签与条目同名两行。（DS 6）

## TASK-13 R1 复审剩余（09-15，两家都判可交付；低危，择期）

44. 本轮新建的空目录在核对后丢失（回退按 .created 的 dir: 删，恢复只重建 changed 文件的父目录）；恢复阶段按 dir: 清单重建。（GLM 1）
45. `.redgreen` 标记固定写 project/.dispatch，不遵循 write_project_dispatch；设置关闭时对账发现不了。（GLM 2 / DS H）
46. 既有文件被替换成链接后恢复阶段会删掉它且不报失败；before 有、after 无但路径存在应判恢复失败。（DS B）
47. 绿跑后取消分支丢弃验证输出；对账 unknown 的说明进不了界面、record_path 指向 .redgreen；worktree 节点的对账文案未指出是 worktree。（GLM 3 / DS E、J）
48. .created 的 dir: 项计入 RollbackSummary.removed 的文件计数（文案）；Windows 启动失败靠英文输出特征识别，spawn 失败变体无测试。（DS F、I / GLM 4）
49. 验证门叫停分支（R1 复审 A）无测试；核对耗时仍计入节点时长。（GLM 5）

## TASK-15 R1 复审剩余（09-15，两家都判可交付；低危，择期）

50. 自动直启的「真实接入」断言仍是源码文本比对；把 effect 判定抽成纯函数再单测。（DS 2）
51. tab 条上的 claude/codex 图标仍是内联字面量副本（CLAUDE_SVG/CODEX_SVG），与 public/icons/tools 同源但各存一份；设置页与接入页两处仍直出机器 id。（DS 4/5、GLM 5）
52. 启动器 `onShown` 每次渲染重复登记（幂等，无功能影响）；有存量默认时探测期间首屏短暂空白属有意取舍。（GLM 2/3）

## TASK-16 R1 复审剩余（09-15，两家都判可交付；低危，择期）

53. 控制台 run 未接图进度 store（图上节点卡无心跳）：`startPipelineHeadless` 支持 begin/end，但 progress store 挂在 GraphTab 内，通道拿不到；需要 graphId → store 的注册表。（DS 2）
54. `aicross adopt` 要求图在内存 state 中，用户关掉该图 tab 后得 409；先 loadGraph 兜底。（GLM N4）
55. `head_and_options_are_http_methods` 与 `HTTP_TIMEOUT` 两条测试仍是自证；NSIS 脚本本体无自动化测试，只有 TS 镜像的纯函数测试。（GLM N5 / 5）

## TASK-17 R1 复审剩余（09-15，两家都判可交付；低危，择期）

56. `detect_into` 的 pi 分支已成生产死代码且保留「不刷登录」旧语义，日后恢复会无声回归；清理或注明。（GLM）
57. 引擎跳过未登录备选的说明只加在返回结果上，不进主尝试留痕；引擎每候选各读一次库取 login。（GLM）
58. `status:"ready"` 但退出码非 0 判未登录而非未检查（有意取舍，仅记口径）。（GLM）

## 安装包（09-15 夜）

59. ~~WiX PATH 片段在 light 阶段失败，MSI 出不了包。~~ **已修（09-16）**：报错是 `LGHT0230`——组件里只有 `Environment`、没有 KeyPath，WiX 就拿 Directory 当 KeyPath，而这种组件不允许 `Guid="*"`。改法是加一个 HKCU 标记值当 KeyPath（与 `System="no"` 的每用户 PATH 同范围）并把 GUID 写死（发布后不可再改，否则升级会重复加 PATH 项）。实测 MSI 出包 5.34 MB，`Environment` 表为 `=-PATH / [~];[INSTALLDIR]`（追加、卸载时移除），组件 KeyPath 指向该注册表行。

## TASK-19 复审剩余（09-16，低危，择期）

60. `src-ui/src/components/left/Explorer.css` 仍有既有的 `12.5px` 字号，与 12 / 13 / 14 / 15 / 16 / 18 的整数阶梯不一致；本期不扩大到 Explorer 的全量字号整理。

## TASK-19 R1 复审剩余（09-16，两家都判可交付；低危，择期）

61. ~~缩放回退的幂等只在当前 JS 会话内；页面重载后仍可能叠一次 CSS 倍率。~~ **已修（09-16）**：生效模式（webview / css）跟着档位一起存进 localStorage，重载后内存表为空时改问存档；存储不可用时退回只靠内存，不抛错。红绿核过。
62. `rail-density-19.test.tsx` 的结构正则是贪婪匹配，状态槽之后再插一列不会红；只有插在名字与状态之间才红。（GLM 3）**同型的另一处已修（09-16）**：`ui-scale-19.test.ts` 扫 PRODUCT.md 定案编号时一路切到「## 顺序」，中间插一节定案就会把别节编号算进来，已改成切到下一个二级标题为止。本条的 rail 正则仍待收紧。
63. `docs/PRODUCT.md` 字号阶梯未记「终端与 pre 例外」，与清理后仅剩的两处 11px 字面上不一致。（GLM 4）

64. ~~Tauri dev 的文件监听把 `src-ui/node_modules/.vite/vitest/**/results.json` 也算成变更，在开发窗口开着时跑 vitest 会触发整轮 Rust 重编 + 重启 app（09-16 窗口实测）。~~ 已加 `.taurignore`（09-16）；**下次启动 dev 时才生效，当时运行中的窗口未重启验证**。

65. ~~接入管理页没有 Esc 关闭，只能点「关闭」。~~ **已修（09-16）**：按层级关——先关「手工接入模型」子弹窗、再关「手工接入 agent」子弹窗、都关完了才关整页，判定抽成 `resourcesEscTarget` 纯函数并有三格用例。

## TASK-20 R1 复审剩余（09-16，两家都判可交付；低危）

66. 右栏预设下拉（`PresetGroups.tsx` 的 `PresetSelect`）的 Esc 关闭挂在冒泡阶段，会被建议条的捕获监听短路：有建议条时打开下拉按 Esc，下拉不收、建议反被忽略。这是第三处 document 级 Esc（前两处 modal 已放行）。代价是再按一次 Esc，可接受；要收口就把下拉打开时的根节点并入放行判定。（GLM 2）
67. `proposalTargetFlags` 用 `closest('input,textarea,[contenteditable]')`，属性选择器会把 `contenteditable="false"`（显式不可编辑）也算成输入控件，多排除一次按键。方向安全（多放行少抢键），元素罕见。改法：`[contenteditable="true"]`。（GLM 5）
68. 归类调用固定了 `runId/nodeId = 'proposal-classify'`，两条建议并发走归类时 `runs/proposal-classify/dispatch/` 下的 STATE.md 与 raw 文件同名互写，最后写者胜。判定只取回复文本不读 STATE，结果不受影响，仅并发时日志失真。（GLM 6）

## TASK-21 复审剩余（09-16，两家都判可交付；低危）

69. 旧库重判比 `channels::known_model_route` 严一档：重判闭包多要求「响应的 provider 等于路由表 provider」，而 channels 侧只比模型名。当前唯一路由（deepseek 两边同 provider）下两者一致；将来若出现跨 provider 的官方路由，实跑冒烟能过、旧失败记录却翻不了案。方向保守，先记着。（GLM 4）
70. 重判结果只在内存，落盘要等下一次任意写。所有消费方都走 `load()` 所以行为正确，与 `ensure_verification` / `ensure_resolved_id` 同一既有约定，盘上 `resources.json` 会长期保持旧值。（GLM 5）
