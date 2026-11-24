import SwiftUI

struct ModesPageView: View {
    @ObservedObject var store: AppStore

    @State private var confirmOverwriteAll: Bool = false
    @State private var confirmDecreaseTarget: (mode: ModeEntry, nextCount: Int, lostCount: Int)? = nil
    @State private var confirmDeleteMode: ModeEntry? = nil
    @State private var editingPresetDraft: PresetEditorDraft? = nil
    @State private var customFocusTarget: CustomFocusRequest? = nil

    struct CustomFocusRequest: Identifiable {
        var id: String { "\(modeId):\(focusPosition)" }
        var modeId: String
        var modeName: String
        var focusPosition: Int
        var currentFocus: String
    }

    private var resources: ResourcesOutput? { store.snapshot?.resources }
    private var modes: [ModeEntry] { store.snapshot?.config.modes ?? [] }
    private var defaultModeId: String { store.snapshot?.config.defaultMode ?? "" }
    private var seatOptions: [SeatOption] { DisplayHelpers.buildSeatOptions(from: resources) }

    private var commonModes: [ModeEntry] {
        modes.filter { $0.builtin && [.shared, .economy, .blind].contains($0.kind) }
    }

    private var moreBuiltinModes: [ModeEntry] {
        modes.filter { $0.builtin && ![.shared, .economy, .blind].contains($0.kind) }
    }

    private var customModes: [ModeEntry] {
        modes.filter { !$0.builtin }
    }

    private var isSkillInstalled: Bool {
        guard let st = store.currentSkillHost else { return true }
        return st.locations.contains { $0.kind != "none" && $0.readByThisHost != false }
    }

    private var overviewItems: [StatCellItem] {
        let defaultName = modes.first(where: { $0.id == defaultModeId })?.name ?? "—"
        let readyCount = modes.filter(\.isReady).count
        let alertSeats = modes.flatMap(\.allSeats).filter { $0.node.hasAlert }.count
        return [
            StatCellItem(label: "默认模式", value: defaultName),
            StatCellItem(label: "已配好模式", value: "\(readyCount) / \(modes.count)"),
            StatCellItem(label: "待处理位置", value: "\(alertSeats)", isAlert: alertSeats > 0)
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
                        editingPresetDraft = PresetEditorDraft.empty()
                    } label: {
                        Label("新建模式", systemImage: "plus")
                    }
                    .buttonStyle(.glass)

                    Button {
                        Task { await store.recommendModes(overwriteAll: false) }
                    } label: {
                        Label(store.isRecommending ? "推荐中…" : "一键推荐", systemImage: "wand.and.stars")
                    }
                    .buttonStyle(.glassProminent)
                    .tint(LGTheme.emerald)
                    .disabled(store.isRecommending || resources?.needsFirstDetect == true)

                    Menu {
                        Button(role: .destructive) {
                            confirmOverwriteAll = true
                        } label: {
                            Label("全部按推荐重填…", systemImage: "arrow.counterclockwise")
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                            .font(.system(size: 14, weight: .medium))
                            .padding(.horizontal, 4)
                    }
                    .buttonStyle(.glass)
                    .help("更多推荐操作")
                }
            }

            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    if let banner = store.bannerMessage {
                        GlassNoticeBanner(
                            icon: store.bannerIsError ? "exclamationmark.triangle.fill" : "checkmark.circle.fill",
                            message: banner,
                            isAlert: store.bannerIsError,
                            onDismiss: { store.clearBanner() }
                        )
                    }

                    topGuidanceBanners

                    GlassStatStrip(items: overviewItems)

                    if !commonModes.isEmpty {
                        modeGroupSection(title: "常用", items: commonModes)
                    }

                    if !moreBuiltinModes.isEmpty {
                        modeGroupSection(title: "更多模式", items: moreBuiltinModes)
                    }

