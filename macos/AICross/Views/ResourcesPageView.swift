import SwiftUI
import AppKit

struct ResourcesPageView: View {
    @ObservedObject var store: AppStore

    @State private var filterUnavailable: Bool = false
    @State private var showAllModelsForAgent: Set<String> = []
    @State private var showNotInstalled: Bool = false
    @State private var confirmPruneAgent: (agent: String?, count: Int)? = nil
    @State private var editingNoteTarget: NoteEditDraft? = nil

    private struct NoteEditDraft: Identifiable {
        var id: String { modelKey }
        var modelKey: String
        var displayTitle: String
        var tags: Set<String>
        var note: String
    }

    private var resources: ResourcesOutput? { store.snapshot?.resources }
    private var allAgents: [ResourceAgentEntry] { resources?.agents ?? [] }
    private var listedAgents: [ResourceAgentEntry] { allAgents.filter(\.isListedInMain) }
    private var notInstalledAgents: [ResourceAgentEntry] { allAgents.filter { !$0.isListedInMain } }

    private var missingTotalCount: Int {
        allAgents.flatMap(\.models).filter(\.isMissingModel).count
    }

    private var unavailableTotalCount: Int {
        listedAgents.flatMap(\.models).filter(\.isUnavailable).count
    }

    private var overviewItems: [StatCellItem] {
        let installedCount = listedAgents.filter(\.isConfirmedInstalled).count
        let usableCount = DisplayHelpers.buildSeatOptions(from: resources).filter { !$0.disabled }.count
        let verifiedCount = listedAgents.flatMap(\.models).filter { $0.state == "已验证" }.count
        return [
            StatCellItem(label: "已装 agent", value: "\(installedCount)"),
            StatCellItem(label: "可用模型", value: "\(usableCount)"),
            StatCellItem(label: "已验证", value: "\(verifiedCount)"),
            StatCellItem(
                label: "不可用",
                value: "\(unavailableTotalCount)",
                isAlert: unavailableTotalCount > 0,
                filterTag: unavailableTotalCount > 0 ? "unavailable" : nil
            )
        ]
    }

    private var detectLabel: String {
        if store.isDetecting {
            if let p = store.detectProgress, p.total > 0 {
                return "探测中 \(p.done)/\(p.total)"
            }
            return "探测中…"
        }
        return (resources?.needsFirstDetect == false) ? "重新探测" : "探测"
    }

