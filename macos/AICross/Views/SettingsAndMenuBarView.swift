import SwiftUI
import AppKit

// MARK: - 派发设置弹窗（1:1 镜像 SettingsDialog.tsx + macOS 专属增强）

struct SettingsSheetView: View {
    @ObservedObject var store: AppStore
    let onClose: () -> Void

    @State private var loadedView: DispatchSettingsView? = nil
    @State private var loadError: String = ""
    @State private var isSaving: Bool = false
    @State private var saveNote: String = ""
    @State private var saveError: String = ""

    // 草稿字段
    @State private var concurrency: Int = 2
    @State private var consoleRunLimit: Int = 2
    @State private var maxRequestsText: String = "12"
    @State private var fixRoundLimit: Int = 1
    @State private var swarmLimit: Int = 3

    @State private var timeoutMinutesText: String = "10"
    @State private var execTimeoutMinutesText: String = "20"
    @State private var idleTimeoutMinutesText: String = "3"

    @State private var redact: Bool = true
    @State private var writeProjectDispatch: Bool = false
    @State private var validationCommand: String = ""
    @State private var acpEnabled: Bool = false

    private var activeAcpReports: [AcpAdapterReport] {
        !store.acpReports.isEmpty ? store.acpReports : (loadedView?.acp ?? [])
    }

    private var acpSummaryText: String {
        let reports = activeAcpReports
        guard !reports.isEmpty else { return "" }
        let parts = reports.map { "\($0.agent) \($0.state)" }
        return "适配器：\(parts.joined(separator: "，"))"
    }

    private var hasMissingAcpAdapter: Bool {
        activeAcpReports.contains { $0.state == "未安装" || $0.state == "安装失败" }
    }

    private var computedPatchResult: (patch: [String: Any]?, error: String) {
        guard let orig = loadedView?.dispatch else { return (nil, "") }

        guard let maxReq = Int(maxRequestsText.trimmingCharacters(in: .whitespaces)),
              (1...100).contains(maxReq) else {
            return (nil, "单次派发上限需填写 1–100 的整数")
        }

        guard let tMin = Double(timeoutMinutesText.trimmingCharacters(in: .whitespaces)),
              tMin >= 1.0 / 60.0, tMin <= 60.0 else {
            return (nil, "只读位置超时需在 1 秒 – 60 分钟之间")
        }

        guard let eMin = Double(execTimeoutMinutesText.trimmingCharacters(in: .whitespaces)),
              eMin >= 1.0 / 60.0, eMin <= 60.0 else {
            return (nil, "可写位置超时需在 1 秒 – 60 分钟之间")
        }

        guard let iMin = Double(idleTimeoutMinutesText.trimmingCharacters(in: .whitespaces)),
              iMin >= 1.0 / 60.0, iMin <= 60.0 else {
            return (nil, "连续无输出超时需在 1 秒 – 60 分钟之间")
        }

        let tSecs = Int((tMin * 60.0).rounded())
        let eSecs = Int((eMin * 60.0).rounded())
        let iSecs = Int((iMin * 60.0).rounded())
        let cleanCmd = validationCommand.trimmingCharacters(in: .whitespacesAndNewlines)

        var patch: [String: Any] = [:]
        if concurrency != orig.concurrency { patch["concurrency"] = concurrency }
        if consoleRunLimit != orig.consoleRunLimit { patch["consoleRunLimit"] = consoleRunLimit }
        if maxReq != orig.maxRequests { patch["maxRequests"] = maxReq }
        if fixRoundLimit != orig.fixRoundLimit { patch["fixRoundLimit"] = fixRoundLimit }
        if swarmLimit != orig.swarmLimit { patch["swarmLimit"] = swarmLimit }
        if tSecs != orig.timeoutSecs { patch["timeoutSecs"] = tSecs }
        if eSecs != orig.execTimeoutSecs { patch["execTimeoutSecs"] = eSecs }
        if iSecs != orig.idleTimeoutSecs { patch["idleTimeoutSecs"] = iSecs }
        if redact != orig.redact { patch["redact"] = redact }
        if writeProjectDispatch != orig.writeProjectDispatch { patch["writeProjectDispatch"] = writeProjectDispatch }
        if cleanCmd != orig.validationCommand { patch["validationCommand"] = cleanCmd }
        if acpEnabled != orig.acpEnabled { patch["acpEnabled"] = acpEnabled }

        return (patch, "")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("派发设置")
                    .font(.system(size: 18, weight: .semibold))
                Spacer()
                Text("aicross v\(store.appVersion)")
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(.secondary)
            }