                    if !customModes.isEmpty {
                        modeGroupSection(title: "自定义", items: customModes)
                    }
                }
                .padding(.horizontal, 28)
                .padding(.top, 8)
                .padding(.bottom, 36)
                .frame(maxWidth: 980)
                .frame(maxWidth: .infinity)
            }
        }
        .alert("全部按推荐重填", isPresented: $confirmOverwriteAll) {
            Button("全部重填", role: .destructive) {
                Task { await store.recommendModes(overwriteAll: true) }
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("将覆盖当前主 agent 在所有模式里手动调整过的人选，重新按推荐规则分配。")
        }
        .alert(
            "减少位置数量",
            isPresented: Binding(
                get: { confirmDecreaseTarget != nil },
                set: { if !$0 { confirmDecreaseTarget = nil } }
            )
        ) {
            Button("确认减少", role: .destructive) {
                if let target = confirmDecreaseTarget {
                    confirmDecreaseTarget = nil
                    Task { await store.setModeCount(modeId: target.mode.id, count: target.nextCount) }
                }
            }
            Button("取消", role: .cancel) {}
        } message: {
            if let target = confirmDecreaseTarget {
                Text("将「\(target.mode.name)」调整为 \(target.nextCount) 个位置会清除末尾 \(target.lostCount) 个已设置的人选。")
            }
        }
        .alert(
            "删除自定义模式",
            isPresented: Binding(
                get: { confirmDeleteMode != nil },
                set: { if !$0 { confirmDeleteMode = nil } }
            )
        ) {
            Button("删除", role: .destructive) {
                if let m = confirmDeleteMode {
                    confirmDeleteMode = nil
                    Task { await store.deleteCustomPreset(id: m.id) }
                }
            }
            Button("取消", role: .cancel) {}
        } message: {
            if let m = confirmDeleteMode {
                Text(deleteImpactDescription(for: m))
            }
        }
        .sheet(item: $editingPresetDraft) { draft in
            PresetEditorSheet(
                initialDraft: draft,
                existingModes: modes,
                onSave: { payload in
                    let ok = await store.saveCustomPreset(payload)
                    if ok { editingPresetDraft = nil }
                    return ok
                },
                onCancel: { editingPresetDraft = nil }
            )
        }
        .sheet(item: $customFocusTarget) { req in
            CustomFocusSheet(
                request: req,
                onSave: { focusText in
                    customFocusTarget = nil
                    Task {
                        await store.setSeatFocus(modeId: req.modeId, positionIndex: req.focusPosition, focus: focusText)
                    }
                },
                onCancel: { customFocusTarget = nil }
            )
        }
    }

    // MARK: - 顶部提示与推荐反馈

    @ViewBuilder
    private var topGuidanceBanners: some View {
        if resources?.needsFirstDetect == true {
            GlassNoticeBanner(
                icon: "sparkle.magnifyingglass",
                message: "尚未探测本机可用模型，先去资源页探测后再分配模式人选。",
                actionTitle: "去探测",
                action: { store.selectTab(.resources) }
            )
        } else if !isSkillInstalled {
            GlassNoticeBanner(
                icon: "puzzlepiece.extension",
                message: "\(store.currentHost?.name ?? store.selectedHostId) 尚未安装 ai-cross skill，安装后即可在对话里直接调用。",
                actionTitle: "去安装",
                action: { store.selectTab(.skill) }
            )
        }

        if !store.recommendNotes.isEmpty || !store.recommendUnverified.isEmpty {
            GlassCard {
                GlassRow(showTopDivider: false, verticalPadding: 12) {
                    Image(systemName: "info.circle.fill")
                        .foregroundStyle(LGTheme.activeBlue)

                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(store.recommendNotes, id: \.self) { note in
                            Text(note)
                                .font(.system(size: 13))
                        }
                        if !store.recommendUnverified.isEmpty {
                            HStack(spacing: 8) {
                                Text("还有未验证的模型，验证通过后可参与推荐：")
                                    .font(.system(size: 12.5))
                                    .foregroundStyle(.secondary)
                                ForEach(store.recommendUnverified) { item in
                                    Button("测试 \(item.agent)（\(item.count)）") {
                                        store.jumpToTestAgent(item.agent)
                                    }
                                    .buttonStyle(.glass)
                                    .controlSize(.small)
                                }
                            }
                        }
                    }

                    Spacer()

                    Button {
                        store.recommendNotes = []
                        store.recommendUnverified = []
                    } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    // MARK: - 模式分组与单卡

    private func modeGroupSection(title: String, items: [ModeEntry]) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 4)

            ForEach(items) { mode in
                modeCard(mode)
            }
        }
    }

    private func modeCard(_ mode: ModeEntry) -> some View {
        let isDefault = mode.id == defaultModeId
        let seats = mode.allSeats

        return GlassCard(isActive: isDefault) {
            GlassRow(showTopDivider: false, verticalPadding: 15) {
                ModeIconBadge(kind: mode.kind, size: 38)

                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 8) {
                        Text(mode.name)
                            .font(.system(size: 15.5, weight: .semibold))

                        GlassPill(
                            text: mode.writeBadgeText,
                            tone: mode.writesFiles ? .warn : .muted
                        )

                        if !mode.isReady {
                            GlassPill(text: "待补齐", tone: .alert)
                        }
                    }

                    if !mode.guideText.isEmpty {
                        Text(mode.guideText)
                            .font(.system(size: 12.5))
                            .foregroundStyle(.secondary)
                            .lineLimit(2)
                    }
                }

                Spacer(minLength: 12)

                // 可调席位数（如盲审/擂台/评审人数 −/+）
                if let cnt = mode.count {
                    HStack(spacing: 6) {
                        Text("\(cnt.role) × \(cnt.value)")
                            .font(.system(size: 12, weight: .medium))
                            .monospacedDigit()
                            .foregroundStyle(.secondary)

                        Button {
                            handleDecreaseCount(mode: mode, cnt: cnt)
                        } label: {
                            Image(systemName: "minus")
                                .font(.system(size: 10, weight: .bold))
                                .frame(width: 14, height: 14)
                        }
                        .buttonStyle(.glass)
                        .controlSize(.small)
                        .disabled(cnt.value <= cnt.min)

                        Button {
                            Task { await store.setModeCount(modeId: mode.id, count: cnt.value + 1) }
                        } label: {
                            Image(systemName: "plus")
                                .font(.system(size: 10, weight: .bold))
                                .frame(width: 14, height: 14)
                        }
                        .buttonStyle(.glass)
                        .controlSize(.small)
                        .disabled(cnt.value >= cnt.max)
                    }
                    .padding(.trailing, 4)
                }

                // 复制为自定义模式
                Button {
                    editingPresetDraft = PresetEditorDraft.fromCopy(mode)
                } label: {
                    Image(systemName: "doc.on.doc")
                        .font(.system(size: 12))
                }
                .buttonStyle(.glass)
                .controlSize(.small)
                .help("复制为自定义模式")

                if !mode.builtin {
                    Button {
                        editingPresetDraft = PresetEditorDraft.fromEditing(mode)
                    } label: {
                        Image(systemName: "pencil")
                            .font(.system(size: 12))
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                    .help("编辑自定义模式")

                    Button {
                        confirmDeleteMode = mode
                    } label: {
                        Image(systemName: "trash")
                            .font(.system(size: 12))
                            .foregroundStyle(LGTheme.dangerRed)
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                    .help("删除自定义模式")
                }

                if isDefault {
                    GlassPill(text: "● 默认", tone: .active)
                } else {
                    Button("设为默认") {
                        Task { await store.setDefaultMode(mode.id) }
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                }
            }

            ForEach(Array(seats.enumerated()), id: \.offset) { _, item in
                seatRow(mode: mode, stage: item.stage, index: item.index, node: item.node)
            }
        }
    }

    // MARK: - 单个位置行（模型下拉 + 思考下拉 + 备选下拉）

    private func seatRow(mode: ModeEntry, stage: Int, index: Int, node: ModeSeatNode) -> some View {
        let fPos = focusPosition(in: mode, stage: stage, index: index)

        return GlassRow(verticalPadding: 11) {
            // 阶段序号徽章
            Text("\(stage)")
                .font(.system(size: 12, weight: .bold, design: .rounded))
                .monospacedDigit()
                .foregroundStyle(.secondary)
                .frame(width: 22, height: 22)
                .background(Color.primary.opacity(0.06), in: RoundedRectangle(cornerRadius: 6, style: .continuous))

            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(node.role)
                        .font(.system(size: 13.5, weight: .medium))

                    if let opts = node.focusOptions, !opts.isEmpty, fPos > 0 {
                        focusPickerMenu(mode: mode, position: fPos, options: opts, current: node.focus ?? "")
                    } else if let f = node.focus, !f.isEmpty {
                        GlassPill(text: f, tone: .muted)
                    }

                    if node.isRecommended {
                        GlassPill(text: "推荐", tone: .ok)
                    }
                    if !node.invalidStateLabel.isEmpty {
                        GlassPill(text: node.invalidStateLabel, tone: .alert)
                            .help(node.ruleViolation)
                    }
                }

                HStack(spacing: 6) {
                    let writeDisplay = node.write == "worktree" ? "独立工作区" : node.write
                    Text("\(node.visibility) · \(writeDisplay)")
                        .font(.system(size: 11.5))
                        .foregroundStyle(.secondary)

                    if !node.ruleViolation.isEmpty {
                        Text("· \(node.ruleViolation)")
                            .font(.system(size: 11.5))
                            .foregroundStyle(LGTheme.dangerRed)
                            .lineLimit(1)
                    }
                }
            }

            Spacer(minLength: 12)

            if node.isPresetFixed, let eff = node.effective {
                GlassPill(text: "\(eff.agent) / \(eff.model)（预设固定）", tone: .muted, monospaced: true)
            } else {
                primaryModelMenu(modeId: mode.id, stage: stage, index: index, node: node)
                thinkingMenu(modeId: mode.id, stage: stage, index: index, node: node)
                fallbackModelMenu(modeId: mode.id, stage: stage, index: index, node: node)
            }
        }
    }

    private func focusPickerMenu(mode: ModeEntry, position: Int, options: [String], current: String) -> some View {
        let trimmed = current.trimmingCharacters(in: .whitespacesAndNewlines)
        return Menu {
            Button {
                Task { await store.setSeatFocus(modeId: mode.id, positionIndex: position, focus: "") }
            } label: {
                if trimmed.isEmpty {
                    Label("默认视角", systemImage: "checkmark")
                } else {
                    Text("默认视角")
                }
            }
            Divider()
            ForEach(options, id: \.self) { opt in
                Button {
                    Task { await store.setSeatFocus(modeId: mode.id, positionIndex: position, focus: opt) }
                } label: {
                    if trimmed == opt {
                        Label(opt, systemImage: "checkmark")
                    } else {
                        Text(opt)
                    }
                }
            }
            if !trimmed.isEmpty && !options.contains(trimmed) {
                Button {} label: {
                    Label(trimmed, systemImage: "checkmark")
                }
            }
            Divider()
            Button("自定义视角…") {
                customFocusTarget = CustomFocusRequest(
                    modeId: mode.id,
                    modeName: mode.name,
                    focusPosition: position,
                    currentFocus: trimmed
                )
            }
        } label: {
            HStack(spacing: 3) {
                Text(trimmed.isEmpty ? "默认视角" : trimmed)
                    .font(.system(size: 11.5, weight: .medium))
                Image(systemName: "chevron.down")
                    .font(.system(size: 9, weight: .semibold))
            }
            .padding(.horizontal, 7)
            .padding(.vertical, 2.5)
        }
        .buttonStyle(.glass)
        .controlSize(.small)
    }

    private func primaryModelMenu(modeId: String, stage: Int, index: Int, node: ModeSeatNode) -> some View {
        let grouped = Dictionary(grouping: seatOptions, by: \.agentName)
        let sortedGroupNames = grouped.keys.sorted()
        let curEff = node.effective

        let buttonLabel: String = {
            if let eff = curEff {
                if let p = eff.provider, !p.isEmpty {
                    return "\(eff.model)（\(eff.agent)·\(p)）"
                }
                return "\(eff.model)（\(eff.agent)）"
            }
            return "未选择模型"
        }()

        return Menu {
            if sortedGroupNames.isEmpty {
                Text("暂无可选模型，请先前往资源页探测")
            } else {
                ForEach(sortedGroupNames, id: \.self) { groupName in
                    Section(groupName) {
                        ForEach(grouped[groupName] ?? []) { opt in
                            Button {
                                applyPrimarySelection(modeId: modeId, stage: stage, index: index, node: node, option: opt)
                            } label: {
                                let isCurrent = curEff?.agent == opt.agent
                                    && (curEff?.provider ?? "") == (opt.provider ?? "")
                                    && curEff?.model == opt.model
                                let suffix = opt.state != "可用" && opt.state != "已验证" ? " [\(opt.state)]" : ""
                                let provText = (opt.provider?.isEmpty == false) ? " · \(opt.provider!)" : ""
                                let title = "\(opt.model)\(provText)\(suffix)"
                                if isCurrent {
                                    Label(title, systemImage: "checkmark")
                                } else {
                                    Text(title)
                                }
                            }
                            .disabled(opt.disabled)
                        }
                    }
                }
            }

            if curEff != nil || node.seat != nil {
                Divider()
                Button(role: .destructive) {
                    Task { await store.clearSeat(modeId: modeId, stage: stage, index: index) }
                } label: {
                    Label("清除此位置人选", systemImage: "xmark.circle")
                }
            }
        } label: {
            HStack(spacing: 6) {
                if let eff = curEff {
                    AgentBrandIcon(agentId: eff.agent, size: 16)
                } else {
                    Image(systemName: "plus.circle.dashed")
                        .font(.system(size: 13))
                        .foregroundStyle(LGTheme.warnAmber)
                }
                Text(buttonLabel)
                    .font(.system(size: 12.5, weight: curEff == nil ? .regular : .medium))
                    .foregroundStyle(curEff == nil ? LGTheme.warnAmber : .primary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.system(size: 9.5, weight: .semibold))
                    .foregroundStyle(.secondary)
            }
            .frame(minWidth: 185, maxWidth: 270, alignment: .leading)
        }
        .buttonStyle(.glass)
        .controlSize(.small)
    }

    private func thinkingMenu(modeId: String, stage: Int, index: Int, node: ModeSeatNode) -> some View {
        let currentThinking = node.seat?.thinking ?? ""
        let options: [(value: String, label: String)] = [
            ("", "思考·预设"),
            ("关", "思考·关"),
            ("低", "思考·低"),
            ("中", "思考·中"),
            ("高", "思考·高")
        ]
        let display = options.first(where: { $0.value == currentThinking })?.label ?? "思考·\(currentThinking)"

        return Menu {
            ForEach(options, id: \.value) { item in
                Button {
                    guard let base = baseSeat(from: node) else { return }
                    var next = base
                    next.thinking = item.value
                    Task { await store.updateSeat(modeId: modeId, stage: stage, index: index, seat: next) }
                } label: {
                    if currentThinking == item.value {
                        Label(item.label, systemImage: "checkmark")
                    } else {
                        Text(item.label)
                    }
                }
            }
        } label: {
            Text(display)
                .font(.system(size: 12))
        }
        .buttonStyle(.glass)
        .controlSize(.small)
        .disabled(baseSeat(from: node) == nil)
    }

    private func fallbackModelMenu(modeId: String, stage: Int, index: Int, node: ModeSeatNode) -> some View {
        let fb = node.seat?.fallback ?? node.effective?.fallback
        let selectableOptions = seatOptions.filter { !$0.disabled }
        let grouped = Dictionary(grouping: selectableOptions, by: \.agentName)
        let sortedGroupNames = grouped.keys.sorted()

        return Menu {
            Button {
                guard var next = baseSeat(from: node) else { return }
                next.fallback = nil
                Task { await store.updateSeat(modeId: modeId, stage: stage, index: index, seat: next) }
            } label: {
                if fb == nil {
                    Label("不设备选", systemImage: "checkmark")
                } else {
                    Text("不设备选")
                }
            }

            Divider()

            ForEach(sortedGroupNames, id: \.self) { groupName in
                Section(groupName) {
                    ForEach(grouped[groupName] ?? []) { opt in
                        Button {
                            guard var next = baseSeat(from: node) else { return }
                            next.fallback = SeatFallback(agent: opt.agent, provider: opt.provider, model: opt.model)
                            Task { await store.updateSeat(modeId: modeId, stage: stage, index: index, seat: next) }
                        } label: {
                            let isCur = fb?.agent == opt.agent && (fb?.provider ?? "") == (opt.provider ?? "") && fb?.model == opt.model
                            let provText = (opt.provider?.isEmpty == false) ? " · \(opt.provider!)" : ""
                            if isCur {
                                Label("\(opt.model)\(provText)", systemImage: "checkmark")
                            } else {
                                Text("\(opt.model)\(provText)")
                            }
                        }
                    }
                }
            }
        } label: {
            Text(fb == nil ? "备选" : "备：\(fb!.model)")
                .font(.system(size: 12))
                .foregroundStyle(fb == nil ? .secondary : .primary)
                .lineLimit(1)
                .frame(maxWidth: 120)
        }
        .buttonStyle(.glass)
        .controlSize(.small)
        .disabled(baseSeat(from: node) == nil)
        .help(fb == nil ? "额度用尽或连接失败时自动切换的备选模型" : "备选：\(fb!.agent) / \(fb!.model)")
    }

    // MARK: - 席位修改辅助

    private func baseSeat(from node: ModeSeatNode) -> Seat? {
        if let s = node.seat { return s }
        if let eff = node.effective {
            return Seat(
                agent: eff.agent,
                provider: eff.provider,
                model: eff.model,
                tier: eff.tier ?? "",
                thinking: eff.thinking ?? "",
                fallback: eff.fallback,
                origin: nil
            )
        }
        return nil
    }

    private func applyPrimarySelection(modeId: String, stage: Int, index: Int, node: ModeSeatNode, option: SeatOption) {
        let existing = baseSeat(from: node)
        let next = Seat(
            agent: option.agent,
            provider: option.provider,
            model: option.model,
            tier: existing?.tier ?? "",
            thinking: existing?.thinking ?? "",
            fallback: existing?.fallback,
            origin: nil
        )
        Task { await store.updateSeat(modeId: modeId, stage: stage, index: index, seat: next) }
    }

    private func focusPosition(in mode: ModeEntry, stage: Int, index: Int) -> Int {
        var pos = 0
        for (sIdx, stNodes) in mode.stages.enumerated() {
            for (nIdx, nd) in stNodes.enumerated() {
                if (nd.focusOptions ?? []).isEmpty { continue }
                pos += 1
                if sIdx == stage - 1 && nIdx == index - 1 { return pos }
            }
        }
        return 0
    }

    private func handleDecreaseCount(mode: ModeEntry, cnt: ModeCount) {
        let nextVal = cnt.value - 1
        guard nextVal >= cnt.min else { return }
        let matchingConfigured = mode.stages.flatMap { $0 }.filter { $0.role == cnt.role && $0.seat != nil }
        let allMatching = mode.stages.flatMap { $0 }.filter { $0.role == cnt.role }
        if allMatching.count > nextVal {
            let tailDropped = allMatching.suffix(allMatching.count - nextVal).filter { $0.seat != nil }.count
            if tailDropped > 0 && !matchingConfigured.isEmpty {
                confirmDecreaseTarget = (mode, nextVal, tailDropped)
                return
            }
        }
        Task { await store.setModeCount(modeId: mode.id, count: nextVal) }
    }

    private func deleteImpactDescription(for mode: ModeEntry) -> String {
        let configuredSeats = mode.stages.flatMap { $0 }.filter { $0.seat != nil }.count
        let isDef = mode.id == defaultModeId
        let slashCount = (store.slashList?.commands ?? []).filter { $0.mode == mode.id }.count
        var parts: [String] = []
        if configuredSeats > 0 { parts.append("\(configuredSeats) 个已设人选") }
        if isDef { parts.append("当前默认模式设置") }
        if slashCount > 0 { parts.append("\(slashCount) 个关联短命令") }
        if parts.isEmpty {
            return "确定删除自定义模式「\(mode.name)」吗？"
        }
        return "删除「\(mode.name)」将同时清理：\(parts.joined(separator: "、"))。"
    }
}

