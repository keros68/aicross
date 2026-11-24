import SwiftUI
import AppKit

struct SkillPageView: View {
    @ObservedObject var store: AppStore

    @State private var confirmUpgradeLoc: SkillLocation? = nil
    @State private var confirmUninstallLoc: SkillLocation? = nil
    @State private var confirmDeleteSlash: SlashCommand? = nil
    @State private var slashDraft: SlashFormDraft? = nil
    @State private var busySlashAction: String = ""

    private struct SlashFormDraft: Equatable {
        var editingName: String // 非空代表编辑已有命令（名称不可改）
        var name: String
        var modeId: String
        var note: String
    }

    private var resources: ResourcesOutput? { store.snapshot?.resources }
    private var modes: [ModeEntry] { store.snapshot?.config.modes ?? [] }
    private var bundledVersion: String { store.skillStatus?.bundledVersion ?? "" }
    private var hostSkill: SkillHostStatus? { store.currentSkillHost }
    private var locations: [SkillLocation] { hostSkill?.locations ?? [] }

    private var activeLocations: [SkillLocation] {
        locations.filter { $0.kind != "none" && $0.readByThisHost != false }
    }

    private var hasGlobalOurs: Bool {
        locations.contains { $0.scope == "global" && $0.kind == "ours" }
    }

    private var upgradableExternal: SkillLocation? {
        locations.first {
            $0.kind == "external" && $0.readByThisHost != false && ($0.manager ?? "") != "skills-manager"
        }
    }

    private var skillSummaryState: (text: String, isAlert: Bool) {
        guard hostSkill != nil else { return ("—", false) }
        if activeLocations.isEmpty { return ("未安装", true) }
        if hostSkill?.updateAvailable == true || activeLocations.contains(where: { $0.outdated == true }) {
            return ("可更新", false)
        }
        return ("已安装", false)
    }

    private var overviewItems: [StatCellItem] {
        let summary = skillSummaryState
        return [
            StatCellItem(label: "随附版本", value: bundledVersion.isEmpty ? "—" : bundledVersion),
            StatCellItem(label: "当前状态", value: summary.text, isAlert: summary.isAlert),
            StatCellItem(label: "生效位置", value: "\(activeLocations.count)")
        ]
    }