    var body: some View {
        VStack(spacing: 0) {
            PageToolbarBar {
                if let detectedAt = resources?.detected_at, !detectedAt.isEmpty {
                    Text("上次探测：\(DisplayHelpers.formatTimestamp(detectedAt))")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }

                if unavailableTotalCount > 0 {
                    Picker("筛选", selection: $filterUnavailable) {
                        Text("全部").tag(false)
                        Text("不可用").tag(true)
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .frame(width: 140)
                }
            } trailing: {
                if missingTotalCount > 0 {
                    Button {
                        confirmPruneAgent = (nil, missingTotalCount)
                    } label: {
                        Label("清理未发现（\(missingTotalCount)）", systemImage: "trash")
                    }
                    .buttonStyle(.glass)
                    .disabled(store.isDetecting)
                }

                Button {
                    Task { await store.detectResources() }
                } label: {
                    Label(detectLabel, systemImage: "arrow.clockwise")
                }
                .buttonStyle(.glassProminent)
                .tint(LGTheme.emerald)
                .disabled(store.isDetecting)
            }

            ScrollViewReader { proxy in
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

                        if resources?.needsFirstDetect == true && !store.isDetecting {
                            GlassNoticeBanner(
                                icon: "sparkle.magnifyingglass",
                                message: "尚未探测本机 agent 与模型，点击右侧按钮开始扫描。",
                                actionTitle: "探测",
                                action: { Task { await store.detectResources() } }
                            )
                        }

                        if resources != nil {
                            GlassStatStrip(
                                items: overviewItems,
                                activeFilter: (filterUnavailable && unavailableTotalCount > 0) ? "unavailable" : nil,
                                onSelectFilter: { next in
                                    filterUnavailable = (next == "unavailable")
                                }
                            )
                        }

                        ForEach(listedAgents) { agent in
                            agentCard(agent)
                                .id(agent.id)
                        }

                        if !notInstalledAgents.isEmpty && !(filterUnavailable && unavailableTotalCount > 0) {
                            notInstalledSection
                        }
                    }
                    .padding(.horizontal, 28)
                    .padding(.bottom, 32)
                    .frame(maxWidth: 960)
                    .frame(maxWidth: .infinity)
                }
                .onChange(of: store.focusResourceAgentId) { _, target in
                    guard let target else { return }
                    withAnimation(.easeInOut(duration: 0.25)) {
                        proxy.scrollTo(target, anchor: .top)
                    }
                    store.focusResourceAgentId = nil
                }
            }
        }
        .alert(
            "清理未发现的模型",
            isPresented: Binding(
                get: { confirmPruneAgent != nil },
                set: { if !$0 { confirmPruneAgent = nil } }
            )
        ) {
            Button("清理", role: .destructive) {
                let target = confirmPruneAgent?.agent
                confirmPruneAgent = nil
                Task { await store.pruneMissingModels(agent: target) }
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("将从资源库移除最近一次探测未发现的 \(confirmPruneAgent?.count ?? 0) 个旧模型条目。")
        }
        .sheet(item: $editingNoteTarget) { draft in
            ModelNoteEditorSheet(
                title: draft.displayTitle,
                kindOptions: store.modelTrack?.kindList ?? [],
                initialTags: draft.tags,
                initialNote: draft.note,
                onSave: { tags, note in
                    editingNoteTarget = nil
                    Task { await store.saveModelNote(modelKey: draft.modelKey, tags: Array(tags), note: note) }
                },
                onCancel: { editingNoteTarget = nil }
            )
        }
    }

    // MARK: - 单个 Agent 可折叠卡片