// MARK: - 自定义视角输入弹窗

private struct CustomFocusSheet: View {
    let request: ModesPageView.CustomFocusRequest
    let onSave: (String) -> Void
    let onCancel: () -> Void

    @State private var text: String = ""

    private var trimmed: String {
        text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var errorText: String {
        if trimmed.isEmpty { return "请输入 1–20 字的审查视角" }
        if trimmed.count > 20 { return "视角不能超过 20 字" }
        return ""
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("自定义评审视角")
                .font(.system(size: 16, weight: .semibold))

            TextField("例如：并发安全、SQL 性能、边界条件", text: $text)
                .textFieldStyle(.roundedBorder)

            if !text.isEmpty && !errorText.isEmpty {
                Text(errorText)
                    .font(.system(size: 12))
                    .foregroundStyle(LGTheme.dangerRed)
            }

            HStack {
                Spacer()
                Button("取消", action: onCancel)
                    .buttonStyle(.glass)
                Button("保存") {
                    onSave(trimmed)
                }
                .buttonStyle(.glassProminent)
                .tint(LGTheme.emerald)
                .disabled(!errorText.isEmpty)
            }
        }
        .padding(24)
        .frame(width: 380)
        .onAppear { text = request.currentFocus }
    }
}

// MARK: - 自定义模式编辑器（镜像 PresetEditor.tsx + preset-edit.ts）

