# fix-50-r2 修正清单（复审后）

来源：Claude 子代理对 fix-50-r1 的复审（结论「修改后可以合并，须先修前两条与环境变量注入」）。编排者另在真机上修了 kimi 厂商推定（`hosts.rs` 的 `kimi_default_provider`：provider 写在 `[models."<default_model>"]` 一节），不要改回。

## 要改

1. **高｜pi 主 agent 时整组模型被改记为主 agent 厂商**：`recommend.rs` 的 `comparison_vendor_of_target` / `candidate_comparison_vendor` 只在「主 agent 是 claude / kimi / codex，且模型 provider 为空（原生模型）」时改写为主 agent 厂商；pi 下的模型一律用资源库里的厂商。补一条 pi 主 agent 的推荐测试（pi 下有多家 provider，审查位仍能从 pi 的其它厂商里选）。
2. **中｜给了 `--project` 时全局写入位置的 `writeTarget` 算错**：按 scope 分别比较——global 位置与 `install_path(host, home, None)`，project 位置与 `install_path(host, home, Some(project))`。补带 `--project` 的 `writeTarget` 断言。
3. **中｜`collect_hosts` 仍读真实环境变量**：给 `collect_hosts` 及经过它的 `list_hosts_at`、`set_vendor_override_at_home`、`recommend_at_home` 加环境变量参数，生产入口传 `std::env::var("ANTHROPIC_BASE_URL")`，测试传 None。
4. **中｜skill 更新替换不稳**：
   - 旧目录先 rename 为备份，临时目录 rename 到位，成功后删备份；失败时把备份 rename 回去。
   - 临时名与备份名加 uuid；skill 写入加一把进程内锁（codex、kimi、pi 共用 `.agents/skills`）。
   - 临时目录与备份目录放在 skills 目录之外的同卷位置（如 skills 目录的上一级），避免残留被 agent 当成第二份 skill；status 扫描时忽略并清理残留的临时 / 备份目录。
5. **低｜推荐写盘后读回占着 modes 锁**：写盘后先释放 `_guard`，再调用 `modes_section`。
6. **低｜厂商规范化缺口**：`resources::infer_vendor` 补 gemini / google → Google；`normalize_vendor` 补「Moonshot AI」「Zhipu AI」「阿里云」等别名；「其他」或空厂商不参与同阶段去重与主 agent 避让；URL 规则补 `minimaxi.com`。
7. **低｜单一配置推定的主 agent 未被检测到时的文案**：改为「modes.json 里唯一的配置 <host> 未被检测到，请用 --host 指定，可选：…」。
8. **低｜维持角色默认后的同阶段冲突**：写盘前对每个阶段补一次检查，同阶段厂商重复或撞上主 agent 厂商时追加 notes。

## 不改

无。R4（生产入口 `detect_all_resources_at` 的盖章无直接测试）留作 Later，已记在 REPORT-48。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前；文档同步。改完在 `tasks/REPORT-50.md` 末尾追加「修正轮 r2」。测试一律用临时目录，绝不写真实家目录。不 commit，不 push。