    @ViewBuilder
    private func agentCard(_ agent: ResourceAgentEntry) -> some View {
        let isFilteringBad = filterUnavailable && unavailableTotalCount > 0
        let badModels = agent.models.filter(\.isUnavailable)

        if isFilteringBad && badModels.isEmpty {
            EmptyView()
        } else {
            let isOpen = isFilteringBad ? true : (agent.isExpandable && store.isAgentExpanded(agent))
            let listedModels = agent.models.filter { !$0.isMissingModel }
            let missingModels = agent.models.filter(\.isMissingModel)
            let showAll = showAllModelsForAgent.contains(agent.id)
            let visibleModels = isFilteringBad
                ? badModels
                : (showAll ? listedModels : Array(listedModels.prefix(8)))
            let hiddenCount = max(0, listedModels.count - 8)
            let providerGroups = groupModelsByProvider(visibleModels)
            let hasMultipleProviders = providerGroups.count > 1 || !(providerGroups.first?.provider.isEmpty ?? true)
            let smokeTargets = agent.batchSmokeTargets
            let batchRunning = (store.batchSmokingAgent?.agent == agent.id) ? store.batchSmokingAgent : nil
            let pendingDetect = store.isDetecting && !store.completedDetectAgents.contains(agent.id)

            GlassCard {
                // 卡片头：点击整行展开 / 收起
                GlassRow(showTopDivider: false, verticalPadding: 14) {
                    Button {
                        guard agent.isExpandable else { return }
                        withAnimation(.spring(response: 0.26, dampingFraction: 0.85)) {
                            store.toggleAgentExpanded(agent.id, defaultOpen: store.isAgentExpanded(agent))
                        }
                    } label: {
                        HStack(spacing: 10) {
                            AgentBrandIcon(agentId: agent.id, size: 26)
                            Text(agent.name.isEmpty ? agent.id : agent.name)
                                .font(.system(size: 15.5, weight: .semibold))
                                .foregroundStyle(.primary)

                            if agent.untested == true {
                                GlassPill(text: "未实测", tone: .muted)
                            }
                            if !agent.version.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                GlassPill(text: agent.version, tone: .muted, monospaced: true)
                            }
                            if pendingDetect {
                                ProgressView()
                                    .controlSize(.small)
                            }

                            Spacer(minLength: 8)

                            agentStatsText(agent)

                            if agent.isExpandable && !isFilteringBad {
                                Image(systemName: isOpen ? "chevron.up" : "chevron.down")
                                    .font(.system(size: 12, weight: .semibold))
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)

                    if !smokeTargets.isEmpty || batchRunning != nil {
                        if let running = batchRunning {
                            HStack(spacing: 6) {
                                Text("测试中 \(running.done)/\(running.total)")
                                    .font(.system(size: 12, design: .monospaced))
                                    .foregroundStyle(.secondary)
                                Button("取消") {
                                    store.cancelBatchSmoke()
                                }
                                .buttonStyle(.glass)
                                .controlSize(.small)
                            }
                        } else {
                            Button {
                                Task { await store.smokeAllForAgent(agent) }
                            } label: {
                                Label("全部测试（\(smokeTargets.count)）", systemImage: "play.fill")
                            }
                            .buttonStyle(.glass)
                            .controlSize(.small)
                            .disabled(store.isDetecting || store.batchSmokingAgent != nil)
                        }
                    }
                }

                if isOpen {
                    if agent.state != "可用" || !agent.note.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        GlassRow {
                            if agent.state != "可用" {
                                GlassPill(text: agent.state, tone: agent.state == "未探测" ? .muted : .alert)
                            }
                            if !agent.note.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                Text(agent.note)
                                    .font(.system(size: 12.5))
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            if agent.state == "未安装" {
                                Button("指定路径") {
                                    pickExecutableForAgent(agent.id)
                                }
                                .buttonStyle(.glass)
                                .controlSize(.small)
                            }
                        }
                    }

                    if let customPath = agent.custom_path, !customPath.isEmpty {
                        GlassRow {
                            Text("指定路径")
                                .font(.system(size: 12.5))
                                .foregroundStyle(.secondary)
                            Text(customPath)
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Spacer()
                            Button("修改") { pickExecutableForAgent(agent.id) }
                                .buttonStyle(.glass)
                                .controlSize(.small)
                            Button("清除") {
                                Task { await store.setAgentCustomPath(agentId: agent.id, path: nil) }
                            }
                            .buttonStyle(.glass)
                            .controlSize(.small)
                        }
                    }

                    if hasMultipleProviders {
                        ForEach(providerGroups, id: \.provider) { group in
                            GlassRow(verticalPadding: 7) {
                                Text(group.provider.isEmpty ? "原生" : group.provider)
                                    .font(.system(size: 11.5, weight: .semibold, design: .monospaced))
                                    .foregroundStyle(.secondary)
                                Spacer()
                            }
                            ForEach(group.models, id: \.uniqueKey) { model in
                                modelRow(model, showProvider: false)
                            }
                        }
                    } else {
                        ForEach(visibleModels, id: \.uniqueKey) { model in
                            modelRow(model, showProvider: true)
                        }
                    }

                    if !isFilteringBad && listedModels.count > 8 {
                        Divider().opacity(0.5)
                        Button {
                            withAnimation(.easeInOut(duration: 0.2)) {
                                if showAll {
                                    showAllModelsForAgent.remove(agent.id)
                                } else {
                                    showAllModelsForAgent.insert(agent.id)
                                }
                            }
                        } label: {
                            HStack(spacing: 6) {
                                Text(showAll ? "收起其余模型" : "展开其余 \(hiddenCount) 个模型")
                                Image(systemName: showAll ? "chevron.up" : "chevron.down")
                                    .font(.system(size: 11, weight: .semibold))
                            }
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 11)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }

                    if !isFilteringBad && !missingModels.isEmpty {
                        GlassRow {
                            Text("最近一次探测未发现 \(missingModels.count) 个旧模型")
                                .font(.system(size: 12.5))
                                .foregroundStyle(.secondary)
                            Spacer()
                            Button("清理") {
                                confirmPruneAgent = (agent.id, missingModels.count)
                            }
                            .buttonStyle(.glass)
                            .controlSize(.small)
                        }
                    }
                }
            }
        }
    }