struct PresetEditorDraft: Identifiable {
    struct StageItem: Identifiable {
        let id = UUID()
        var nodes: [NodeItem]
    }

    struct NodeItem: Identifiable {
        let id = UUID()
        var role: String
        var visibility: String
        var write: String
        var thinking: String
    }

    let id = UUID()
    var presetId: String // 空串代表新建
    var name: String
    var description: String
    var stages: [StageItem]

    static let roleOptions = ["出方案", "执行", "验收", "验证", "拆分"]
    static let visibilityOptions = ["共享", "盲"]
    static let writeOptions: [(value: String, label: String)] = [
        ("只读", "只读"),
        ("单写者", "单写者"),
        ("worktree", "独立工作区")
    ]
    static let thinkingOptions: [(value: String, label: String)] = [
        ("", "思考·沿用"),
        ("关", "思考·关"),
        ("低", "思考·低"),
        ("中", "思考·中"),
        ("高", "思考·高")
    ]

    static func empty() -> PresetEditorDraft {
        PresetEditorDraft(
            presetId: "",
            name: "",
            description: "",
            stages: [StageItem(nodes: [NodeItem(role: "执行", visibility: "共享", write: "只读", thinking: "")])]
        )
    }

    static func fromCopy(_ mode: ModeEntry) -> PresetEditorDraft {
        PresetEditorDraft(
            presetId: "",
            name: "\(mode.name) 副本",
            description: mode.description ?? "",
            stages: extractStages(from: mode)
        )
    }

