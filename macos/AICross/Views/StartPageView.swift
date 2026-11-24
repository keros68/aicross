import SwiftUI

struct StartPageView: View {
    @ObservedObject var store: AppStore
    @State private var confirmUpgradeForSetup: SkillLocation? = nil
    @State private var confirmUpgradeOnly: SkillLocation? = nil

    private var resources: ResourcesOutput? { store.snapshot?.resources }
    private var detected: Bool { resources != nil && resources?.needsFirstDetect == false }
    private var workers: [DisplayHelpers.WorkerSummary] {
        DisplayHelpers.buildWorkers(from: resources, hostId: store.selectedHostId)
    }
    private var bundledVersion: String { store.skillStatus?.bundledVersion ?? "" }

    private var modes: [ModeEntry] { store.snapshot?.config.modes ?? [] }
    private var defaultModeId: String { store.snapshot?.config.defaultMode ?? "" }

    private var reviewModeEntry: ModeEntry? {
        if let cur = modes.first(where: { $0.id == defaultModeId }),
           cur.kind == .blind || cur.kind == .jury {
            return cur
        }
        return modes.first(where: { $0.id == "builtin-blind" })
    }

    private var workModeEntry: ModeEntry? {
        if let cur = modes.first(where: { $0.id == defaultModeId }),
           cur.kind == .shared || cur.kind == .economy || cur.kind == .arena || cur.kind == .swarm {
            return cur
        }
        return modes.first(where: { $0.id == "builtin-economy" })
    }

    private var staleExternalSkill: SkillLocation? {
        store.currentSkillHost?.locations.first {
            $0.kind == "external" && $0.readByThisHost != false && $0.outdated == true && ($0.manager ?? "") != "skills-manager"
        }
    }

    private var isSkillReady: Bool {
        guard let hostSt = store.currentSkillHost else { return false }
        let installed = hostSt.locations.contains { $0.kind != "none" && $0.readByThisHost != false }
        return installed && !hostSt.updateAvailable && staleExternalSkill == nil
    }

    private var isSetupDone: Bool {
        guard detected, !store.selectedHostId.isEmpty, isSkillReady else { return false }
        guard let r = reviewModeEntry, let w = workModeEntry else { return false }
        return r.isReady && w.isReady
    }

    private var missingSeatsCount: Int {
        let targetModes = [reviewModeEntry, workModeEntry].compactMap { $0 }
        var seen = Set<String>()
        var total = 0
        for m in targetModes where seen.insert(m.id).inserted {
            total += m.allSeats.filter { $0.node.isMissing }.count
        }
        return total
    }

    private var unverifiedTotalCount: Int {
        store.recommendUnverified.reduce(0) { $0 + $1.count }
    }

    private var detectButtonTitle: String {
        if store.isDetecting {
            if let p = store.detectProgress, p.total > 0 {
                return "探测中 \(p.done)/\(p.total)"
            }
            return "探测中…"
        }
        return detected ? "重新探测" : "探测"
    }

