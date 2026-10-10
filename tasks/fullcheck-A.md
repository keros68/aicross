未运行构建、测试、agent CLI，也未在真实用户目录复现安装或删除。指定生产代码及相关调用链已完成静态检查；测试正文未逐条读完，TASK／REPORT／fix 按 A 块相关需求查阅，未全文通读全部期次。全程未修改文件、未联网。

1. **P1：查询 skill 状态会删除唯一可恢复的安装备份。**  
   位置：[src/skill_install.rs:451](D:/work/aicross/src/skill_install.rs:451)、[src/skill_install.rs:583](D:/work/aicross/src/skill_install.rs:583)。  
   **问题与触发条件：**更新先把旧安装改名为备份，再把临时目录换到目标位置。如果进程在两步之间终止，或者恢复备份失败，旧安装只剩备份目录。下次 `skill status` 或窗口查询状态时，会仅凭名称前缀递归删除它。另一个进程查询状态，也能删除正在安装的临时目录或备份；进程内锁无法保护这个场景。  
   **建议：**状态查询保持只读；目标缺失时保留并恢复备份。清理必须确认事务已结束、目录确属本次安装，并协调跨进程操作。现有清理测试只验证空残留目录被删，缺少中断恢复、恢复失败和跨进程安装覆盖。

2. **P1：斜杠命令沿目录链接写入、删除外部文件。**  
   位置：[src/slash.rs:196](D:/work/aicross/src/slash.rs:196)、[src/slash.rs:638](D:/work/aicross/src/slash.rs:638)。  
   **问题与触发条件：**`file_state` 使用会跟随链接的 `metadata`，Kimi 命令目录也未检查 junction／symlink。如果 `~/.kimi-code/skills/<命令名>` 链接到外部目录，其中 `SKILL.md` 带有管理标记，`slash add` 会覆盖外部文件，`slash remove` 或 `preset remove` 会删除外部文件。这是路径中间目录的链接，删除操作不会只删除链接本身。  
   **建议：**写入和删除共用路径归属检查，识别命令目录及文件的链接；外部管理的链接应拒绝修改。除真正不存在外，元数据检查失败应按占用处理。补 Kimi junction、symlink 和悬空链接测试。

3. **P1：安装的 skill 尚未接入当前编排配置器，官方流程会失败或忽略默认模式。**  
   位置：[src/skill_install.rs:23](D:/work/aicross/src/skill_install.rs:23)、[skill-bundle/ai-cross/references/channels.md:351](D:/work/aicross/skill-bundle/ai-cross/references/channels.md:351)。  
   **问题与触发条件：**安装器直接写出内嵌包，但包内引擎示例固定传 `--preset economy`，不传 `--host`，也没有按要求读取 `aicross resources` 和 `aicross config show`。配置了两个主 agent 后，照此流程派发会因缺少 `--host` 退出；只有一个主 agent 时，固定的 economy 又会覆盖用户设定的默认模式。这与 PRODUCT.md 第 44、51、71 行不符。  
   **建议：**在 skill 源仓更新流程后同步安装包：读取资源与配置时带当前宿主，派发沿用该宿主和配置的默认模式。补安装包内容与实际命令契约的集成检查。

4. **P2：预设在加锁前读取，旧快照能绕过修改后的校验并重新写回人选。**  
   位置：[src/console_cli.rs:2082](D:/work/aicross/src/console_cli.rs:2082)、[src/console_cli.rs:2127](D:/work/aicross/src/console_cli.rs:2127)、[src/recommend.rs:881](D:/work/aicross/src/recommend.rs:881)。  
   **问题与触发条件：**窗口命令可在阻塞线程池并发执行。一次 `set-seat` 先读到自定义模式中的盲验证位；另一次 `preset_save` 将该位置改成共享执行位并清空人选；前者随后取得锁，仍按旧预设校验，可以把 agy 写进新共享执行位。推荐同样在锁外读取预设，能按过期形状写配置。  
   **建议：**在同一把锁内重新读取预设与 modes，再校验和写入。资源解析可以在锁外完成，但预设必须在锁内重新确认；默认模式设置、清除位置等同类调用方也应统一处理。补协调读写时序的并发测试。

5. **P2：推荐声称“不采用非法角色默认”，实际生效人选仍会回退到它。**  
   位置：[src/recommend.rs:567](D:/work/aicross/src/recommend.rs:567)、[src/recommend.rs:616](D:/work/aicross/src/recommend.rs:616)，实际补人路径：[src/engine/mod.rs:1004](D:/work/aicross/src/engine/mod.rs:1004)。  
   **问题与触发条件：**例如将 acceptance 设为 agy——当前命令允许——且验收位没有合规候选。推荐输出“未采用；需手动指定”，但写入 `None`。`fill_seats` 随后把 `None` 解释为使用角色默认，最终验收仍是 agy；派发会在需要项目副本时失败。这没有完成 fix-61-r1 第 2 条要求。  
   **建议：**让推荐结果能够明确阻止非法默认回退，或在共同补人路径应用位置能力校验。现有测试只检查空 seat 和说明文字，应继续检查保存后的 `config show` 与派发计划中的实际人选。

6. **P2：临时安装目录的“同卷”假设在 Windows 目录重定向下不成立。**  
   位置：[src/skill_install.rs:570](D:/work/aicross/src/skill_install.rs:570)、[src/skill_install.rs:586](D:/work/aicross/src/skill_install.rs:586)。  
   **问题与触发条件：**若 `C:\Users\…\.agents\skills` 是指向 `D:\Skills` 的 junction，安装目标实际在 D 盘，而临时目录、备份目录按字面父路径建立在 C 盘。首次安装的换名、已有安装的备份都会成为跨卷目录移动并失败。当前检查只识别末端 `ai-cross` 的链接，不能发现这种父目录重定向。  
   **建议：**按解析后的实际目录选择同卷暂存位置，且避免暂存目录被宿主扫描；补父目录跨卷 junction 的安装与更新测试。

未发现 P0。除上述问题，A 块中未发现坏配置拒写、v1 处理、密钥输出或多行任务内容经 argv 传递的新增缺陷。

修后发布。
codex
修后发布。