    static func fromEditing(_ mode: ModeEntry) -> PresetEditorDraft {
        PresetEditorDraft(
            presetId: mode.id,
            name: mode.name,
            description: mode.description ?? "",
            stages: extractStages(from: mode)
        )
    }

    private static func extractStages(from mode: ModeEntry) -> [StageItem] {
        let mapped = mode.stages.filter { !$0.isEmpty }.map { stageNodes in
            StageItem(nodes: stageNodes.map { node in
                let r = roleOptions.contains(node.role) ? node.role : "执行"
                let v = visibilityOptions.contains(node.visibility) ? node.visibility : "共享"
                let w = writeOptions.contains(where: { $0.value == node.write }) ? node.write : "只读"
                let tRaw = (node.presetThinking ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                let t = thinkingOptions.contains(where: { $0.value == tRaw }) ? tRaw : ""
                return NodeItem(role: r, visibility: v, write: w, thinking: t)
            })
        }
        return mapped.isEmpty ? empty().stages : mapped
    }
}

private struct PresetEditorSheet: View {
    @State var draft: PresetEditorDraft
    let existingModes: [ModeEntry]
    let onSave: (PresetPayload) async -> Bool
    let onCancel: () -> Void

    @State private var nameTouched: Bool = false
    @State private var isSaving: Bool = false

