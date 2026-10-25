# REPORT-63 「开始」页

## 改了什么

- 新页签「开始」排在最前，没存过页签时默认进它；存过页签的照旧（`config/page-tabs.ts`）。常驻挂载，探测或配置进行中切走再切回状态仍在。
- `config/StartPage.tsx`：三张步骤卡。
  1. 本机的 AI：「探测」/「重新探测」，进度同资源页（`resources-detect-progress`）；探测后列出确认装了的 agent 名称与可选模型数，「详情」进资源页。
  2. 配好：一排主 agent 按钮（与顶栏下拉同一状态）；skill 状态（未安装 / 已安装 / 可更新 / 由某软件管理，旧于内置标黄）；「一键配好」= 需要时 `skill_install(host, global)` + `config_recommend(host, null, false)`；完成后显示审与干活各用哪个模式、每个位置的角色与模型（未选或失效虚线红框），按钮换成「重新推荐」与「改人选」。推荐返回的说明逐条列出。
  3. 怎么用：「回到 <主 agent>，输入：」加两条示例（审 / 干活），各带复制按钮；调用写法取 `slash_list.invoke`，读不到按主 agent 默认。
- `config/start-view.ts`：纯逻辑。审 / 干活的模式选法与 skill 1.23.0 一致（审：默认模式是盲审或评审就用它，否则盲审模式；干活：默认模式是共享、经济、擂台、蜂群之一就用它，否则经济模式）。
- `ConfigPages.css` 加 `start-*` 样式；`App.tsx` 接入。
- 文档：`docs/MANUAL.md` 窗口页数与「开始」一节；`AGENTS.md` 技术栈一行改成现状（原写「只有首页与查看页」）。

没改后端。

## 验证

```
cd src-ui && npx vitest run      → Test Files 22 passed, Tests 240 passed
npm run build                    → built in 866ms
npx eslint src                   → 无输出
```

新增 `config/__tests__/start-view.test.ts`（找到的 agent、模式选法、位置行、配好状态、要不要装 skill、示例输入），`page-tabs.test.ts` 按新页签更新。

开发窗口（`tauri dev`）启动正常，顶栏出现「开始」页签。用桌面自动化点击页签时坐标偏移，没能进到开始页截图；开始页的实际外观与「一键配好」在真机上未核对。

## 没做

- 「一键配好」不设默认模式，沿用现有默认（未设时为经济模式）。
- 没有改模式、资源、skill 三页。