    private func modelRow(_ model: ResourceModelEntry, showProvider: Bool) -> some View {
        let isSmoking = store.smokingKeys.contains(model.uniqueKey)
        let trackKey = "\(model.agent):\(model.provider):\(model.id)"
        let tracked = store.modelTrack?.models.first(where: { $0.model == trackKey })

        return GlassRow(verticalPadding: 10) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Text(model.id)
                        .font(.system(size: 13, weight: .medium, design: .monospaced))
                        .textSelection(.enabled)

                    if showProvider && !model.provider.isEmpty {
                        Text(model.provider)
                            .font(.system(size: 11.5, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }

                    if !model.resolvedDisplay.isEmpty {
                        Text("真身：\(model.resolvedDisplay)")
                            .font(.system(size: 11.5, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }

                    Spacer()

                    modelStateView(model)

                    if model.canSmoke {
                        Button {
                            Task { await store.smokeSingleModel(model) }
                        } label: {
                            if isSmoking {
                                ProgressView().controlSize(.mini)
                            } else {
                                Label("测试连接", systemImage: "play.fill")
                                    .labelStyle(.titleAndIcon)
                            }
                        }
                        .buttonStyle(.glass)
                        .controlSize(.small)
                        .disabled(isSmoking || store.isDetecting)
                    }

                    if store.modelTrack != nil {
                        Button {
                            editingNoteTarget = NoteEditDraft(
                                modelKey: trackKey,
                                displayTitle: model.id,
                                tags: Set(tracked?.tags ?? []),
                                note: tracked?.note ?? ""
                            )
                        } label: {
                            Image(systemName: "tag")
                                .font(.system(size: 12))
                        }
                        .buttonStyle(.glass)
                        .controlSize(.small)
                        .help("标注擅长任务类型与备注")
                    }
                }

                if !model.note.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !model.isMissingModel {
                    Text(model.note)
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                if let tracked, (!tracked.tagNames.isEmpty || !tracked.note.isEmpty) {
                    let prefix = tracked.tagNames.isEmpty ? "" : "擅长：\(tracked.tagNames.joined(separator: "、"))"
                    let sep = (!prefix.isEmpty && !tracked.note.isEmpty) ? " · " : ""
                    Text("\(prefix)\(sep)\(tracked.note)")
                        .font(.system(size: 12))
                        .foregroundStyle(LGTheme.activeBlue)
                        .lineLimit(1)
                }
            }
        }
    }

    @ViewBuilder
    private func modelStateView(_ model: ResourceModelEntry) -> some View {
        let st = model.displayState
        if model.isUnavailable {
            GlassPill(text: st, tone: .alert)
        } else if st == "已验证" || st == "可用" {
            GlassPill(text: st, tone: .ok)
        } else {
            GlassPill(text: st, tone: .muted)
        }
    }

    private func agentStatsText(_ agent: ResourceAgentEntry) -> some View {
        let badCount = agent.models.filter(\.isUnavailable).count
        let verifiedCount = agent.models.filter { $0.state == "已验证" }.count
        return HStack(spacing: 4) {
            Text("\(agent.models.count) 个模型")
            if agent.isNativeAgent {
                Text("· 原生")
            } else if !agent.models.isEmpty {
                Text("· \(verifiedCount) 已验证")
            }
            if badCount > 0 {
                Text("· \(badCount) 不可用")
                    .foregroundStyle(LGTheme.dangerRed)
            }
        }
        .font(.system(size: 12.5))
        .foregroundStyle(.secondary)
    }

    // MARK: - 未安装的 Agent 折叠区

    private var notInstalledSection: some View {
        GlassCard {
            GlassRow(showTopDivider: false, verticalPadding: 12) {
                Button {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        showNotInstalled.toggle()
                    }
                } label: {
                    HStack {
                        Text("未安装 \(notInstalledAgents.count) 个")
                            .font(.system(size: 13.5, weight: .medium))
                            .foregroundStyle(.secondary)
                        Spacer()
                        Image(systemName: showNotInstalled ? "chevron.up" : "chevron.down")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(.secondary)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }

            if showNotInstalled {
                ForEach(notInstalledAgents) { agent in
                    GlassRow {
                        AgentBrandIcon(agentId: agent.id, size: 22)
                        Text(agent.name.isEmpty ? agent.id : agent.name)
                            .font(.system(size: 13.5, weight: .medium))
                        if let hp = agent.homepage, !hp.isEmpty {
                            Text(hp)
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                        Spacer()
                        Button("指定路径") {
                            pickExecutableForAgent(agent.id)
                        }
                        .buttonStyle(.glass)
                        .controlSize(.small)
                    }
                }
            }
        }
    }

    private func groupModelsByProvider(_ models: [ResourceModelEntry]) -> [(provider: String, models: [ResourceModelEntry])] {
        var order: [String] = []
        var dict: [String: [ResourceModelEntry]] = [:]
        for m in models {
            let p = m.provider.trimmingCharacters(in: .whitespacesAndNewlines)
            if dict[p] == nil {
                order.append(p)
                dict[p] = []
            }
            dict[p]?.append(m)
        }
        return order.map { ($0, dict[$0] ?? []) }
    }

    private func pickExecutableForAgent(_ agentId: String) {
        let panel = NSOpenPanel()
        panel.title = "选择 \(agentId) 可执行文件"
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.showsHiddenFiles = true
        if panel.runModal() == .OK, let url = panel.url {
            Task { await store.setAgentCustomPath(agentId: agentId, path: url.path) }
        }
    }
}

private struct ModelNoteEditorSheet: View {
    let title: String
    let kindOptions: [KindOption]
    @State var selectedTags: Set<String>
    @State var note: String
    let onSave: (Set<String>, String) -> Void
    let onCancel: () -> Void

    init(
        title: String,
        kindOptions: [KindOption],
        initialTags: Set<String>,
        initialNote: String,
        onSave: @escaping (Set<String>, String) -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.title = title
        self.kindOptions = kindOptions
        _selectedTags = State(initialValue: initialTags)
        _note = State(initialValue: initialNote)
        self.onSave = onSave
        self.onCancel = onCancel
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("标注模型用途 · \(title)")
                .font(.system(size: 16, weight: .semibold))

            if !kindOptions.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text("擅长的任务类型")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(.secondary)
                    HStack(spacing: 8) {
                        ForEach(kindOptions) { opt in
                            let on = selectedTags.contains(opt.id)
                            Button(opt.name) {
                                if on { selectedTags.remove(opt.id) }
                                else { selectedTags.insert(opt.id) }
                            }
                            .buttonStyle(.plain)
                            .font(.system(size: 12.5, weight: on ? .semibold : .regular))
                            .padding(.horizontal, 10)
                            .padding(.vertical, 5)
                            .background(on ? LGTheme.activeBlue.opacity(0.16) : Color.primary.opacity(0.06), in: Capsule())
                            .overlay(
                                Capsule().strokeBorder(on ? LGTheme.activeBlue : Color.clear, lineWidth: 1)
                            )
                        }
                    }
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("一行备注")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.secondary)
                TextField("例如：审美好、写长文稳、排查并发问题准", text: $note)
                    .textFieldStyle(.roundedBorder)
            }

            HStack {
                Spacer()
                Button("取消", action: onCancel)
                    .buttonStyle(.glass)
                Button("保存") {
                    onSave(selectedTags, note.trimmingCharacters(in: .whitespacesAndNewlines))
                }
                .buttonStyle(.glassProminent)
                .tint(LGTheme.emerald)
            }
        }
        .padding(24)
        .frame(width: 440)
    }
}