            if !loadError.isEmpty {
                Text(loadError)
                    .font(.system(size: 13))
                    .foregroundStyle(LGTheme.dangerRed)
            } else if loadedView == nil {
                HStack {
                    Spacer()
                    ProgressView("正在读取设置…")
                    Spacer()
                }
                .padding(.vertical, 40)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        limitsSection
                        timeoutsSection
                        otherSection
                        macOSIntegrationSection

                        if let path = loadedView?.path, !path.isEmpty {
                            Text("设置文件：\(path)")
                                .font(.system(size: 11.5, design: .monospaced))
                                .foregroundStyle(.tertiary)
                                .textSelection(.enabled)
                                .padding(.horizontal, 4)
                        }
                    }
                    .padding(.vertical, 2)
                }
                .frame(maxHeight: 450)
            }

            let validationErr = computedPatchResult.error
            if !validationErr.isEmpty {
                Text(validationErr)
                    .font(.system(size: 12))
                    .foregroundStyle(LGTheme.dangerRed)
            }
            if !saveError.isEmpty {
                Text(saveError)
                    .font(.system(size: 12))
                    .foregroundStyle(LGTheme.dangerRed)
            }
            if !saveNote.isEmpty {
                GlassNoticeBanner(
                    icon: "info.circle.fill",
                    message: saveNote,
                    onDismiss: { saveNote = "" }
                )
            }

            HStack {
                Spacer()
                Button("关闭", action: onClose)
                    .buttonStyle(.glass)

                let patchCount = computedPatchResult.patch?.count ?? 0
                Button(isSaving ? "保存中…" : "保存") {
                    Task { await saveChanges() }
                }
                .buttonStyle(.glassProminent)
                .tint(LGTheme.emerald)
                .disabled(isSaving || computedPatchResult.patch == nil || patchCount == 0)
            }
        }
        .padding(24)
        .frame(width: 580)
        .task {
            await loadSettings()
        }
    }

    private var limitsSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("上限")
                .font(.system(size: 12.5, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 4)

            GlassCard {
                settingSegmentedRow(
                    title: "并发上限",
                    hint: "同一阶段同时运行的位置数",
                    choices: [1, 2, 3],
                    selection: $concurrency,
                    showTopDivider: false
                )
                settingSegmentedRow(
                    title: "同时派发上限",
                    hint: "同时进行的派发次数，到顶后新的派发被拒绝",
                    choices: [1, 2, 3],
                    selection: $consoleRunLimit
                )
                settingNumberRow(
                    title: "单次派发上限",
                    hint: "一次派发包含的位置数（1–100）",
                    unit: "路",
                    text: $maxRequestsText
                )
                settingSegmentedRow(
                    title: "修正轮上限",
                    hint: "验收或验证未通过时最多重派的次数",
                    choices: [0, 1, 2, 3],
                    selection: $fixRoundLimit
                )
                settingSegmentedRow(
                    title: "蜂群上限",
                    hint: "蜂群模式一次最多拆出的件数",
                    choices: [1, 2, 3, 4],
                    selection: $swarmLimit
                )
            }
        }
    }

    private var timeoutsSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("超时")
                .font(.system(size: 12.5, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 4)

            GlassCard {
                settingNumberRow(
                    title: "只读位置超时",
                    hint: "只读与盲审位置的总时长",
                    unit: "分钟",
                    text: $timeoutMinutesText,
                    showTopDivider: false
                )
                settingNumberRow(
                    title: "可写位置超时",
                    hint: "可写位置的总时长",
                    unit: "分钟",
                    text: $execTimeoutMinutesText
                )
                settingNumberRow(
                    title: "连续无输出超时",
                    hint: "连续没有输出到这个时长按超时结束",
                    unit: "分钟",
                    text: $idleTimeoutMinutesText
                )
            }
        }
    }

    private var otherSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("其它")
                .font(.system(size: 12.5, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 4)

            GlassCard {
                settingToggleRow(
                    title: "派发内容脱敏",
                    hint: "发送前把密钥、令牌等常见形态替换为占位符",
                    isOn: $redact,
                    showTopDivider: false
                )
                settingToggleRow(
                    title: "派发记录写进项目目录",
                    hint: "不开启时只写入 ~/.aicross 数据目录",
                    isOn: $writeProjectDispatch
                )
                GlassRow(verticalPadding: 10) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("验证命令")
                            .font(.system(size: 13.5, weight: .medium))
                        Text("可写执行位置跑完后在项目里运行的命令；留空不验证")
                            .font(.system(size: 11.5))
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    TextField("例如 cargo test", text: $validationCommand)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: 12.5, design: .monospaced))
                        .frame(width: 210)
                }
                settingToggleRow(
                    title: "ACP 续聊",
                    hint: "开启后将在后台自动下载安装适配器（首次约 600 MB）；未就绪时自动回退 CLI",
                    isOn: $acpEnabled
                )
                if store.isInstallingAcp {
                    GlassRow(verticalPadding: 8) {
                        ProgressView()
                            .controlSize(.small)
                        let mbPart = store.acpDownloadedMB > 0 ? "（已下载 \(store.acpDownloadedMB) MB）" : ""
                        Text("正在后台下载安装 ACP 适配器\(mbPart)，可随时关闭此窗口…")
                            .font(.system(size: 11.5))
                            .foregroundStyle(.secondary)
                        Spacer()
                    }
                } else if !acpSummaryText.isEmpty {
                    GlassRow(verticalPadding: 8) {
                        Text(acpSummaryText)
                            .font(.system(size: 11.5))
                            .foregroundStyle(.secondary)
                        Spacer()
                        if acpEnabled && hasMissingAcpAdapter {
                            Button("后台安装适配器") {
                                store.startBackgroundAcpInstall(announceStart: true)
                            }
                            .buttonStyle(.glass)
                            .controlSize(.small)
                        }
                    }
                }
            }
        }
    }

    private var macOSIntegrationSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("macOS 系统集成")
                .font(.system(size: 12.5, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 4)

            GlassCard {
                settingToggleRow(
                    title: "在菜单栏显示快捷图标",
                    hint: "随时查看最近派发状态或快速切换主 agent",
                    isOn: Binding(
                        get: { store.showMenuBarIcon },
                        set: { store.showMenuBarIcon = $0 }
                    ),
                    showTopDivider: false
                )

                GlassRow(verticalPadding: 10) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("命令行 aicross 链接")
                            .font(.system(size: 13.5, weight: .medium))
                        Text("复制软链接命令，在终端里直接使用随附引擎")
                            .font(.system(size: 11.5))
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button("复制安装命令") {
                        let enginePath = EngineClient.shared.engineExecutableURL()?.path ?? "/Applications/aicross.app/Contents/Resources/aicross-engine"
                        store.copyText("sudo ln -sf \"\(enginePath)\" /usr/local/bin/aicross")
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                }
            }
        }
    }

    private func settingSegmentedRow(
        title: String,
        hint: String,
        choices: [Int],
        selection: Binding<Int>,
        showTopDivider: Bool = true
    ) -> some View {
        GlassRow(showTopDivider: showTopDivider, verticalPadding: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 13.5, weight: .medium))
                Text(hint)
                    .font(.system(size: 11.5))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Picker(title, selection: selection) {
                ForEach(choices, id: \.self) { c in
                    Text("\(c)").tag(c)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: CGFloat(choices.count * 38))
        }
    }

    private func settingNumberRow(
        title: String,
        hint: String,
        unit: String,
        text: Binding<String>,
        showTopDivider: Bool = true
    ) -> some View {
        GlassRow(showTopDivider: showTopDivider, verticalPadding: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 13.5, weight: .medium))
                Text(hint)
                    .font(.system(size: 11.5))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            HStack(spacing: 6) {
                TextField("", text: text)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 12.5, design: .monospaced))
                    .multilineTextAlignment(.trailing)
                    .frame(width: 72)
                Text(unit)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func settingToggleRow(
        title: String,
        hint: String,
        isOn: Binding<Bool>,
        showTopDivider: Bool = true
    ) -> some View {
        GlassRow(showTopDivider: showTopDivider, verticalPadding: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 13.5, weight: .medium))
                Text(hint)
                    .font(.system(size: 11.5))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Toggle("", isOn: isOn)
                .toggleStyle(.switch)
                .labelsHidden()
        }
    }

    private func formatMinutes(_ secs: Int) -> String {
        let mins = Double(secs) / 60.0
        if abs(mins.rounded() - mins) < 0.0001 {
            return "\(Int(mins.rounded()))"
        }
        return String(format: "%.2f", mins)
    }

    private func applyLoaded(_ view: DispatchSettingsView) {
        loadedView = view
        let d = view.dispatch
        concurrency = d.concurrency
        consoleRunLimit = d.consoleRunLimit
        maxRequestsText = "\(d.maxRequests)"
        fixRoundLimit = d.fixRoundLimit
        swarmLimit = d.swarmLimit
        timeoutMinutesText = formatMinutes(d.timeoutSecs)
        execTimeoutMinutesText = formatMinutes(d.execTimeoutSecs)
        idleTimeoutMinutesText = formatMinutes(d.idleTimeoutSecs)
        redact = d.redact
        writeProjectDispatch = d.writeProjectDispatch
        validationCommand = d.validationCommand
        acpEnabled = d.acpEnabled
    }

    private func loadSettings() async {
        // 若内存中已有当前配置，立即零延迟渲染，随后静默补齐文件路径与适配器状态
        if loadedView == nil, let cachedDispatch = store.snapshot?.config.dispatch {
            let home = FileManager.default.homeDirectoryForCurrentUser.path
            let initialView = DispatchSettingsView(
                dispatch: cachedDispatch,
                path: "\(home)/.aicross/dispatch-settings.json",
                acp: store.acpReports.isEmpty ? nil : store.acpReports
            )
            applyLoaded(initialView)
        }

        do {
            let v = try await store.fetchDispatchSettings()
            if (computedPatchResult.patch?.count ?? 0) == 0 {
                applyLoaded(v)
            } else {
                loadedView = DispatchSettingsView(
                    dispatch: loadedView?.dispatch ?? v.dispatch,
                    path: v.path,
                    acp: v.acp
                )
            }
        } catch {
            if loadedView == nil {
                loadError = error.localizedDescription
            }
        }
    }

    private func saveChanges() async {
        guard let patch = computedPatchResult.patch, !patch.isEmpty else { return }
        isSaving = true
        saveError = ""
        saveNote = ""
        do {
            let res = try await store.saveDispatchSettings(patch: patch)
            if let cur = loadedView {
                applyLoaded(DispatchSettingsView(
                    dispatch: res.dispatch,
                    path: cur.path,
                    acp: res.acp ?? cur.acp
                ))
            }
            if let note = res.note, !note.isEmpty {
                saveNote = note
            } else {
                onClose()
            }
        } catch {
            saveError = error.localizedDescription
        }
        isSaving = false
    }
}

