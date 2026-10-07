# fix-50-r1 修正清单

来源：Claude 子代理与 Antigravity（Gemini 3.8 Flash）两份盲审，外加编排者核对 TASK-51 前端契约时发现的两条。编排者已复现或核对代码的条目注明「已核实」。

## 要改

### 高

1. **`config show --host` 跑不通（两家共同，已核实：退出 2「不认识的参数：--host」）**：`show` 分支先调 `extra_arg_error(args, 3)`。删掉它，只留 `unknown_option_error(args, 3, &["--host"])`。补测试：`config show --host claude` 退出 0 且返回该主 agent 的配置。
2. **`recommend --all` 跑不通（两家共同，已核实：退出 2「缺少 --all 的值」）**：校验函数把白名单选项都当带值参数。给校验函数加「不带值的开关」清单（或先剥离 `--all`）。补测试：`--all` 在开头、中间、末尾都能通过。
3. **厂商写法不一致，跨厂商避让失效（两家共同 + 编排者，已核实）**：`hosts.rs` 输出「智谱 / 月之暗面 / 阿里」，资源库 `resources::infer_vendor` 用 `Zhipu / Moonshot / Alibaba`。改法：
   - 全仓共用一张厂商规范化函数，统一用资源库的英文名（Anthropic、OpenAI、Google、Moonshot、Zhipu、DeepSeek、Alibaba、MiniMax、StepFun…）；`hosts.rs` 的推定结果、`vendorOverride`、推荐里的比较全部先规范化，比较不区分大小写。
   - `vendorOverride` 只接受规范化后的已知厂商名或 `auto`（窗口下拉传的就是这些英文名，见 TASK-51 报告），其它值退出 2。
   - 主 agent 是 claude 且推定为第三方端点时，资源库里 claude 原生模型（同一后端）在推荐比较时按端点厂商算，不能被当成另一家选为审查者。
   - URL 匹配改为先解析主机名再匹配；补 `anthropic.com` → Anthropic、`api.kimi.com` / `moonshot` → Moonshot、`bigmodel.cn` / `z.ai` → Zhipu 等常见端点。
   - 测试夹具改用 `infer_vendor` 的真实输出，不再手写中文厂商名。补测试：kimi 主 agent 时 Moonshot 模型不会被选为审查者。
4. **`skill_status` 不标写入位置（编排者，TASK-51 前端需要）**：每个位置项追加 `writeTarget: bool`（是不是「我们安装时写入的位置」），前端据此决定按钮状态，不再按路径猜。

### 中

5. **空位置被当成占位（两家共同）**：`kept_effective` 对 `seat.is_none()` 且由角色默认补上的位置返回 true，厂商被提前计入同阶段与执行厂商集合，挤掉合理人选。空位与 recommended 位不计入占位；只有最终选不出人、维持角色默认时才计入，并在 notes 写「该位置将由角色默认 X 执行」。补带 `roles.json` 默认值的推荐测试。
6. **推荐计算不在锁内（两家共同）**：`recommend_at_home` 在锁外宽松读并计算，写盘时整份覆盖，可能冲掉期间手选的 manual 人选；`changed` 为 false 时还会绕过严格读。改为锁内先严格读、再计算、再写盘。
7. **档位表 provider 精确匹配（两家共同）**：真实 provider 是 `zai-coding-cn`、`opencode-go` 等，表里写 `zai` 落不中。provider 改为前缀匹配（或按资源库实际出现的 provider 名补全），补 `opencode-go` 等条目与 claude 别名（`fable`、`opus`、`sonnet`、`haiku`）。补测试：用真实 provider 名核对档位与计费。
8. **`config_snapshot` 窗口命令不走主 agent 缺省规则（两家共同）**：改为经 `resolve_window_host`；但 **host 为 null 且一份配置都没有时，返回资源与空配置，不报错**（资源页在首次探测前就要能读到快照，见 TASK-51 报告）。多于一份配置且未给 host 时与命令行一致报错。
9. **进程环境变量 `ANTHROPIC_BASE_URL` 不可注入（Claude 独有）**：在设了该变量的会话里跑测试会失败。把环境变量作为参数注入（同家目录），补环境变量路径的测试。
10. **include_dir 不随 skill-bundle 变化重编（Claude 独有）**：`build.rs` 加 `println!("cargo:rerun-if-changed=skill-bundle")`。
11. **更新时中途失败留下半成品（Claude 独有）**：先写到同级临时目录（含标记文件），成功后再替换旧目录。
12. **测试缺口（两家）**：「按次排后」测试换成按次模型在资源库排前、同档的夹具；补「只有一份配置时不给 --host 自动推定」的写命令与 show 用例；可更新判定的正向用例先放好标记文件；出方案优先高档的用例；codex 新旧两处同时存在的 duplicate 用例。

### 低

13. 没给 `--host` 时不要为了校验去读资源库（读不了也不该退出 3），只在给了 `--host` 时校验。
14. `resolve_host` 与 `set_vendor_override` 大小写口径统一（都先转小写）；推定出的主 agent 也校验是否仍被检测到。
15. capabilities 去掉 `dialog:default`（对话框只在 Rust 侧调用）。
16. 路径存在但是普通文件时按 `external` 处理，不当 `none`。
17. `--project` 等于家目录时按规范化路径去重，避免 duplicate 误报。
18. `skill_uninstall` 失败文案写「卸载任务失败」。

## 不改

无。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前；`docs/CONSOLE-CHANNEL.md` 同步厂商取值、`writeTarget`、`config_snapshot` 的 null 行为。改完在 `tasks/REPORT-50.md` 末尾追加「修正轮 r1」一节。测试一律用临时目录，绝不写真实家目录。不 commit，不 push。