    init(
        initialDraft: PresetEditorDraft,
        existingModes: [ModeEntry],
        onSave: @escaping (PresetPayload) async -> Bool,
        onCancel: @escaping () -> Void
    ) {
        _draft = State(initialValue: initialDraft)
        _nameTouched = State(initialValue: !initialDraft.name.isEmpty)
        self.existingModes = existingModes
        self.onSave = onSave
        self.onCancel = onCancel
    }

    private var nameError: String {
        let trimmed = draft.name.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return "名称不能为空" }
        if trimmed.count > 40 { return "名称不能超过 40 字" }
        if existingModes.contains(where: { $0.id != draft.presetId && $0.name.trimmingCharacters(in: .whitespacesAndNewlines) == trimmed }) {
            return "模式名称已存在"
        }
        return ""
    }

    private func stageError(at idx: Int) -> String {
        let nodes = draft.stages[idx].nodes
        if nodes.isEmpty { return "该阶段至少需要 1 个位置" }
        if nodes.filter({ $0.write == "单写者" }).count > 1 {
            return "同一阶段最多只能有 1 个单写者"
        }
        return ""
    }

    private var totalSeatsError: String {
        let count = draft.stages.reduce(0) { $0 + $1.nodes.count }
        if count == 0 { return "至少需要 1 个位置" }
        if count > 12 { return "全部位置总数不能超过 12 个" }
        return ""
    }

