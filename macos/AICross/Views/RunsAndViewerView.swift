import SwiftUI

// MARK: - 派发记录列表页（镜像 Home.tsx）

struct RunsPageView: View {
    @ObservedObject var store: AppStore
    @State private var statusFilter: String = "all" // all | running | done | failed
    @State private var searchText: String = ""

    private var runningCount: Int {
        store.runs.filter { $0.status == "running" }.count
    }

    private var doneCount: Int {
        store.runs.filter { $0.status == "done" || $0.status == "ok" }.count
    }

    private var failedCount: Int {
        store.runs.filter { $0.status == "failed" || $0.status == "error" }.count
    }

    private var filteredRuns: [RunListItem] {
        store.runs.filter { item in
            let statusMatches: Bool = {
                switch statusFilter {
                case "running": return item.status == "running"
                case "done": return item.status == "done" || item.status == "ok"
                case "failed": return item.status == "failed" || item.status == "error"
                default: return true
                }
            }()
            guard statusMatches else { return false }
            let q = searchText.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if q.isEmpty { return true }
            return item.taskTitle.lowercased().contains(q)
                || item.presetName.lowercased().contains(q)
                || item.runId.lowercased().contains(q)
        }
    }

    private var overviewItems: [StatCellItem] {
        [
            StatCellItem(label: "最近派发", value: "\(store.runs.count)", filterTag: "all"),
            StatCellItem(label: "运行中", value: "\(runningCount)", filterTag: runningCount > 0 ? "running" : nil),
            StatCellItem(label: "已完成", value: "\(doneCount)", filterTag: doneCount > 0 ? "done" : nil),
            StatCellItem(label: "失败", value: "\(failedCount)", isAlert: failedCount > 0, filterTag: failedCount > 0 ? "failed" : nil)
        ]
    }

