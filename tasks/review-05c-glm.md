# TASK-05C 盲审（GLM-5.3-Flash via cc-switch，只读，2026-09-14）

# 代码审查 — ui/phase5c（TASK-05 C 节）

审查方式说明（影响所有结论的边界）：本会话没有 shell 权限，`git diff main` 跑不了。改动集是按系统提供的 git status 快照 + REPORT 文件清单 + 文件 mtime 排序 + 内容核查还原的；构建产物数字（chunk 大小、cargo/vitest 通过数、msi/exe 出包）引自 REPORT 的日志原文，未复跑。越界与行尾结论注明了依据。

## 1. 需求完整性

- `docs/MANUAL.md:55-56` — **密钥第 5 条与代码行为不符**。手册写「替换 `sk-` 开头的串、URL 里的内嵌凭据和 Bearer 头」，但 `src/dispatch.rs:1552-1562` 的六条正则是：PEM 私钥、`Authorization: Bearer`、password/token/api_key 类赋值、AKIA、**仅 postgres/mysql/mongodb** 三种协议的连接串 URL。没有通配 `sk-` 前缀的规则（`sk-…` 只有作为 `api_key:` 赋值出现才被第 4 条打中，`dispatch.rs:2166-2177` 的测试正是这么构造的）；`https://user:token@host` 形态完全不覆盖。这是安全章节，用户会据此高估脱敏范围。改法：要么按六条真实规则改写手册措辞，要么在 `redaction_patterns()` 补 `sk-[A-Za-z0-9]{16,}` 和通用 `scheme://user:pass@` 两条再保留手册说法。
- `src-ui/src/components/center/graph/GraphTab.tsx:408-409` — 「选预设」步骤在右栏收起时**跳转失效**。`dispatch({TOGGLE_RIGHT_PANEL})` 是 React 状态更新（handler 返回后才提交），而 `window.dispatchEvent` 同步执行；此刻 `Compiler.tsx:34` 的监听器所在的 RightPanel 还没 mount，事件必然丢失。结果是面板打开了，但停在 localStorage 记忆的上次 tab（比如「任务」）而不是「方案与编排」。新装用户靠默认值 `'orchestration'` 侥幸正确，老用户（`'aicross-right-panel-tab'` 存了 `'tasks'`）必踩。改法：事件延迟到提交后（`setTimeout(0)`/rAF），或 Compiler mount 时检查一个一次性 pending 标记。
- `src-ui/src/components/right/TaskBoard.tsx:56-58` — **「损坏文件不覆盖」只覆盖了一半**。`commands.loadTasks()` 被 Rust 拒绝（文件存在但读不出：权限、占用、非 UTF-8，`server.rs:3460-3466` 现在会返回 Err）时，前端走 catch 落回 localStorage 空板，`isLoadedRef=true`，下一次任何变更就把新数组写回文件（`write_tasks_at` 只校验载荷是数组，不检查目标文件是否已有内容）。`server.rs:3456-3459` 的文档注释宣称「frontend stops saving on the second so the file survives untouched」——对解析失败路径成立，对读取失败路径不成立。改法：`isTauri` 下 load 被拒绝时走 `loadDamaged` 语义（停止写入 + 提示），localStorage 回退只保留给非 Tauri 环境。
- `tauri.conf.json:40` — `bundle.targets` 仍是上游的 `["deb","rpm","dmg","app","msi","nsis"]`。Windows 主机实测只出了 msi+nsis（CLI 按平台过滤），但留着四个从未在支持平台上验证过的目标，谁在 mac/linux 上跑 `tauri build` 就会第一次真编它们。低优先级：砍到当前承诺的平台（`msi`,`nsis`）或加注释说明其余目标未验证。
- 其余各项核过无问题：`beforeBuildCommand` 对象形式（`tauri.conf.json:9`）是 Tauri 2 的正确写法且与 `beforeDevCommand` 同形；图标是纯 AC 字标、`tauri.conf.json:43-52` 引用的 8 个文件全部存在、Square*/StoreLogo/android/ios 全套齐、`256/512` 由脚本补齐；引导三步的「打开目录」（dialog 插件已在 `server.rs:3861` 注册）与「接入资源」（`TitleBar.tsx:29-33` 监听 `OPEN_RESOURCES_EVENT`）指向真实位置；任务 tab 默认关（`Compiler.tsx:23` 默认 `'orchestration'`，且换用新 key 避开上游遗留值）；MANUAL 各节实测行数 4/9/9/7/10/5，均 ≤10 行，文案是说明体。