    private var hasAnyError: Bool {
        if !nameError.isEmpty || !totalSeatsError.isEmpty { return true }
        for idx in draft.stages.indices where !stageError(at: idx).isEmpty {
            return true
        }
        return false
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(draft.presetId.isEmpty ? "新建模式" : "编辑模式")
                .font(.system(size: 18, weight: .semibold))

            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 12) {
                    Text("名称")
                        .font(.system(size: 13, weight: .medium))
                        .frame(width: 44, alignment: .leading)
                    TextField("例如：双盲安全审查", text: Binding(
                        get: { draft.name },
                        set: {
                            nameTouched = true
                            draft.name = $0
                        }
                    ))
                    .textFieldStyle(.roundedBorder)
                }

                if nameTouched && !nameError.isEmpty {
                    Text(nameError)
                        .font(.system(size: 12))
                        .foregroundStyle(LGTheme.dangerRed)
                        .padding(.leading, 56)
                }

                HStack(spacing: 12) {
                    Text("介绍")
                        .font(.system(size: 13, weight: .medium))
                        .frame(width: 44, alignment: .leading)
                    TextField("一句话说明适用场景（可选）", text: $draft.description)
                        .textFieldStyle(.roundedBorder)
                }
            }

            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    ForEach(Array(draft.stages.enumerated()), id: \.element.id) { sIdx, stage in
                        if sIdx > 0 {
                            HStack {
                                Spacer()
                                Image(systemName: "arrow.down")
                                    .font(.system(size: 12, weight: .bold))
                                    .foregroundStyle(.secondary)
                                Spacer()
                            }
                        }

                        GlassCard {
                            GlassRow(showTopDivider: false, verticalPadding: 10) {
                                Text("第 \(sIdx + 1) 阶段")
                                    .font(.system(size: 13.5, weight: .semibold))
                                Spacer()
                                Button {
                                    draft.stages[sIdx].nodes.append(
                                        .init(role: "执行", visibility: "共享", write: "只读", thinking: "")
                                    )
                                } label: {
                                    Label("添加位置", systemImage: "plus")
                                }
                                .buttonStyle(.glass)
                                .controlSize(.small)

                                if draft.stages.count > 1 {
                                    Button {
                                        draft.stages.remove(at: sIdx)
                                    } label: {
                                        Image(systemName: "trash")
                                            .foregroundStyle(LGTheme.dangerRed)
                                    }
                                    .buttonStyle(.glass)
                                    .controlSize(.small)
                                }
                            }

                            ForEach(Array(stage.nodes.enumerated()), id: \.element.id) { nIdx, _ in
                                GlassRow(verticalPadding: 8) {
                                    Picker("角色", selection: $draft.stages[sIdx].nodes[nIdx].role) {
                                        ForEach(PresetEditorDraft.roleOptions, id: \.self) { r in
                                            Text(r).tag(r)
                                        }
                                    }
                                    .labelsHidden()
                                    .frame(width: 96)

                                    Picker("可见性", selection: $draft.stages[sIdx].nodes[nIdx].visibility) {
                                        ForEach(PresetEditorDraft.visibilityOptions, id: \.self) { v in
                                            Text(v).tag(v)
                                        }
                                    }
                                    .labelsHidden()
                                    .frame(width: 84)

                                    Picker("写权限", selection: $draft.stages[sIdx].nodes[nIdx].write) {
                                        ForEach(PresetEditorDraft.writeOptions, id: \.value) { w in
                                            Text(w.label).tag(w.value)
                                        }
                                    }
                                    .labelsHidden()
                                    .frame(width: 118)

                                    Picker("思考", selection: $draft.stages[sIdx].nodes[nIdx].thinking) {
                                        ForEach(PresetEditorDraft.thinkingOptions, id: \.value) { t in
                                            Text(t.label).tag(t.value)
                                        }
                                    }
                                    .labelsHidden()
                                    .frame(width: 110)

                                    Spacer()

                                    Button {
                                        draft.stages[sIdx].nodes.remove(at: nIdx)
                                    } label: {
                                        Image(systemName: "xmark")
                                            .font(.system(size: 11))
                                    }
                                    .buttonStyle(.glass)
                                    .controlSize(.small)
                                    .disabled(stage.nodes.count <= 1)
                                }
                            }

                            let sErr = stageError(at: sIdx)
                            if !sErr.isEmpty {
                                GlassRow(verticalPadding: 6) {
                                    Text(sErr)
                                        .font(.system(size: 12))
                                        .foregroundStyle(LGTheme.dangerRed)
                                    Spacer()
                                }
                            }
                        }
                    }

                    Button {
                        draft.stages.append(
                            .init(nodes: [.init(role: "验证", visibility: "盲", write: "只读", thinking: "")])
                        )
                    } label: {
                        Label("添加下一阶段", systemImage: "plus.rectangle.on.rectangle")
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                }
            }
            .frame(minHeight: 240, maxHeight: 380)

            if !totalSeatsError.isEmpty {
                Text(totalSeatsError)
                    .font(.system(size: 12))
                    .foregroundStyle(LGTheme.dangerRed)
            }

            HStack {
                Spacer()
                Button("取消", action: onCancel)
                    .buttonStyle(.glass)
                    .disabled(isSaving)

                Button(isSaving ? "保存中…" : "保存") {
                    nameTouched = true
                    guard !hasAnyError else { return }
                    isSaving = true
                    let cleanDesc = draft.description
                        .replacingOccurrences(of: "\n", with: " ")
                        .trimmingCharacters(in: .whitespacesAndNewlines)
                    let payload = PresetPayload(
                        id: draft.presetId.isEmpty ? nil : draft.presetId,
                        name: draft.name.trimmingCharacters(in: .whitespacesAndNewlines),
                        description: cleanDesc.isEmpty ? nil : cleanDesc,
                        stages: draft.stages.map { st in
                            PresetStagePayload(nodes: st.nodes.map { nd in
                                PresetNodePayload(
                                    role: nd.role,
                                    visibility: nd.visibility,
                                    write: nd.write,
                                    thinking: nd.thinking
                                )
                            })
                        }
                    )
                    Task {
                        _ = await onSave(payload)
                        isSaving = false
                    }
                }
                .buttonStyle(.glassProminent)
                .tint(LGTheme.emerald)
                .disabled(isSaving || (nameTouched && hasAnyError))
            }
        }
        .padding(24)
        .frame(width: 580)
    }
}