    var body: some View {
        VStack(spacing: 0) {
            PageToolbarBar {
                Picker("状态筛选", selection: $statusFilter) {
                    Text("全部").tag("all")
                    Text("运行中").tag("running")
                    Text("已完成").tag("done")
                    Text("失败").tag("failed")
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 260)
            } trailing: {
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                    TextField("搜索任务或模式", text: $searchText)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12.5))
                        .frame(width: 170)
                    if !searchText.isEmpty {
                        Button {
                            searchText = ""
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .font(.system(size: 11))
                                .foregroundStyle(.secondary)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .glassEffect(.regular, in: .capsule)
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

                    GlassStatStrip(
                        items: overviewItems,
                        activeFilter: statusFilter == "all" ? nil : statusFilter,
                        onSelectFilter: { next in
                            statusFilter = next ?? "all"
                        }
                    )

                    GlassCard {
                        // 表头
                        GlassRow(showTopDivider: false, verticalPadding: 10) {
                            Text("时间")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(.secondary)
                                .frame(width: 110, alignment: .leading)

                            Text("模式")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(.secondary)
                                .frame(width: 132, alignment: .leading)

                            Text("任务")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(.secondary)

                            Spacer()

                            Text("状态")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(.secondary)
                                .frame(width: 84, alignment: .trailing)
                        }

                        if filteredRuns.isEmpty {
                            GlassRow(verticalPadding: 34) {
                                Spacer()
                                VStack(spacing: 8) {
                                    Image(systemName: "tray")
                                        .font(.system(size: 26))
                                        .foregroundStyle(.tertiary)
                                    Text(store.runs.isEmpty
                                         ? "暂无派发记录。在主 agent 对话里输入调用命令后，派发结果会自动出现在这里。"
                                         : "没有符合筛选条件的派发记录。")
                                        .font(.system(size: 13))
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                            }
                        } else {
                            ForEach(filteredRuns) { item in
                                runTableRow(item)
                            }
                        }
                    }
                }
                .padding(.horizontal, 28)
                .padding(.top, 8)
                .padding(.bottom, 32)
                .frame(maxWidth: 980)
                .frame(maxWidth: .infinity)
            }
        }
        .task {
            await store.refreshRunsList()
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 5_000_000_000)
                if Task.isCancelled { break }
                await store.refreshRunsList()
            }
        }
    }

    private func runTableRow(_ item: RunListItem) -> some View {
        let kind = ModeKind.from(id: "", name: item.presetName)
        return Button {
            Task { await store.openRun(item.runId) }
        } label: {
            GlassRow(verticalPadding: 12) {
                Text(DisplayHelpers.formatTimestamp(item.startedAt))
                    .font(.system(size: 12.5, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .frame(width: 110, alignment: .leading)

                HStack(spacing: 7) {
                    ModeIconBadge(kind: kind, size: 22)
                    Text(item.presetName.isEmpty ? "默认" : item.presetName)
                        .font(.system(size: 13, weight: .medium))
                        .lineLimit(1)
                }
                .frame(width: 132, alignment: .leading)

                Text(item.taskTitle.isEmpty ? item.runId : item.taskTitle)
                    .font(.system(size: 13.5))
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                    .truncationMode(.tail)

                Spacer(minLength: 12)

                HStack(spacing: 6) {
                    GlassPill(
                        text: DisplayHelpers.formatRunStatus(item.status),
                        tone: runStatusTone(item.status)
                    )
                    Image(systemName: "chevron.right")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(.tertiary)
                }
                .frame(width: 96, alignment: .trailing)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func runStatusTone(_ status: String) -> PillTone {
        switch status {
        case "running": return .active
        case "done", "ok": return .ok
        case "failed", "error": return .alert
        default: return .muted
        }
    }
}

// MARK: - 派发详情查看页（镜像 RunViewer.tsx）

struct RunViewerView: View {
    @ObservedObject var store: AppStore
    let runId: String

    @State private var adoptingNodeId: String? = nil

    private var view: RunView? { store.currentRunView }

    private var stagesGrouped: [(stage: Int, nodes: [RunViewNode])] {
        guard let nodes = view?.nodes else { return [] }
        let dict = Dictionary(grouping: nodes, by: \.stage)
        return dict.keys.sorted().map { st in
            (st, dict[st] ?? [])
        }
    }

    private var canAdoptInRun: Bool {
        guard let v = view else { return false }
        let kind = ModeKind.from(id: "", name: v.presetName)
        if kind == .arena { return true }
        return stagesGrouped.contains { pair in
            pair.nodes.filter { $0.artifacts != nil && $0.hasResult }.count > 1
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            PageToolbarBar {
                Button {
                    store.closeRunViewer()
                } label: {
                    Label("返回派发记录", systemImage: "chevron.left")
                }
                .buttonStyle(.glass)

                if let v = view {
                    let kind = ModeKind.from(id: "", name: v.presetName)
                    ModeIconBadge(kind: kind, size: 24)
                    Text(v.presetName)
                        .font(.system(size: 14.5, weight: .semibold))
                    GlassPill(
                        text: DisplayHelpers.formatRunStatus(v.status),
                        tone: runTone(v.status)
                    )
                }
            } trailing: {
                if let v = view {
                    Text("ID: \(v.runId)")
                        .font(.system(size: 11.5, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)

                    Button {
                        store.copyText(v.runId)
                    } label: {
                        Image(systemName: "doc.on.doc")
                            .font(.system(size: 11.5))
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                    .help("复制 Run ID")
                }
            }

            ScrollView {
                if let v = view {
                    VStack(alignment: .leading, spacing: 16) {
                        runSummaryHeader(v)

                        if !v.reason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                            GlassNoticeBanner(
                                icon: v.status == "failed" ? "exclamationmark.triangle.fill" : "info.circle.fill",
                                message: v.reason,
                                isAlert: v.status == "failed"
                            )
                        }

                        if !v.leftover.isEmpty {
                            leftoverCard(v.leftover)
                        }

                        ForEach(stagesGrouped, id: \.stage) { group in
                            stageSection(run: v, stage: group.stage, nodes: group.nodes)
                        }
                    }
                    .padding(.horizontal, 28)
                    .padding(.top, 8)
                    .padding(.bottom, 36)
                    .frame(maxWidth: 1120)
                    .frame(maxWidth: .infinity)
                } else {
                    VStack(spacing: 12) {
                        ProgressView()
                        Text("正在加载派发详情…")
                            .font(.system(size: 13))
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 80)
                }
            }
        }
        .task(id: runId) {
            await store.loadRunDetail(runId)
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                if Task.isCancelled { break }
                if store.currentRunView?.status == "running" {
                    await store.loadRunDetail(runId)
                }
            }
        }
    }

    private func runSummaryHeader(_ v: RunView) -> some View {
        let totalMs = v.nodes.compactMap(\.ms).max()
        return GlassCard {
            GlassRow(showTopDivider: false, verticalPadding: 14) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(v.taskTitle.isEmpty ? v.graphTitle : v.taskTitle)
                        .font(.system(size: 16, weight: .semibold))
                        .textSelection(.enabled)

                    HStack(spacing: 16) {
                        Label("开始：\(DisplayHelpers.formatTimestamp(v.startedAt))", systemImage: "clock")
                        if let totalMs, totalMs > 0 {
                            Label("最长阶段耗时：\(DisplayHelpers.formatDuration(ms: totalMs))", systemImage: "stopwatch")
                        }
                        if !v.project.isEmpty {
                            Label(v.project, systemImage: "folder")
                                .lineLimit(1)
                                .truncationMode(.middle)
                        }
                    }
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                }
                Spacer()
            }
        }
    }

    private func leftoverCard(_ items: [String]) -> some View {
        GlassCard {
            GlassRow(showTopDivider: false, verticalPadding: 12) {
                Label("遗留问题 · \(items.count) 项", systemImage: "exclamationmark.bubble.fill")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(LGTheme.warnAmber)
                Spacer()
            }
            ForEach(Array(items.enumerated()), id: \.offset) { _, line in
                GlassRow(verticalPadding: 9) {
                    Text("• \(line)")
                        .font(.system(size: 13))
                        .textSelection(.enabled)
                    Spacer()
                }
            }
        }
    }

    private func stageSection(run: RunView, stage: Int, nodes: [RunViewNode]) -> some View {
        let colCount = min(max(nodes.count, 1), 2)
        let columns = Array(repeating: GridItem(.flexible(minimum: 320), spacing: 14, alignment: .top), count: colCount)

        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Text("第 \(stage) 阶段")
                    .font(.system(size: 13.5, weight: .semibold))
                    .foregroundStyle(.secondary)
                Text("· \(nodes.count) 个位置")
                    .font(.system(size: 12.5))
                    .foregroundStyle(.tertiary)
                Spacer()
            }
            .padding(.horizontal, 4)

            LazyVGrid(columns: columns, alignment: .leading, spacing: 14) {
                ForEach(nodes) { node in
                    nodeCard(run: run, node: node)
                }
            }
        }
    }

    private func nodeCard(run: RunView, node: RunViewNode) -> some View {
        GlassCard(isActive: node.adopted) {
            // 卡片头
            GlassRow(showTopDivider: false, verticalPadding: 12) {
                AgentBrandIcon(agentId: node.agent, size: 28)

                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(node.swarmTitle.isEmpty ? node.role : "\(node.role) · \(node.swarmTitle)")
                            .font(.system(size: 14, weight: .semibold))

                        if node.adopted {
                            GlassPill(text: "已采纳", tone: .ok)
                        }
                    }

                    let prov = node.provider.isEmpty ? "" : " · \(node.provider)"
                    Text("\(node.agent)\(prov) / \(node.model)")
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                Spacer()

                if canAdoptInRun && node.hasResult && !node.adopted && run.status != "running" {
                    Button(adoptingNodeId == node.nodeId ? "采纳中…" : "采纳此版本") {
                        Task {
                            adoptingNodeId = node.nodeId
                            await store.adoptNode(runId: run.runId, nodeId: node.nodeId)
                            adoptingNodeId = nil
                        }
                    }
                    .buttonStyle(.glassProminent)
                    .tint(LGTheme.emerald)
                    .controlSize(.small)
                    .disabled(adoptingNodeId != nil)
                }

                GlassPill(
                    text: DisplayHelpers.formatNodeStatus(node.status),
                    tone: nodeTone(node.status)
                )
            }

            // 关键运行事实
            GlassRow(verticalPadding: 10) {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 14) {
                        factPair(label: "耗时", value: DisplayHelpers.formatDuration(ms: node.ms))
                        if let tIn = node.tokensIn, let tOut = node.tokensOut {
                            let cacheStr = (node.tokensCache ?? 0) > 0 ? " (缓存 \(node.tokensCache!))" : ""
                            factPair(label: "Token", value: "入 \(tIn) / 出 \(tOut)\(cacheStr)")
                        }
                        if !node.independence.isEmpty {
                            factPair(label: "独立性", value: node.independence)
                        }
                    }

                    if !node.modelReported.isEmpty {
                        HStack(spacing: 6) {
                            Text("实际响应模型：\(node.modelReported)")
                                .font(.system(size: 11.5, design: .monospaced))
                                .foregroundStyle(node.isIdentityMismatch ? LGTheme.dangerRed : .secondary)
                            if node.isIdentityMismatch {
                                GlassPill(text: "实际模型不符", tone: .alert)
                            }
                        }
                    }

                    if !node.fallbackFrom.isEmpty && !node.fallbackTo.isEmpty {
                        Text("已自动切换备选：\(node.fallbackFrom) → \(node.fallbackTo)")
                            .font(.system(size: 11.5))
                            .foregroundStyle(LGTheme.warnAmber)
                    }
                }
                Spacer()
            }

            // 结构化审查结论 或 正文输出
            if node.reviewStructured {
                structuredReviewBody(node)
            } else if !node.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                GlassRow(verticalPadding: 12) {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("输出内容")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(.secondary)
                            Spacer()
                            Button("复制") {
                                store.copyText(node.text)
                            }
                            .buttonStyle(.glass)
                            .controlSize(.mini)
                        }

                        ScrollView {
                            Text(node.text)
                                .font(.system(size: 12.5))
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .textSelection(.enabled)
                        }
                        .frame(maxHeight: 280)
                    }
                }
            } else if !node.skippedReason.isEmpty {
                GlassRow(verticalPadding: 10) {
                    Text(node.skippedReason)
                        .font(.system(size: 12.5))
                        .foregroundStyle(.secondary)
                    Spacer()
                }
            }

            // 产物 Diff 折叠区
            if let art = node.artifacts, (art.changedFileCount > 0 || !art.diff.isEmpty) {
                GlassRow(verticalPadding: 10) {
                    DisclosureGroup("文件改动 · \(art.changedFileCount) 个文件") {
                        ScrollView([.horizontal, .vertical]) {
                            Text(art.diff)
                                .font(.system(size: 11.5, design: .monospaced))
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .textSelection(.enabled)
                                .padding(8)
                        }
                        .frame(maxHeight: 240)
                        .background(Color.primary.opacity(0.04), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                        .padding(.top, 6)
                    }
                    .font(.system(size: 12.5, weight: .medium))
                }
            }
        }
    }

    @ViewBuilder
    private func structuredReviewBody(_ node: RunViewNode) -> some View {
        GlassRow(verticalPadding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                if node.reviewErrors.isEmpty {
                    HStack(spacing: 6) {
                        Image(systemName: "checkmark.seal.fill")
                            .foregroundStyle(LGTheme.emerald)
                        Text("未发现会导致出错的问题")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(LGTheme.emerald)
                    }
                } else {
                    Text("会出错 · \(node.reviewErrors.count) 条")
                        .font(.system(size: 12.5, weight: .semibold))
                        .foregroundStyle(LGTheme.dangerRed)

                    ForEach(Array(node.reviewErrors.enumerated()), id: \.offset) { idx, errLine in
                        Text("\(idx + 1). \(errLine)")
                            .font(.system(size: 12.5))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(8)
                            .background(LGTheme.dangerRed.opacity(0.08), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                            .textSelection(.enabled)
                    }
                }

                if !node.reviewSuggestions.isEmpty {
                    DisclosureGroup("改进建议 · \(node.reviewSuggestions.count) 条") {
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(Array(node.reviewSuggestions.enumerated()), id: \.offset) { idx, sug in
                                Text("\(idx + 1). \(sug)")
                                    .font(.system(size: 12.5))
                                    .foregroundStyle(.secondary)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .textSelection(.enabled)
                            }
                        }
                        .padding(.top, 6)
                    }
                    .font(.system(size: 12.5, weight: .medium))
                }

                if !node.text.isEmpty {
                    DisclosureGroup("查看回复原文") {
                        ScrollView {
                            Text(node.text)
                                .font(.system(size: 12))
                                .foregroundStyle(.secondary)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .textSelection(.enabled)
                        }
                        .frame(maxHeight: 200)
                        .padding(.top, 6)
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
                }
            }
        }
    }

    private func factPair(label: String, value: String) -> some View {
        HStack(spacing: 4) {
            Text("\(label)：")
                .foregroundStyle(.tertiary)
            Text(value)
                .foregroundStyle(.secondary)
        }
        .font(.system(size: 11.5))
    }

    private func runTone(_ status: String) -> PillTone {
        switch status {
        case "running": return .active
        case "done", "ok": return .ok
        case "failed", "error": return .alert
        default: return .muted
        }
    }

    private func nodeTone(_ status: String) -> PillTone {
        switch status {
        case "done", "ok": return .ok
        case "running", "pending": return .active
        case "failed", "error", "quota_exhausted": return .alert
        default: return .muted
        }
    }
}