## 2. 逻辑正确性

- shiki 改法：`LANG_MAP` 的 31 个语言 id 与 `LANG_LOADERS` 的 31 个 key 逐一对照完全一致，常用语言（ts/js/py/rs/go/java/c/cpp/cs/前端三件套/md/yaml/toml/sh/sql…）无遗漏；缺的（lua、ini 等）是 `LANG_MAP` 原来就没有的扩展名，回落纯文本与改前行为一致。`shiki/langs/*.mjs` 返回的 namespace 传给 `loadLanguage` 会被 shiki 的 `MaybeModule` 解包 `.default`，用法成立。未发现问题。
- WASM `?url`：打包后是 dist 内的同源静态资源（Windows 下 `http://tauri.localhost/assets/*.wasm`），`fetch` 同源可用，不依赖 `assetProtocol`；`tsconfig.app.json:8` 有 `vite/client` 类型，`tsc` 能过。未发现问题。
- React.lazy 三处兜底：三处都在条件渲染内（`App.tsx:287`、`TitleBar.tsx:196`、`CenterPanel.tsx:1966`），首屏不加载；`fallback={null}` 在本地资源下只有几十毫秒空档。小瑕疵：齿轮点下去到 modal 出现之间无任何反馈，本地资源下可接受，不改也行。
- 引导对「探测未完成」的处理：资源侧正确（`modelsRead` 标记 + `modelsError` → `null` 不显示，`GraphTab.tsx:389`）；**图列表侧不行**——`GraphTab.tsx:186` `listGraphs` 失败时 `setMetas([])`，`graphCount` 变 0，引导可能在图状态未知时出现，与 388 行注释自己立的「读不出按不知道处理」规则矛盾。改法：失败时置一个 metasError，`graphCount` 同样给 `null`。
- reducer 抽取：`cycleStatus` 与 `NEXT_STATUS` 循环一致；`moveTask` 的展平-重插逻辑与按 `STATUS_ORDER` 分组渲染自洽（存盘顺序变为渲染顺序是上游既有行为）。注意 `TaskNoteView.tsx:176-193`、`TaskPromptView.tsx:300-316` 各自还留着一份内联重排实现，「变更搬进纯函数」只覆盖了列表视图——不是行为缺陷，但拖放语义现在有三份拷贝。

## 3. 安全

- tasks.json 读写：路径固定在 `paths::aicross_root()`（`server.rs:3444-3449`），前端只传内容不传路径，无注入面；写前校验 JSON 数组、同目录临时文件 + rename、失败清理临时文件，原子性成立。除上面 1 中 TaskBoard 的读取错误路径外未发现问题。
- 图标脚本：只用 `node:fs` + `node:zlib`，零网络、零第三方依赖。未发现问题。
- MANUAL 密钥节：第 1-4、6 条核实准确（`src/` 下确无 `~/.pi/auth.json`、`~/.cc-switch/*.db` 的直接读取，cc-switch 只经 `cc_switch.py` 子进程间接接触，`resources.rs:932`；「卸载不删 `~/.aicross`」也核实成立，`installer.nsi:842-847` 只在勾选后删 WebView2 配置目录）。第 5 条问题见维度 1 第一条，是本期最该修的一处。

## 4. 代码质量与契约

