
未运行 cargo、npm、测试或 agent CLI，未做真实窗口交互与渲染验证；图标图片未逐个视觉检查，历史 REPORT 未逐份通读。以下基于 `50ca3c4..HEAD` 的前端代码审查；Rust 只追踪了命令契约和相关文件操作调用链。

未发现 P0 问题。

1. **P1：窗口读取 skill 状态可能删除另一进程正在更新的安装。**  
   位置：[ModesPage.tsx:340](/D:/work/aicross/src-ui/src/config/ModesPage.tsx:340)、[skill_install.rs:452](/D:/work/aicross/src/skill_install.rs:452)。  
   **问题与触发条件：**窗口自动调用 `skill_status`，后端按名字前缀删除临时目录和备份，锁只在进程内有效。另一 CLI 进程更新时，若已将旧安装改名为备份、尚未把新目录移入，窗口扫描可以删掉两者，导致替换和恢复都失败，原安装丢失。  
   **建议：**状态查询不执行删除；安装操作采用跨进程互斥，残留清理须确认所有权及对应操作已经结束。

2. **P1：卸载确认的路径与实际删除路径不一致。**  
   位置：[SkillPage.tsx:184](/D:/work/aicross/src-ui/src/config/SkillPage.tsx:184)、[skill-view.ts:71](/D:/work/aicross/src-ui/src/config/skill-view.ts:71)、[skill_install.rs:512](/D:/work/aicross/src/skill_install.rs:512)。  
   **问题与触发条件：**每个 `ours` 位置都显示卸载，但请求只传 host、scope，不传确认框展示的路径。例如旧位置 `.codex/skills/ai-cross` 和 `.agents/skills/ai-cross` 都保留安装标记，点击旧位置卸载，实际删除的是 `.agents` 下那份；旧位置仍在。  
   **建议：**当前契约下，只允许卸载 `writeTarget` 位置；若支持旧位置卸载，则传递并严格验证实际目标。补双位置回归测试。

3. **P1：保存结果可能被刷新丢弃，后续编辑会覆盖刚保存的配置。**  
   位置：[ModesPage.tsx:328](/D:/work/aicross/src-ui/src/config/ModesPage.tsx:328)、[ModesPage.tsx:385](/D:/work/aicross/src-ui/src/config/ModesPage.tsx:385)。  
   **问题与触发条件：**读取和写入共用 `snapshotSeq`。保存 A→B 尚未完成时刷新，后发的读取若取得旧配置 A，保存 B 的成功结果会因序号过期而被丢弃，界面继续显示 A。随后修改思考强度或备选，会把包含 A 的整个人选重新保存，撤销 B。  
   **建议：**写入期间推迟读取，写入完成后统一重读；或让过期写入响应触发新的读取，不能直接丢弃。补“旧读取与成功写入交错”的回归测试。

4. **P1：配置了多个主 agent 后，自定义模式无法保存。**  
   位置：[tauri.ts:118](/D:/work/aicross/src-ui/src/tauri.ts:118)、[PresetEditor.tsx:97](/D:/work/aicross/src-ui/src/config/PresetEditor.tsx:97)、[server.rs:379](/D:/work/aicross/src/server.rs:379)。  
   **问题与触发条件：**`preset_save` 后端接受并解析 host，前端完全没有传。当 `modes.json` 存在两个或更多主 agent 配置时，后端返回“需要 --host”，窗口中新建、编辑、复制模式均无法保存。  
   **建议：**将当前 host 从 ModesPage 传至编辑器和 IPC 包装函数；补多主 agent 场景下的命令参数测试。

5. **P2：搜索后重新打开下拉，回车可能清除人选。**  
   位置：[ModelPicker.tsx:74](/D:/work/aicross/src-ui/src/config/ModelPicker.tsx:74)、[ModelPicker.tsx:95](/D:/work/aicross/src-ui/src/config/ModelPicker.tsx:95)。  
   **问题与触发条件：**打开时清空查询，却用旧过滤列表计算选中索引。已有明确人选时，搜索到该人选、关闭、重新打开，旧索引可能为 0；新列表第 0 项变成“清除人选”，直接回车便删除已保存的人选设置。备选下拉也受影响。  
   **建议：**按清空查询后的列表计算索引，或保存选中项的 key。补“搜索→关闭→重开→回车”组件测试。

6. **P2：后端已标记不能派发的人选，窗口仍显示配置完成。**  
   位置：[tauri.ts:316](/D:/work/aicross/src-ui/src/tauri.ts:316)、[config-view.ts:117](/D:/work/aicross/src-ui/src/config/config-view.ts:117)、[console_cli.rs:1115](/D:/work/aicross/src/console_cli.rs:1115)。  
   **问题与触发条件：**后端返回 `effective.invalid`，前端类型和状态计算都忽略它。例如自定义盲位置配置 agy 后，仅把可见性改为共享，原人选会保留；后端标记违反规则，但窗口仍显示正常，并计入“已配好”，派发时才拒绝。  
   **建议：**接收并优先展示 `invalid`，纳入配置完成判断；覆盖主选和备选违反位置规则的测试。

7. **P2：弹层按窗口空间定位，却被页面滚动容器裁切。**  
   位置：[ModelPicker.tsx:76](/D:/work/aicross/src-ui/src/config/ModelPicker.tsx:76)、[ConfigPages.css:285](/D:/work/aicross/src-ui/src/config/ConfigPages.css:285)、[global.css:505](/D:/work/aicross/src-ui/src/styles/global.css:505)。  
   **问题与触发条件：**弹层是滚动正文内的绝对定位元素，正文设置了 overflow 裁切；定位却按整个窗口高度计算。向上展开且模型列表较长时，会把标题栏、工具栏所在区域也算作可用空间，搜索框或列表上部落到正文边界之外。  
   **建议：**使用 portal 加固定定位，或按实际滚动容器边界限制高度。现有测试仅检查数值计算，应补小窗口下的真实布局验证。

8. **P2：下拉过滤和“可用模型”统计不符合当前要求。**  
   位置：[config-view.ts:63](/D:/work/aicross/src-ui/src/config/config-view.ts:63)、[config-view.ts:191](/D:/work/aicross/src-ui/src/config/config-view.ts:191)、[config-view.ts:505](/D:/work/aicross/src-ui/src/config/config-view.ts:505)。  
   **问题与触发条件：**TASK-62 要求只列可用、已验证的模型，但当前过滤仍允许非当前值的“连接超时”“额度用尽”模型进入可选列表。“可用模型”统计还包含被下拉排除的未验证模型。  
   **建议：**统一使用明确的可选状态集合；现值可以保留展示，统计复用相同条件。补超时、额度用尽、未验证三个状态的测试。

除上述问题外，C 块未发现新增的密钥记录或多行 prompt 经 argv 传递路径；首次使用流程、文案没有发现需单独报告的功能问题。关键测试缺口集中在 IPC 参数、真实卸载目标、异步请求交错和下拉组件交互。

结论：修后发布。