    var body: some View {
        VStack(spacing: 0) {
            PageToolbarBar {
                if let host = store.currentHost {
                    HostVendorSubbar(store: store, host: host)
                } else {
                    Text("尚未检测到可用的主 agent")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }
            } trailing: {
                if !store.selectedHostId.isEmpty {
                    Button {
                        chooseProjectAndInstall()
                    } label: {
                        Label("安装到项目…", systemImage: "folder.badge.plus")
                    }
                    .buttonStyle(.glass)

                    globalActionControl
                }
            }

            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    if let banner = store.bannerMessage {
                        GlassNoticeBanner(
                            icon: store.bannerIsError ? "exclamationmark.triangle.fill" : "checkmark.circle.fill",
                            message: banner,
                            isAlert: store.bannerIsError,
                            onDismiss: { store.clearBanner() }
                        )
                    }

                    if resources?.needsFirstDetect == true {
                        GlassNoticeBanner(
                            icon: "sparkle.magnifyingglass",
                            message: "尚未探测本机 agent，建议先探测以识别主 agent 与可用模型。",
                            actionTitle: "去探测",
                            action: { store.selectTab(.resources) }
                        )
                    }

                    if hostSkill?.duplicate == true {
                        GlassNoticeBanner(
                            icon: "exclamationmark.circle.fill",
                            message: "当前主 agent 存在多份 ai-cross skill，可能互相覆盖，建议卸载多余副本。",
                            isAlert: false
                        )
                    }

                    GlassStatStrip(items: overviewItems)

                    locationsCard

                    if store.slashList?.supported != false, store.currentHost != nil {
                        slashCommandsCard
                    }
                }
                .padding(.horizontal, 28)
                .padding(.top, 8)
                .padding(.bottom, 32)
                .frame(maxWidth: 940)
                .frame(maxWidth: .infinity)
            }
        }
        .alert(
            "升级旧版 skill",
            isPresented: Binding(
                get: { confirmUpgradeLoc != nil },
                set: { if !$0 { confirmUpgradeLoc = nil } }
            )
        ) {
            Button("备份并升级") {
                if let loc = confirmUpgradeLoc {
                    confirmUpgradeLoc = nil
                    Task { await store.upgradeExternalSkill(path: loc.path) }
                }
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("该位置的 skill（\(confirmUpgradeLoc?.version ?? "旧版")）由其他方式安装。升级将替换为随附版本 \(bundledVersion)，原目录自动备份到 .aicross-skill-backup。")
        }
        .alert(
            "卸载 ai-cross skill",
            isPresented: Binding(
                get: { confirmUninstallLoc != nil },
                set: { if !$0 { confirmUninstallLoc = nil } }
            )
        ) {
            Button("卸载", role: .destructive) {
                if let loc = confirmUninstallLoc {
                    confirmUninstallLoc = nil
                    Task { await store.uninstallSkill(location: loc) }
                }
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("将从 \(confirmUninstallLoc?.path ?? "") 移除 ai-cross skill。")
        }
        .alert(
            "删除斜杠命令",
            isPresented: Binding(
                get: { confirmDeleteSlash != nil },
                set: { if !$0 { confirmDeleteSlash = nil } }
            )
        ) {
            Button("删除", role: .destructive) {
                if let cmd = confirmDeleteSlash {
                    confirmDeleteSlash = nil
                    Task {
                        busySlashAction = cmd.name
                        await store.removeSlashCommand(name: cmd.name)
                        if slashDraft?.editingName == cmd.name { slashDraft = nil }
                        busySlashAction = ""
                    }
                }
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("确定删除短命令 \(confirmDeleteSlash?.trigger ?? "") 吗？")
        }
    }

    // MARK: - 右上角全局安装控件

    @ViewBuilder
    private var globalActionControl: some View {
        if hostSkill?.updateAvailable == true {
            Button {
                Task { await store.installSkill(scope: "global") }
            } label: {
                Label("更新全局", systemImage: "arrow.triangle.2.circlepath")
            }
            .buttonStyle(.glassProminent)
            .tint(LGTheme.emerald)
        } else if let ext = upgradableExternal, !hasGlobalOurs {
            Button {
                confirmUpgradeLoc = ext
            } label: {
                Label("升级旧版", systemImage: "arrow.up.circle.fill")
            }
            .buttonStyle(.glassProminent)
            .tint(LGTheme.emerald)
        } else if hasGlobalOurs {
            GlassPill(text: "全局已安装", tone: .ok)
        } else {
            Button {
                Task { await store.installSkill(scope: "global") }
            } label: {
                Label("安装全局", systemImage: "square.and.arrow.down")
            }
            .buttonStyle(.glassProminent)
            .tint(LGTheme.emerald)
        }
    }

    // MARK: - Skill 读取位置卡片

    private var locationsCard: some View {
        let visibleLocs = locations.filter { $0.kind != "none" || $0.writeTarget == true || $0.scope == "global" }

        return GlassCard {
            GlassRow(showTopDivider: false, verticalPadding: 14) {
                Label("安装位置", systemImage: "puzzlepiece.extension")
                    .font(.system(size: 15, weight: .semibold))
                Spacer()
                if !store.selectedProjectForSkill.isEmpty {
                    Text("当前查看项目：\(URL(fileURLWithPath: store.selectedProjectForSkill).lastPathComponent)")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
            }

            if visibleLocs.isEmpty {
                GlassRow {
                    Text(store.hosts.isEmpty ? "请先探测本机主 agent。" : "当前主 agent 尚未安装 ai-cross skill，点击右上角「安装全局」即可启用。")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                    Spacer()
                }
            } else {
                ForEach(visibleLocs) { loc in
                    locationRow(loc)
                }
            }
        }
    }

    private func locationRow(_ loc: SkillLocation) -> some View {
        GlassRow {
            GlassPill(
                text: loc.scope == "project" ? "项目" : "全局",
                tone: loc.scope == "project" ? .active : .muted
            )

            VStack(alignment: .leading, spacing: 3) {
                Text(loc.path)
                    .font(.system(size: 12.5, design: .monospaced))
                    .foregroundStyle(loc.kind == "none" ? .secondary : .primary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .textSelection(.enabled)

                if let target = loc.linkTarget, !target.isEmpty {
                    Text("符号链接指向：\(target)")
                        .font(.system(size: 11.5))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                } else if loc.readByThisHost == false {
                    Text("当前主 agent 不读取该位置")
                        .font(.system(size: 11.5))
                        .foregroundStyle(.secondary)
                }
            }

            Spacer(minLength: 12)

            if loc.kind == "none" {
                GlassPill(text: "未安装", tone: .muted)
            } else if loc.kind == "external" {
                GlassPill(
                    text: loc.version.isEmpty ? "外部安装" : "外部 · \(loc.version)",
                    tone: loc.outdated == true ? .warn : .muted
                )
                if (loc.manager ?? "") != "skills-manager" {
                    Button("升级") {
                        confirmUpgradeLoc = loc
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                }
            } else {
                GlassPill(
                    text: loc.version.isEmpty ? "已安装" : loc.version,
                    tone: loc.outdated == true ? .warn : .ok,
                    monospaced: true
                )
            }

            if loc.kind != "none" {
                Button {
                    NSWorkspace.shared.selectFile(nil, inFileViewerRootedAtPath: loc.path)
                } label: {
                    Image(systemName: "folder")
                        .font(.system(size: 12))
                }
                .buttonStyle(.glass)
                .controlSize(.small)
                .help("在访达中打开目录")
            }

            if loc.kind == "ours" {
                Button {
                    confirmUninstallLoc = loc
                } label: {
                    Image(systemName: "trash")
                        .font(.system(size: 12))
                        .foregroundStyle(LGTheme.dangerRed)
                }
                .buttonStyle(.glass)
                .controlSize(.small)
                .help("卸载")
            }
        }
    }

    // MARK: - 斜杠短命令卡片（镜像 SlashCard.tsx）

    private var slashModeRows: [SlashModeEntry] {
        store.slashList?.modes ?? []
    }

    private var otherSlashCommands: [SlashCommand] {
        let shownNames = Set(slashModeRows.compactMap { $0.command?.name })
        return (store.slashList?.commands ?? []).filter { !shownNames.contains($0.name) }
    }

    private var staleSlashCommands: [SlashCommand] {
        (store.slashList?.commands ?? []).filter { $0.stale == true }
    }

    private var canGenerateAllSlash: Bool {
        !slashModeRows.isEmpty && slashModeRows.contains { $0.command == nil }
    }

    private var slashCommandsCard: some View {
        let hostId = store.selectedHostId
        let invokeStr = DisplayHelpers.invokePrefix(hostId: hostId, fromSlash: store.slashList?.invoke)

        return GlassCard {
            GlassRow(showTopDivider: false, verticalPadding: 14) {
                Label("斜杠命令", systemImage: "terminal")
                    .font(.system(size: 15, weight: .semibold))

                Spacer()

                if !slashModeRows.isEmpty {
                    Button {
                        Task {
                            busySlashAction = "*"
                            await store.generateAllModeSlashCommands(prefix: "ax-")
                            busySlashAction = ""
                        }
                    } label: {
                        Text(busySlashAction == "*" ? "生成中…" : "全部生成")
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                    .disabled(!canGenerateAllSlash || !busySlashAction.isEmpty || slashDraft != nil)
                }

                if !staleSlashCommands.isEmpty {
                    Button {
                        Task {
                            busySlashAction = "update:*"
                            for cmd in staleSlashCommands {
                                _ = await store.addSlashCommand(name: cmd.name, modeId: cmd.mode, note: cmd.note)
                            }
                            busySlashAction = ""
                        }
                    } label: {
                        Text(busySlashAction == "update:*" ? "更新中…" : "全部更新")
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                    .disabled(!busySlashAction.isEmpty || slashDraft != nil)
                }

                Button {
                    slashDraft = SlashFormDraft(
                        editingName: "",
                        name: "",
                        modeId: modes.first?.id ?? "",
                        note: ""
                    )
                } label: {
                    Label("添加", systemImage: "plus")
                }
                .buttonStyle(.glass)
                .controlSize(.small)
                .disabled(slashDraft != nil || modes.isEmpty)
            }

            if activeLocations.isEmpty {
                GlassRow {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(LGTheme.warnAmber)
                    Text("\(store.currentHost?.name ?? hostId) 尚未安装 ai-cross skill，先安装后短命令才能生效。")
                        .font(.system(size: 12.5))
                        .foregroundStyle(.secondary)
                    Spacer()
                }
            }

            // 默认唤起行
            GlassRow {
                Text("默认")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.secondary)
                    .frame(width: 72, alignment: .leading)

                Text(invokeStr)
                    .font(.system(size: 13, weight: .medium, design: .monospaced))
                    .textSelection(.enabled)

                Spacer()

                Button("复制") {
                    store.copyText(invokeStr)
                }
                .buttonStyle(.glass)
                .controlSize(.small)
            }

            // 内置模式逐行
            ForEach(slashModeRows) { row in
                let kind = ModeKind.from(id: row.mode, name: row.modeName)
                GlassRow {
                    HStack(spacing: 8) {
                        ModeIconBadge(kind: kind, size: 22)
                        Text(row.modeName.isEmpty ? row.mode : row.modeName)
                            .font(.system(size: 13, weight: .medium))
                    }
                    .frame(width: 118, alignment: .leading)
                    .help(row.description ?? "")

                    HStack(spacing: 6) {
                        Text(row.argForm)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                        Button {
                            store.copyText(row.argForm)
                        } label: {
                            Image(systemName: "doc.on.doc")
                                .font(.system(size: 11))
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(.secondary)
                        .help("复制带参数写法")
                    }

                    Spacer(minLength: 10)

                    if let cmd = row.command {
                        Text(cmd.trigger)
                            .font(.system(size: 12.5, weight: .semibold, design: .monospaced))
                            .help(cmd.path)
                        slashCommandControls(cmd)
                    } else {
                        let preview = "\(triggerPrefix(for: hostId))\(row.suggestedName)"
                        Text(preview)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(.tertiary)

                        Button(busySlashAction == row.mode ? "生成中…" : "生成") {
                            Task {
                                busySlashAction = row.mode
                                _ = await store.addSlashCommand(name: row.suggestedName, modeId: row.mode, note: "")
                                busySlashAction = ""
                            }
                        }
                        .buttonStyle(.glass)
                        .controlSize(.small)
                        .disabled(!busySlashAction.isEmpty || slashDraft != nil)
                    }
                }
            }

            // 自定义/其它短命令小节
            if !otherSlashCommands.isEmpty {
                GlassRow(verticalPadding: 8) {
                    Text("其它短命令")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(.secondary)
                    Spacer()
                }
                ForEach(otherSlashCommands) { cmd in
                    let kind = ModeKind.from(id: cmd.mode, name: cmd.modeName)
                    GlassRow {
                        Text(cmd.trigger)
                            .font(.system(size: 13, weight: .semibold, design: .monospaced))
                            .help(cmd.path)

                        HStack(spacing: 6) {
                            ModeIconBadge(kind: kind, size: 20)
                            Text(cmd.modeName.isEmpty ? cmd.mode : cmd.modeName)
                                .font(.system(size: 12.5))
                                .foregroundStyle(.secondary)
                        }

                        if !cmd.note.isEmpty {
                            Text(cmd.note)
                                .font(.system(size: 12))
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }

                        Spacer()
                        slashCommandControls(cmd)
                    }
                }
            }

            // 新增 / 编辑表单
            if let draft = slashDraft {
                slashEditorRow(draft: draft, hostId: hostId)
            }
        }
    }

    @ViewBuilder
    private func slashCommandControls(_ cmd: SlashCommand) -> some View {
        let invalid = cmd.state == "名称无效"
        let tone: PillTone = cmd.state == "已写入" ? .ok : ((cmd.state == "未写入" || cmd.state == "被占用" || invalid) ? .alert : .muted)

        GlassPill(text: cmd.state, tone: tone)

        if cmd.stale == true && !invalid {
            GlassPill(text: "待更新", tone: .warn)
            Button(busySlashAction == "update:\(cmd.name)" ? "更新中…" : "更新") {
                Task {
                    busySlashAction = "update:\(cmd.name)"
                    _ = await store.addSlashCommand(name: cmd.name, modeId: cmd.mode, note: cmd.note)
                    busySlashAction = ""
                }
            }
            .buttonStyle(.glass)
            .controlSize(.small)
            .disabled(!busySlashAction.isEmpty || slashDraft != nil)
        }

        if !invalid {
            Button {
                store.copyText(cmd.trigger)
            } label: {
                Image(systemName: "doc.on.doc")
                    .font(.system(size: 11.5))
            }
            .buttonStyle(.glass)
            .controlSize(.small)
            .help("复制触发命令")

            Button {
                let validMode = modes.contains(where: { $0.id == cmd.mode }) ? cmd.mode : (modes.first?.id ?? "")
                slashDraft = SlashFormDraft(editingName: cmd.name, name: cmd.name, modeId: validMode, note: cmd.note)
            } label: {
                Image(systemName: "pencil")
                    .font(.system(size: 11.5))
            }
            .buttonStyle(.glass)
            .controlSize(.small)
            .disabled(!busySlashAction.isEmpty || slashDraft != nil)
            .help("编辑")
        }

        Button {
            confirmDeleteSlash = cmd
        } label: {
            Image(systemName: "trash")
                .font(.system(size: 11.5))
                .foregroundStyle(LGTheme.dangerRed)
        }
        .buttonStyle(.glass)
        .controlSize(.small)
        .disabled(!busySlashAction.isEmpty || slashDraft != nil)
        .help("删除")
    }

    private func slashEditorRow(draft: SlashFormDraft, hostId: String) -> some View {
        let err = validateSlashName(draft.name)
        return GlassRow(verticalPadding: 14) {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 10) {
                    HStack(spacing: 3) {
                        Text(triggerPrefix(for: hostId))
                            .font(.system(size: 12.5, design: .monospaced))
                            .foregroundStyle(.secondary)
                        TextField("短命令名（如 ax-review）", text: Binding(
                            get: { slashDraft?.name ?? "" },
                            set: { slashDraft?.name = $0.trimmingCharacters(in: .whitespaces).lowercased() }
                        ))
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: 12.5, design: .monospaced))
                        .frame(width: 150)
                        .disabled(!draft.editingName.isEmpty)
                    }

                    Picker("模式", selection: Binding(
                        get: { slashDraft?.modeId ?? "" },
                        set: { slashDraft?.modeId = $0 }
                    )) {
                        ForEach(modes) { m in
                            Text(m.name).tag(m.id)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 140)

                    TextField("附加说明（可选）", text: Binding(
                        get: { slashDraft?.note ?? "" },
                        set: { slashDraft?.note = String($0.prefix(200)) }
                    ))
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 12.5))

                    Button("保存") {
                        guard let cur = slashDraft else { return }
                        Task {
                            busySlashAction = "saving"
                            let cleanNote = cur.note.replacingOccurrences(of: "\n", with: " ").trimmingCharacters(in: .whitespacesAndNewlines)
                            let ok = await store.addSlashCommand(name: cur.name, modeId: cur.modeId, note: cleanNote)
                            busySlashAction = ""
                            if ok { slashDraft = nil }
                        }
                    }
                    .buttonStyle(.glassProminent)
                    .tint(LGTheme.emerald)
                    .controlSize(.small)
                    .disabled(!err.isEmpty || draft.modeId.isEmpty || !busySlashAction.isEmpty)

                    Button("取消") {
                        slashDraft = nil
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                }

                if !draft.name.isEmpty && !err.isEmpty {
                    Text(err)
                        .font(.system(size: 12))
                        .foregroundStyle(LGTheme.dangerRed)
                }
            }
        }
    }

    // MARK: - 辅助纯函数

    private func triggerPrefix(for host: String) -> String {
        switch host {
        case "codex": return "/prompts:"
        case "kimi": return "/skill:"
        default: return "/"
        }
    }

    private func validateSlashName(_ raw: String) -> String {
        let s = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !s.isEmpty, s.count <= 32 else { return "请输入 1–32 位小写字母、数字或 -" }
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyz0123456789-")
        if s.unicodeScalars.contains(where: { !allowed.contains($0) }) {
            return "只能用小写字母、数字和 -"
        }
        if s == "ai-cross" { return "ai-cross 为保留名" }
        return ""
    }

    private func chooseProjectAndInstall() {
        let panel = NSOpenPanel()
        panel.title = "选择要安装 ai-cross skill 的项目目录"
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.prompt = "安装到此项目"
        if panel.runModal() == .OK, let url = panel.url {
            Task { await store.installSkill(scope: "project", projectDir: url.path) }
        }
    }
}