- 越界核对（依据：内容检索 + mtime 排序，非真 diff）：`src-ui/src/graph/` 无任何 tasks/first-run/icon 相关内容；`src/channels/` 检索 tasks/shiki/icon/manual 零命中；`src/dispatch.rs` 内容只有前期已验证的脱敏与 cc-switch 逻辑；本期 Rust 改动集中于 `server.rs` 新增段。与 REPORT「未动三个禁区」的说法一致，但此结论的证据强度低于真 diff，合并前建议编排者跑一次 `git diff main --stat` 复核文件清单。
- CRLF：`src/server.rs` 4611 行中 4607 行 CRLF（4 行 LF），`CenterPanel.tsx` 2094 行中 1972 行 CRLF（122 行 LF）——两个混排文件确实没被整文件翻面，与 REPORT「按字节打补丁」的说法吻合；`MANUAL.md`、`shiki.ts` 纯 LF。
- `vite.config.ts:110-116` — strictExecutionOrder 的注释到位：说明了开了会让 45 个语法 chunk 进入口 modulepreload 集、以及「语法是纯数据模块、顺序保证买不到东西」的理由，与 REPORT 不确定处 2 一致。核过无问题。
- 小项：`vite.config.ts:136` `output: {…} as never` 把整个 output 对象铸成 never 绕类型检查，代价是这一段失去全部类型提示，用 `// @ts-expect-error` 或只铸不认识的字段更好。`shiki.ts:130-137` `tokenizeByLang` 全仓零调用方（注释里说的 Markdown 围栏路径不存在），本期重写该文件后留下死导出，接上或删掉。

## 5. 测试

- Rust 4 条真实且断言目标行为（`server.rs:3511-3567`：缺失→`[]`、往返+无临时文件残留、坏载荷不覆盖好文件、损坏文件原样返回不吞成 `[]`）；vitest 两条新文件真实（`task-reducer.test.ts` 覆盖 5 种拖放情形和 8 种坏输入，`first-run.test.ts` 正面覆盖了验收条款「只在无图无资源时出现」）。REPORT 的通过数可信但未复跑。
- 最该补的 3 个：
  1. **load 错误路径**：`commands.loadTasks` reject → 断言板面进入停止写入状态而非可写的空板（直接钉住维度 1 的 TaskBoard 缺陷）。
  2. **shiki 冒烟**：经真实 loader `tokenizeFile` 一段 ts/py 样本断言 tokens 非空 + 未知语言返回 `null`——REPORT 不确定处 1 自己承认 shiki/core + `?url` 这条路径零覆盖，且它是本期风险最高的功能改写。
  3. **引导第三步接线**：`rightPanelHidden=true` 时点「选预设」，断言最终 tab 是「方案与编排」（jsdom；或先把动作序列抽成纯函数再测时序）——直接钉住事件丢失缺陷。

---

## 对照 C 节原始需求是否完整实现

**一句话：五项交付全部落地且主体质量过硬，但有三处需要修复才能算闭环——手册的脱敏描述与实际规则不符、tasks.json「损坏不覆盖」对读取错误路径失效、引导第三步在右栏收起时跳转丢失。**

缺项（按严重度排序）：

1. `docs/MANUAL.md:55-56` 密钥第 5 条 overstates 脱敏范围（无 `sk-` 通配规则、URL 仅三种数据库协议）——安全文档错误。
2. `src-ui/src/components/right/TaskBoard.tsx:56-58` 读文件失败回退空板后仍会写盘，可覆盖原文件，违背本期的核心保证（数据丢失边界）。
3. `src-ui/src/components/center/graph/GraphTab.tsx:408-409` 「选预设」事件在右栏未 mount 时丢失，老用户落在错误 tab。
4. （低）`GraphTab.tsx:186` 图列表读取失败按 0 张图处理，引导判定不自洽。
5. （低）`shiki.ts:130` 死导出 `tokenizeByLang`；`TaskNoteView`/`TaskPromptView` 各留一份内联拖放实现；diff 高亮、拖放、三步跳转仍无窗口手测（任务书禁了 dev 窗口，需编排者补）。
---
usage: [usage] provider=Zhipu GLM model=GLM-5.3-Flash input=3122311 fresh=125639 cache_create=0 cache_read=2996672 output=30652 turns=82 ms=1121475