// MARK: - macOS 菜单栏快捷面板

struct MenuBarPopoverView: View {
    @ObservedObject var store: AppStore
    let onOpenMainWindow: () -> Void
    let onOpenRunInMain: (String) -> Void

    private var defaultModeName: String {
        let defId = store.snapshot?.config.defaultMode ?? ""
        return store.snapshot?.config.modes.first(where: { $0.id == defId })?.name ?? "默认"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                if let host = store.currentHost {
                    AgentBrandIcon(agentId: host.id, size: 22)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(host.name)
                            .font(.system(size: 13.5, weight: .semibold))
                        Text("默认模式：\(defaultModeName)")
                            .font(.system(size: 11.5))
                            .foregroundStyle(.secondary)
                    }
                } else {
                    Text("aicross")
                        .font(.system(size: 14, weight: .semibold))
                }
                Spacer()
                Button("打开面板", action: onOpenMainWindow)
                    .buttonStyle(.glassProminent)
                    .tint(LGTheme.emerald)
                    .controlSize(.small)
            }

            Divider()

            Text("最近派发")
                .font(.system(size: 11.5, weight: .semibold))
                .foregroundStyle(.secondary)

            if store.runs.isEmpty {
                Text("暂无派发记录")
                    .font(.system(size: 12.5))
                    .foregroundStyle(.tertiary)
                    .padding(.vertical, 8)
            } else {
                VStack(spacing: 4) {
                    ForEach(store.runs.prefix(5)) { run in
                        Button {
                            onOpenRunInMain(run.runId)
                        } label: {
                            HStack(spacing: 8) {
                                Circle()
                                    .fill(dotColor(for: run.status))
                                    .frame(width: 7, height: 7)
                                Text(run.presetName)
                                    .font(.system(size: 11.5, weight: .medium))
                                    .foregroundStyle(.secondary)
                                Text(run.taskTitle.isEmpty ? run.runId : run.taskTitle)
                                    .font(.system(size: 12.5))
                                    .foregroundStyle(.primary)
                                    .lineLimit(1)
                                Spacer()
                            }
                            .padding(.vertical, 5)
                            .padding(.horizontal, 6)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            Divider()

            HStack {
                Button {
                    Task { await store.reloadAll(showToast: false) }
                } label: {
                    Label("刷新", systemImage: "arrow.clockwise")
                        .font(.system(size: 12))
                }
                .buttonStyle(.glass)
                .controlSize(.small)

                Spacer()

                Button("退出 aicross") {
                    NSApplication.shared.terminate(nil)
                }
                .buttonStyle(.plain)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
            }
        }
        .padding(14)
        .frame(width: 320)
    }

    private func dotColor(for status: String) -> Color {
        switch status {
        case "running": return LGTheme.activeBlue
        case "done", "ok": return LGTheme.emerald
        case "failed", "error": return LGTheme.dangerRed
        default: return .secondary
        }
    }
}