    var body: some View {
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

                step1HostCard
                step2WorkerCard
                step3SetupCard
                step4UsageCard
            }
            .padding(.horizontal, 28)
            .padding(.top, 10)
            .padding(.bottom, 32)
            .frame(maxWidth: 920)
            .frame(maxWidth: .infinity)
        }
        .alert(
            "升级 ai-cross skill",
            isPresented: Binding(
                get: { confirmUpgradeForSetup != nil || confirmUpgradeOnly != nil },
                set: { if !$0 { confirmUpgradeForSetup = nil; confirmUpgradeOnly = nil } }
            )
        ) {
            Button("升级") {
                if let loc = confirmUpgradeForSetup {
                    confirmUpgradeForSetup = nil
                    Task { await store.runOneClickSetup(upgradeExternalPath: loc.path) }
                } else if let loc = confirmUpgradeOnly {
                    confirmUpgradeOnly = nil
                    Task { await store.upgradeExternalSkill(path: loc.path) }
                }
            }
            Button("取消", role: .cancel) {}
        } message: {
            let loc = confirmUpgradeForSetup ?? confirmUpgradeOnly
            Text("\(store.currentHost?.name ?? store.selectedHostId) 里的 ai-cross skill（\(loc?.version ?? "旧版")）由其他软件安装，读不到面板设置。升级会替换为 \(bundledVersion)，原文件自动备份到 .aicross-skill-backup。")
        }
    }

    // MARK: - 1. 主 agent

    private var step1HostCard: some View {
        GlassCard {
            GlassRow(showTopDivider: false, verticalPadding: 14) {
                StepBadgeView(number: 1, isDone: detected && !store.selectedHostId.isEmpty)

                VStack(alignment: .leading, spacing: 2) {
                    Text("主 agent")
                        .font(.system(size: 16, weight: .semibold))
                    Text("你和 AI 对话的软件，ai-cross skill 装在这里")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }

                Spacer()

                if detected {
                    Button {
                        Task { await store.detectResources() }
                    } label: {
                        Label(detectButtonTitle, systemImage: "arrow.clockwise")
                    }
                    .buttonStyle(.glass)
                    .disabled(store.isDetecting || store.isSettingUp)
                } else {
                    Button {
                        Task { await store.detectResources() }
                    } label: {
                        Label(detectButtonTitle, systemImage: "sparkle.magnifyingglass")
                    }
                    .buttonStyle(.glassProminent)
                    .tint(LGTheme.emerald)
                    .disabled(store.isDetecting || store.isSettingUp)
                }
            }

            if detected {
                if store.hosts.isEmpty {
                    GlassRow {
                        Text("本机尚未找到可用的主 agent")
                            .font(.system(size: 13))
                            .foregroundStyle(.secondary)
                    }
                } else {
                    GlassRow {
                        Text("在哪个软件里用")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(.secondary)
                            .frame(width: 112, alignment: .leading)

                        GlassEffectContainer(spacing: 6) {
                            HStack(spacing: 6) {
                                ForEach(store.hosts) { item in
                                    let isSelected = item.id == store.selectedHostId
                                    Button {
                                        withAnimation(.spring(response: 0.28, dampingFraction: 0.82)) {
                                            store.selectHost(item.id)
                                        }
                                    } label: {
                                        HStack(spacing: 6) {
                                            AgentBrandIcon(agentId: item.id, size: 18)
                                            Text(item.name)
                                                .font(.system(size: 13, weight: isSelected ? .semibold : .regular))
                                        }
                                        .padding(.horizontal, 11)
                                        .padding(.vertical, 6)
                                    }
                                    .buttonStyle(.plain)
                                    .background(
                                        isSelected ? LGTheme.activeBlue.opacity(0.16) : Color.primary.opacity(0.05),
                                        in: Capsule()
                                    )
                                    .overlay {
                                        Capsule()
                                            .strokeBorder(
                                                isSelected ? LGTheme.activeBlue.opacity(0.65) : Color.primary.opacity(0.08),
                                                lineWidth: 1
                                            )
                                    }
                                }
                            }
                        }
                        Spacer()
                    }

                    if !store.selectedHostId.isEmpty, store.skillStatus != nil {
                        GlassRow {
                            Text("ai-cross skill")
                                .font(.system(size: 13, weight: .medium))
                                .foregroundStyle(.secondary)
                                .frame(width: 112, alignment: .leading)

                            skillStateContent
                            Spacer()
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var skillStateContent: some View {
        let hostSt = store.currentSkillHost
        if let ext = hostSt?.locations.first(where: { $0.kind == "external" && $0.readByThisHost != false }) {
            let mgrLabel = (ext.manager?.isEmpty == false) ? "由 \(ext.manager!) 管理" : "外部安装"
            GlassPill(text: ext.version.isEmpty ? mgrLabel : "\(mgrLabel) · \(ext.version)", tone: .muted)
            if ext.outdated == true {
                GlassPill(text: "需要升级到 \(bundledVersion)", tone: .warn)
                if (ext.manager ?? "") != "skills-manager" {
                    Button("升级") {
                        confirmUpgradeOnly = ext
                    }
                    .buttonStyle(.glassProminent)
                    .tint(LGTheme.emerald)
                    .controlSize(.small)
                }
            }
        } else if let ours = hostSt?.locations.first(where: { $0.kind == "ours" && $0.readByThisHost != false }) {
            if !bundledVersion.isEmpty && ours.version != bundledVersion {
                GlassPill(text: "可更新 · \(ours.version) → \(bundledVersion)", tone: .warn)
                Button("更新") {
                    Task { await store.installSkill(scope: "global") }
                }
                .buttonStyle(.glassProminent)
                .tint(LGTheme.emerald)
                .controlSize(.small)
            } else {
                GlassPill(text: ours.version.isEmpty ? "已安装" : "已安装 · \(ours.version)", tone: .ok)
            }
        } else {
            GlassPill(text: "未安装", tone: .muted)
        }
    }

    // MARK: - 2. 副 agent

    private var step2WorkerCard: some View {
        GlassCard {
            GlassRow(showTopDivider: false, verticalPadding: 14) {
                StepBadgeView(number: 2, isDone: !workers.isEmpty)

                VStack(alignment: .leading, spacing: 2) {
                    Text("副 agent")
                        .font(.system(size: 16, weight: .semibold))
                    Text(detected ? "被派去干活、审查的各家；同一家的模型可能经几个 agent 都能用" : "被派去干活、审查的；探测后自动列出")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }

                Spacer()

                if detected {
                    Button("详情") {
                        store.selectTab(.resources)
                    }
                    .buttonStyle(.glass)
                }
            }

            ForEach(workers) { worker in
                GlassRow {
                    HStack(spacing: 8) {
                        AgentBrandIcon(agentId: worker.id, size: 20)
                        Text(worker.name)
                            .font(.system(size: 13.5, weight: .medium))
                    }
                    .frame(width: 140, alignment: .leading)

                    Text(worker.detailText)
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)

                    Spacer()

                    if worker.isHost {
                        GlassPill(text: "当前主 agent", tone: .muted)
                    }
                }
            }

            if detected && workers.isEmpty {
                GlassRow {
                    Text("没有找到能派发的副 agent")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    // MARK: - 3. 配好

    private var step3SetupCard: some View {
        GlassCard {
            GlassRow(showTopDivider: false, verticalPadding: 14) {
                StepBadgeView(number: 3, isDone: isSetupDone)

                VStack(alignment: .leading, spacing: 2) {
                    Text("配好")
                        .font(.system(size: 16, weight: .semibold))
                    if !detected {
                        Text("先点击第 1 步「探测」后即可一键配好")
                            .font(.system(size: 13))
                            .foregroundStyle(.secondary)
                    } else if store.selectedHostId.isEmpty {
                        Text("请先在上方选择你平时使用的主 agent")
                            .font(.system(size: 13))
                            .foregroundStyle(.secondary)
                    } else {
                        Text("自动安装全局 skill 并为审查与日常执行选好跨厂商模型（手动选过的不动）")
                            .font(.system(size: 13))
                            .foregroundStyle(.secondary)
                    }
                }

                Spacer()

                if isSetupDone {
                    Button("改人选") {
                        store.selectTab(.modes)
                    }
                    .buttonStyle(.glass)
                }

                if detected && !store.selectedHostId.isEmpty {
                    if isSetupDone {
                        Button {
                            triggerOneClickSetup()
                        } label: {
                            Label(store.isSettingUp ? "配置中…" : "重新推荐", systemImage: "sparkles")
                        }
                        .buttonStyle(.glass)
                        .disabled(store.isSettingUp || store.isDetecting)
                    } else {
                        Button {
                            triggerOneClickSetup()
                        } label: {
                            Label(store.isSettingUp ? "配置中…" : "一键配好", systemImage: "sparkles")
                        }
                        .buttonStyle(.glassProminent)
                        .tint(LGTheme.emerald)
                        .disabled(store.isSettingUp || store.isDetecting)
                    }
                }
            }

            if detected && !store.selectedHostId.isEmpty && store.snapshot != nil {
                modeSummaryRow(label: "审", mode: reviewModeEntry)
                modeSummaryRow(label: "干活", mode: workModeEntry)

                if missingSeatsCount > 0 {
                    GlassRow {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("还有 \(missingSeatsCount) 个位置尚未选到模型。可点击「一键配好」自动推荐，或前往「模式」页手动挑选。")
                                .font(.system(size: 13))
                                .foregroundStyle(.secondary)
                            if unverifiedTotalCount > 0 {
                                Text("另有 \(unverifiedTotalCount) 个第三方模型尚未测试连接，在「资源」页验证通过后即可参与推荐。")
                                    .font(.system(size: 12.5))
                                    .foregroundStyle(LGTheme.warnAmber)
                            }
                        }
                        Spacer()
                        if unverifiedTotalCount > 0 {
                            Button("去验证") {
                                store.selectTab(.resources)
                            }
                            .buttonStyle(.glass)
                            .controlSize(.small)
                        }
                        Button("手选人选") {
                            store.selectTab(.modes)
                        }
                        .buttonStyle(.glass)
                        .controlSize(.small)
                    }
                }
            }
        }
    }

    private func modeSummaryRow(label: String, mode: ModeEntry?) -> some View {
        GlassRow {
            Text(label)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(.secondary)
                .frame(width: 64, alignment: .leading)

            if let mode {
                HStack(spacing: 8) {
                    ModeIconBadge(kind: mode.kind, size: 24)
                    Text(mode.name)
                        .font(.system(size: 13.5, weight: .semibold))
                }
                .frame(width: 126, alignment: .leading)

                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(Array(mode.allSeats.enumerated()), id: \.offset) { _, item in
                            let modelText = item.node.effective?.model ?? "未选"
                            let isAlert = item.node.hasAlert
                            HStack(spacing: 5) {
                                Text(item.node.role)
                                    .font(.system(size: 11.5, weight: .medium))
                                    .foregroundStyle(.secondary)
                                Text(modelText)
                                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                                    .foregroundStyle(isAlert ? LGTheme.dangerRed : .primary)
                            }
                            .padding(.horizontal, 9)
                            .padding(.vertical, 4)
                            .background(
                                isAlert ? LGTheme.dangerRed.opacity(0.10) : Color.primary.opacity(0.05),
                                in: RoundedRectangle(cornerRadius: 7, style: .continuous)
                            )
                            .help([item.node.effective?.agent, item.node.effective?.provider, item.node.effective?.model]
                                .compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " / "))
                        }
                    }
                }
            } else {
                Text("未找到模式")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
    }

    // MARK: - 4. 怎么用

    private var step4UsageCard: some View {
        let invoke = DisplayHelpers.invokePrefix(hostId: store.selectedHostId, fromSlash: store.slashList?.invoke)
        let examples: [(String, String)] = [
            ("审", "\(invoke) 让别家审一下 main.py"),
            ("干活", "\(invoke) 写一个读取 CSV 的函数，做完让别家验收")
        ]

        return GlassCard {
            GlassRow(showTopDivider: false, verticalPadding: 14) {
                StepBadgeView(number: 4, isDone: false)

                VStack(alignment: .leading, spacing: 2) {
                    Text("怎么用")
                        .font(.system(size: 16, weight: .semibold))
                    Text(store.currentHost != nil ? "回到 \(store.currentHost!.name) 对话框，直接输入：" : "回到你常用的 AI 软件对话框，直接输入：")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }

            ForEach(examples, id: \.0) { label, cmd in
                GlassRow {
                    GlassPill(text: label, tone: .muted)
                        .frame(width: 56, alignment: .leading)

                    Text(cmd)
                        .font(.system(size: 13, design: .monospaced))
                        .textSelection(.enabled)
                        .lineLimit(1)

                    Spacer()

                    Button {
                        store.copyText(cmd)
                    } label: {
                        Label("复制", systemImage: "doc.on.doc")
                    }
                    .buttonStyle(.glass)
                    .controlSize(.small)
                }
            }
        }
    }

    private func triggerOneClickSetup() {
        if let stale = staleExternalSkill {
            confirmUpgradeForSetup = stale
        } else {
            Task { await store.runOneClickSetup() }
        }
    }
}
