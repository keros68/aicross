import Foundation
import AppKit
import Combine

@MainActor
final class AppStore: ObservableObject {
    static let shared = AppStore()

    private static let selectedTabKey = "aicross.native.selectedTab"
    private static let selectedHostKey = "aicross.native.selectedHost"
    private static let expandedAgentsKey = "aicross.native.expandedAgents"
    static let showMenuBarKey = "aicross.native.showMenuBar"

    // 导航与弹窗状态
    @Published var tab: PageTab = .start
    @Published var activeRunId: String? = nil
    @Published var showSettingsSheet: Bool = false
    @Published var focusResourceAgentId: String? = nil

    // 核心快照与主 Agent 状态
    @Published var appVersion: String = "0.3.15"
    @Published var hosts: [HostEntry] = []
    @Published var hostsLoaded: Bool = false
    @Published var selectedHostId: String = ""
    @Published var snapshot: ConfigSnapshot? = nil
    @Published var skillStatus: SkillStatus? = nil
    @Published var slashList: SlashList? = nil
    @Published var modelTrack: ModelTrack? = nil
    @Published var runs: [RunListItem] = []
    @Published var currentRunView: RunView? = nil

    // 交互与进度反馈状态
    @Published var isRefreshing: Bool = false
    @Published var refreshToast: String = ""
    @Published var isDetecting: Bool = false
    @Published var detectProgress: DetectProgress? = nil
    @Published var completedDetectAgents: Set<String> = []
    @Published var isSettingUp: Bool = false
    @Published var isRecommending: Bool = false
    @Published var smokingKeys: Set<String> = []
    @Published var batchSmokingAgent: (agent: String, done: Int, total: Int)? = nil
    @Published var bannerMessage: String? = nil
    @Published var bannerIsError: Bool = false
    @Published var recommendNotes: [String] = []
    @Published var recommendUnverified: [UnverifiedAgentCount] = []
    @Published var expandedAgents: [String: Bool] = [:]
    @Published var selectedProjectForSkill: String = ""
    @Published var isInstallingAcp: Bool = false
    @Published var acpDownloadedMB: Int = 0
    @Published var acpReports: [AcpAdapterReport] = []

    var onMenuBarVisibilityChanged: ((Bool) -> Void)?

    private let client = EngineClient.shared
    private var batchCancelRequested = false
    private var hasInitialized = false

    var showMenuBarIcon: Bool {
        get {
            if UserDefaults.standard.object(forKey: Self.showMenuBarKey) == nil { return true }
            return UserDefaults.standard.bool(forKey: Self.showMenuBarKey)
        }
        set {
            objectWillChange.send()
            UserDefaults.standard.set(newValue, forKey: Self.showMenuBarKey)
            onMenuBarVisibilityChanged?(newValue)
        }
    }

    var currentHost: HostEntry? {
        hosts.first(where: { $0.id == selectedHostId }) ?? hosts.first
    }

    var currentSkillHost: SkillHostStatus? {
        guard let hId = currentHost?.id ?? (selectedHostId.isEmpty ? nil : selectedHostId) else { return nil }
        return skillStatus?.hosts.first(where: { $0.host == hId })
    }

    private init() {
        if let rawTab = UserDefaults.standard.string(forKey: Self.selectedTabKey),
           let savedTab = PageTab(rawValue: rawTab) {
            self.tab = savedTab
        }
        if let savedHost = UserDefaults.standard.string(forKey: Self.selectedHostKey) {
            self.selectedHostId = savedHost
        }
        if let rawDict = UserDefaults.standard.dictionary(forKey: Self.expandedAgentsKey) as? [String: Bool] {
            self.expandedAgents = rawDict
        }
    }

    func selectTab(_ next: PageTab) {
        if activeRunId != nil {
            activeRunId = nil
            currentRunView = nil
        }
        guard tab != next else { return }
        tab = next
        UserDefaults.standard.set(next.rawValue, forKey: Self.selectedTabKey)
    }

    func selectHost(_ id: String) {
        guard !id.isEmpty, selectedHostId != id else { return }
        selectedHostId = id
        UserDefaults.standard.set(id, forKey: Self.selectedHostKey)
        recommendNotes = []
        recommendUnverified = []
        Task { await reloadForCurrentHost() }
    }

    func toggleAgentExpanded(_ agentId: String, defaultOpen: Bool) {
        let current = expandedAgents[agentId] ?? defaultOpen
        expandedAgents[agentId] = !current
        UserDefaults.standard.set(expandedAgents, forKey: Self.expandedAgentsKey)
    }

    func isAgentExpanded(_ agent: ResourceAgentEntry) -> Bool {
        if let saved = expandedAgents[agent.id] {
            return saved
        }
        return agent.models.contains(where: \.isUnavailable) || agent.state != "可用"
    }

    func jumpToTestAgent(_ agentId: String) {
        expandedAgents[agentId] = true
        UserDefaults.standard.set(expandedAgents, forKey: Self.expandedAgentsKey)
        focusResourceAgentId = agentId
        selectTab(.resources)
    }

    // MARK: - 初始化与全局刷新

    func bootstrapIfNeeded(initialOpenRunId: String? = nil) async {
        if let runId = initialOpenRunId, !runId.isEmpty {
            await openRun(runId)
        }
        guard !hasInitialized else { return }
        hasInitialized = true
        await reloadAll(showToast: false)
        await autoUpdateManagedSkillsQuietly()
        if snapshot?.config.dispatch.acpEnabled == true, Self.isExternalAcpNpmRunning() {
            startBackgroundAcpInstall(announceStart: false)
        }
    }

    func reloadAll(showToast: Bool = true) async {
        guard !isRefreshing else { return }
        isRefreshing = true
        defer { isRefreshing = false }

        if let ver: String = try? await client.call("app_version") {
            appVersion = ver
        }

        var ok = true
        do {
            let loadedHosts: [HostEntry] = try await client.call("hosts_list")
            hosts = loadedHosts
            hostsLoaded = true
            if !loadedHosts.contains(where: { $0.id == selectedHostId }) {
                selectedHostId = loadedHosts.first?.id ?? ""
                if !selectedHostId.isEmpty {
                    UserDefaults.standard.set(selectedHostId, forKey: Self.selectedHostKey)
                }
            }
        } catch {
            hostsLoaded = true
            ok = false
        }

        let hostOk = await reloadForCurrentHost()
        await refreshRunsList()
        if let runId = activeRunId {
            await loadRunDetail(runId)
        }

        if showToast {
            showTemporaryToast(ok && hostOk ? "已刷新" : "刷新失败")
        }
    }

    @discardableResult
    func reloadForCurrentHost() async -> Bool {
        let hId = selectedHostId
        let snapPayload: [String: Any] = hId.isEmpty ? [:] : ["host": hId]
        let skillPayload: [String: Any] = selectedProjectForSkill.isEmpty ? [:] : ["project": selectedProjectForSkill]

        let clientRef = client
        async let snapTask: ConfigSnapshot? = try? await clientRef.call("config_snapshot", payload: snapPayload)
        async let skillTask: SkillStatus? = try? await clientRef.call("skill_status", payload: skillPayload)
        async let slashTask: SlashList? = hId.isEmpty ? nil : (try? await clientRef.call("slash_list", payload: ["host": hId]))
        async let trackTask: ModelTrack? = try? await clientRef.call("model_track")

        let (snapRes, skillRes, slashRes, trackRes) = await (snapTask, skillTask, slashTask, trackTask)

        var allOk = true
        if let snapRes {
            snapshot = snapRes
        } else {
            allOk = false
        }
        if let skillRes {
            skillStatus = skillRes
        } else {
            allOk = false
        }
        slashList = hId.isEmpty ? nil : (slashRes ?? slashList)
        if let trackRes {
            modelTrack = trackRes
        }
        return allOk
    }

    private func autoUpdateManagedSkillsQuietly() async {
        guard let st = skillStatus else { return }
        let bundled = st.bundledVersion.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !bundled.isEmpty else { return }
        var updatedAny = false
        for hostSt in st.hosts {
            let needsUpdate = hostSt.locations.contains {
                $0.scope == "global" && $0.kind == "ours" && $0.version.trimmingCharacters(in: .whitespacesAndNewlines) != bundled
            }
            if needsUpdate {
                if let _: SkillHostStatus = try? await client.call("skill_install", payload: ["host": hostSt.host, "scope": "global"]) {
                    updatedAny = true
                }
            }
        }
        if updatedAny {
            await reloadForCurrentHost()
        }
    }

    // MARK: - 资源探测、连接测试、清理与路径

    func detectResources() async {
        guard !isDetecting else { return }
        isDetecting = true
        detectProgress = nil
        completedDetectAgents = []
        clearBanner()
        client.invalidateEnvironmentCache()

        do {
            let _: ResourcesOutput = try await client.call("resources_detect", timeout: 240) { [weak self] prog in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    self.detectProgress = prog
                    self.completedDetectAgents.insert(prog.agent)
                }
            }
            let loadedHosts: [HostEntry] = (try? await client.call("hosts_list")) ?? hosts
            hosts = loadedHosts
            hostsLoaded = true
            if selectedHostId.isEmpty || !loadedHosts.contains(where: { $0.id == selectedHostId }) {
                selectedHostId = loadedHosts.first?.id ?? ""
                if !selectedHostId.isEmpty {
                    UserDefaults.standard.set(selectedHostId, forKey: Self.selectedHostKey)
                }
            }
            await reloadForCurrentHost()
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }

        detectProgress = nil
        isDetecting = false
    }

    func smokeSingleModel(_ model: ResourceModelEntry) async {
        let key = model.uniqueKey
        guard !smokingKeys.contains(key) else { return }
        smokingKeys.insert(key)
        defer { smokingKeys.remove(key) }

        var payload: [String: Any] = ["agent": model.agent, "model": model.id]
        if !model.provider.isEmpty {
            payload["provider"] = model.provider
        }
        do {
            try await client.callVoid("resources_smoke", payload: payload, timeout: 190)
            await reloadForCurrentHost()
        } catch {
            setBanner("\(model.id) 测试失败：\(error.localizedDescription)", isError: true)
            await reloadForCurrentHost()
        }
    }

    func smokeAllForAgent(_ agent: ResourceAgentEntry) async {
        let targets = agent.batchSmokeTargets
        guard batchSmokingAgent == nil, !targets.isEmpty else { return }
        batchCancelRequested = false
        batchSmokingAgent = (agent.id, 0, targets.count)

        var done = 0
        for model in targets {
            if batchCancelRequested { break }
            var payload: [String: Any] = ["agent": model.agent.isEmpty ? agent.id : model.agent, "model": model.id]
            if !model.provider.isEmpty {
                payload["provider"] = model.provider
            }
            _ = try? await client.callVoid("resources_smoke", payload: payload, timeout: 190)
            done += 1
            batchSmokingAgent = (agent.id, done, targets.count)
        }

        await reloadForCurrentHost()
        batchSmokingAgent = nil
    }

    func cancelBatchSmoke() {
        batchCancelRequested = true
    }

    func pruneMissingModels(agent: String? = nil) async {
        var payload: [String: Any] = [:]
        if let agent { payload["agent"] = agent }
        do {
            let _: PruneResult = try await client.call("resources_prune", payload: payload)
            await reloadForCurrentHost()
            setBanner("已清理未发现的模型条目。", isError: false)
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func setAgentCustomPath(agentId: String, path: String?) async {
        var payload: [String: Any] = ["agent": agentId]
        if let p = path?.trimmingCharacters(in: .whitespacesAndNewlines), !p.isEmpty {
            payload["path"] = p
        }
        do {
            try await client.callVoid("resources_set_path", payload: payload)
            await detectResources()
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func saveModelNote(modelKey: String, tags: [String], note: String) async {
        do {
            let _: ModelNoteValue = try await client.call("model_note_set", payload: [
                "model": modelKey,
                "tags": tags,
                "note": note
            ])
            if let tr: ModelTrack = try? await client.call("model_track") {
                modelTrack = tr
            }
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    // MARK: - 一键配好、模式与席位编排

    func runOneClickSetup(upgradeExternalPath: String? = nil) async {
        guard !isSettingUp else { return }
        isSettingUp = true
        clearBanner()
        defer { isSettingUp = false }

        do {
            if snapshot == nil || snapshot?.resources.needsFirstDetect == true {
                let _: ResourcesOutput = try await client.call("resources_detect", timeout: 240)
                let loadedHosts: [HostEntry] = (try? await client.call("hosts_list")) ?? hosts
                hosts = loadedHosts
                hostsLoaded = true
                if selectedHostId.isEmpty, let first = loadedHosts.first {
                    selectedHostId = first.id
                    UserDefaults.standard.set(first.id, forKey: Self.selectedHostKey)
                }
            }

            guard !selectedHostId.isEmpty else {
                setBanner("未检测到可用的主 agent，请先安装 Claude Code、Codex 或 Kimi Code 等软件。", isError: true)
                return
            }

            if let extPath = upgradeExternalPath {
                try await client.callVoid("skill_upgrade", payload: ["host": selectedHostId, "path": extPath])
            } else {
                let hasExternal = currentSkillHost?.locations.contains { $0.kind == "external" && $0.readByThisHost != false } ?? false
                if !hasExternal {
                    let _: SkillHostStatus = try await client.call("skill_install", payload: ["host": selectedHostId, "scope": "global"])
                }
            }

            // 清理可能附带的隔离扩展属性
            cleanInstalledSkillQuarantine()

            let rec: RecommendResult = try await client.call("config_recommend", payload: [
                "host": selectedHostId,
                "all": false
            ])
            recommendNotes = rec.notes
            recommendUnverified = rec.unverified
            await reloadForCurrentHost()
            setBanner("已完成配置，可直接复制下方示例回到主 agent 使用。", isError: false)
        } catch {
            setBanner(error.localizedDescription, isError: true)
            await reloadForCurrentHost()
        }
    }

    func recommendModes(overwriteAll: Bool) async {
        guard !selectedHostId.isEmpty, !isRecommending else { return }
        isRecommending = true
        clearBanner()
        defer { isRecommending = false }

        do {
            let rec: RecommendResult = try await client.call("config_recommend", payload: [
                "host": selectedHostId,
                "all": overwriteAll
            ])
            recommendNotes = rec.notes
            recommendUnverified = rec.unverified
            await reloadForCurrentHost()
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func setDefaultMode(_ modeId: String) async {
        guard !selectedHostId.isEmpty else { return }
        do {
            let res: DefaultModeResult = try await client.call("config_set_default_mode", payload: [
                "host": selectedHostId,
                "mode": modeId
            ])
            if var snap = snapshot {
                snap.config.defaultMode = res.defaultMode
                snap.config.modes = res.modes
                snapshot = snap
            }
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func updateSeat(modeId: String, stage: Int, index: Int, seat: Seat) async {
        guard !selectedHostId.isEmpty else { return }
        var seatDict: [String: Any] = [
            "agent": seat.agent,
            "model": seat.model,
            "tier": seat.tier,
            "thinking": seat.thinking
        ]
        if let p = seat.provider, !p.isEmpty {
            seatDict["provider"] = p
        } else {
            seatDict["provider"] = NSNull()
        }
        if let fb = seat.fallback {
            var fbDict: [String: Any] = ["agent": fb.agent, "model": fb.model]
            fbDict["provider"] = (fb.provider?.isEmpty == false) ? fb.provider! : NSNull()
            seatDict["fallback"] = fbDict
        } else {
            seatDict["fallback"] = NSNull()
        }

        do {
            let updated: ModeEntry = try await client.call("config_set_seat", payload: [
                "host": selectedHostId,
                "mode": modeId,
                "stage": stage,
                "index": index,
                "seat": seatDict
            ])
            mergeUpdatedMode(updated)
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func clearSeat(modeId: String, stage: Int, index: Int) async {
        guard !selectedHostId.isEmpty else { return }
        do {
            let updated: ModeEntry = try await client.call("config_clear_seat", payload: [
                "host": selectedHostId,
                "mode": modeId,
                "stage": stage,
                "index": index
            ])
            mergeUpdatedMode(updated)
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func setModeCount(modeId: String, count: Int) async {
        guard !selectedHostId.isEmpty else { return }
        do {
            let updated: ModeEntry = try await client.call("config_set_count", payload: [
                "host": selectedHostId,
                "mode": modeId,
                "count": count
            ])
            mergeUpdatedMode(updated)
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func setSeatFocus(modeId: String, positionIndex: Int, focus: String) async {
        guard !selectedHostId.isEmpty else { return }
        do {
            let updated: ModeEntry = try await client.call("config_set_focus", payload: [
                "host": selectedHostId,
                "mode": modeId,
                "index": positionIndex,
                "focus": focus
            ])
            mergeUpdatedMode(updated)
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func setHostVendor(vendor: String) async {
        guard !selectedHostId.isEmpty else { return }
        do {
            let _: HostEntry = try await client.call("host_set_vendor", payload: [
                "host": selectedHostId,
                "vendor": vendor
            ])
            if let loadedHosts: [HostEntry] = try? await client.call("hosts_list") {
                hosts = loadedHosts
            }
            await reloadForCurrentHost()
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func saveCustomPreset(_ preset: PresetPayload) async -> Bool {
        do {
            let encoded = try JSONEncoder().encode(preset)
            let obj = try JSONSerialization.jsonObject(with: encoded)
            var payload: [String: Any] = ["preset": obj]
            if !selectedHostId.isEmpty { payload["host"] = selectedHostId }
            let res: PresetSaveResult = try await client.call("preset_save", payload: payload)
            if var snap = snapshot {
                snap.config.modes = res.modes
                snapshot = snap
            }
            return true
        } catch {
            setBanner(error.localizedDescription, isError: true)
            return false
        }
    }

    func deleteCustomPreset(id: String) async {
        do {
            try await client.callVoid("preset_remove", payload: ["id": id])
            await reloadForCurrentHost()
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    private func mergeUpdatedMode(_ updated: ModeEntry) {
        guard var snap = snapshot else { return }
        snap.config.modes = snap.config.modes.map { $0.id == updated.id ? updated : $0 }
        snapshot = snap
    }

    // MARK: - Skill 与短命令管理

    func installSkill(scope: String, projectDir: String? = nil) async {
        guard !selectedHostId.isEmpty else { return }
        var payload: [String: Any] = ["host": selectedHostId, "scope": scope]
        if let dir = projectDir, !dir.isEmpty {
            payload["project"] = dir
            selectedProjectForSkill = dir
        }
        do {
            let _: SkillHostStatus = try await client.call("skill_install", payload: payload)
            cleanInstalledSkillQuarantine()
            await reloadForCurrentHost()
            setBanner(scope == "project" ? "已安装到所选项目目录。" : "已安装全局 skill。", isError: false)
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func upgradeExternalSkill(path: String) async {
        guard !selectedHostId.isEmpty else { return }
        do {
            try await client.callVoid("skill_upgrade", payload: ["host": selectedHostId, "path": path])
            cleanInstalledSkillQuarantine()
            await reloadForCurrentHost()
            setBanner("已备份旧版并升级为随附版本。", isError: false)
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func uninstallSkill(location: SkillLocation) async {
        guard !selectedHostId.isEmpty else { return }
        var payload: [String: Any] = ["host": selectedHostId, "scope": location.scope]
        if location.scope == "project", !selectedProjectForSkill.isEmpty {
            payload["project"] = selectedProjectForSkill
        }
        do {
            try await client.callVoid("skill_uninstall", payload: payload)
            await reloadForCurrentHost()
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func addSlashCommand(name: String, modeId: String, note: String) async -> Bool {
        guard !selectedHostId.isEmpty else { return false }
        var payload: [String: Any] = ["host": selectedHostId, "name": name, "mode": modeId]
        if !note.isEmpty { payload["note"] = note }
        do {
            let updated: SlashList = try await client.call("slash_add", payload: payload)
            slashList = updated
            return true
        } catch {
            setBanner(error.localizedDescription, isError: true)
            return false
        }
    }

    func removeSlashCommand(name: String) async {
        guard !selectedHostId.isEmpty else { return }
        do {
            let updated: SlashList = try await client.call("slash_remove", payload: [
                "host": selectedHostId,
                "name": name
            ])
            slashList = updated
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    func generateAllModeSlashCommands(prefix: String = "ax-") async {
        guard !selectedHostId.isEmpty else { return }
        do {
            let updated: SlashList = try await client.call("slash_add_all", payload: [
                "host": selectedHostId,
                "prefix": prefix
            ])
            slashList = updated
            setBanner("已按内置模式补齐短命令。", isError: false)
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    private func cleanInstalledSkillQuarantine() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        for rel in [".claude/skills/ai-cross", ".agents/skills/ai-cross", ".gemini/antigravity/skills/ai-cross"] {
            let p = home.appendingPathComponent(rel).path
            if FileManager.default.fileExists(atPath: p) {
                EngineClient.stripQuarantine(at: p)
            }
        }
    }

    // MARK: - 派发记录与查看页

    func refreshRunsList() async {
        if let items: [RunListItem] = try? await client.call("list_runs") {
            runs = items
        }
    }

    func openRun(_ runId: String) async {
        activeRunId = runId
        await loadRunDetail(runId)
    }

    func closeRunViewer() {
        activeRunId = nil
        currentRunView = nil
        tab = .runs
    }

    func loadRunDetail(_ runId: String) async {
        do {
            let detail: RunView = try await client.call("load_run_view", payload: ["runId": runId])
            currentRunView = detail
        } catch {
            currentRunView = nil
        }
    }

    func adoptNode(runId: String, nodeId: String) async {
        do {
            let updated: RunView = try await client.call("adopt_run_node", payload: [
                "runId": runId,
                "nodeId": nodeId
            ])
            currentRunView = updated
            await refreshRunsList()
        } catch {
            setBanner(error.localizedDescription, isError: true)
        }
    }

    // MARK: - 派发设置与后台 ACP 适配器安装

    func fetchDispatchSettings() async throws -> DispatchSettingsView {
        let view: DispatchSettingsView = try await client.call("dispatch_settings", timeout: 25)
        if let reports = view.acp {
            acpReports = reports
        }
        return view
    }

    func saveDispatchSettings(patch: [String: Any]) async throws -> DispatchSaveResult {
        let res: DispatchSaveResult = try await client.call(
            "dispatch_save",
            payload: ["patch": patch, "deferAcpInstall": true],
            timeout: 25
        )
        if var snap = snapshot {
            snap.config.dispatch = res.dispatch
            snapshot = snap
        }
        if let reports = res.acp {
            acpReports = reports
        }
        showTemporaryToast("设置已保存")

        let needsAcpInstall = res.dispatch.acpEnabled && (
            (res.acp ?? acpReports).contains(where: { $0.state == "未安装" })
            || (patch["acpEnabled"] as? Bool == true && (res.acp ?? acpReports).isEmpty)
        )
        if needsAcpInstall {
            startBackgroundAcpInstall(announceStart: true)
        }

        return res
    }

    func startBackgroundAcpInstall(announceStart: Bool = true) {
        guard !isInstallingAcp else { return }
        isInstallingAcp = true
        acpDownloadedMB = Self.measureAcpDirectoryMB()

        if announceStart {
            setBanner("设置已生效。正在后台下载安装 ACP 适配器（首次约 600 MB，安装完成前派发自动使用 CLI）。", isError: false)
        }

        let monitorTask = Task.detached(priority: .utility) { [weak self] in
            while !Task.isCancelled {
                let mb = AppStore.measureAcpDirectoryMB()
                await MainActor.run {
                    self?.acpDownloadedMB = mb
                }
                try? await Task.sleep(nanoseconds: 2_000_000_000)
            }
        }

        Task { @MainActor [weak self] in
            guard let self else {
                monitorTask.cancel()
                return
            }
            defer {
                monitorTask.cancel()
                self.isInstallingAcp = false
            }

            // 若已有外部 npm 正在往 ~/.aicross/acp 安装，先等待其结束避免并发写锁冲突
            while Self.isExternalAcpNpmRunning() {
                try? await Task.sleep(nanoseconds: 2_500_000_000)
            }

            do {
                let result: AcpInstallResult = try await self.client.call("acp_install", timeout: 1800)
                self.acpReports = result.acp
                let failed = result.acp.filter { $0.state == "安装失败" || $0.state == "没有 npm" }
                let summary = result.acp.map { "\($0.agent) \($0.state)" }.joined(separator: "，")
                if failed.isEmpty {
                    self.setBanner("ACP 适配器就绪：\(summary)", isError: false)
                } else {
                    self.setBanner("ACP 适配器部分未就绪（\(summary)），派发时将自动回退一次性 CLI。", isError: true)
                }
            } catch {
                self.setBanner("ACP 适配器后台安装未完成：\(error.localizedDescription)（不影响日常 CLI 派发）", isError: true)
            }
        }
    }

    nonisolated private static func isExternalAcpNpmRunning() -> Bool {
        guard FileManager.default.isExecutableFile(atPath: "/usr/bin/pgrep") else { return false }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/pgrep")
        proc.arguments = ["-f", "npm install.*@agentclientprotocol/"]
        proc.standardInput = FileHandle.nullDevice
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        try? proc.run()
        proc.waitUntilExit()
        return proc.terminationStatus == 0
    }

    nonisolated private static func measureAcpDirectoryMB() -> Int {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let acpDir = ProcessInfo.processInfo.environment["AICROSS_HOME"]
            .map { URL(fileURLWithPath: $0).appendingPathComponent("acp") }
            ?? home.appendingPathComponent(".aicross/acp")
        guard FileManager.default.fileExists(atPath: acpDir.path),
              FileManager.default.isExecutableFile(atPath: "/usr/bin/du") else {
            return 0
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/du")
        proc.arguments = ["-sm", acpDir.path]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = FileHandle.nullDevice
        try? proc.run()
        proc.waitUntilExit()
        let out = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        let firstToken = out.split(whereSeparator: { $0.isWhitespace }).first.map(String.init) ?? "0"
        return Int(firstToken) ?? 0
    }

    // MARK: - 剪贴板与横幅辅助

    func copyText(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        showTemporaryToast("已复制")
    }

    func setBanner(_ message: String, isError: Bool) {
        bannerMessage = message
        bannerIsError = isError
    }

    func clearBanner() {
        bannerMessage = nil
        bannerIsError = false
    }

    private func showTemporaryToast(_ text: String) {
        refreshToast = text
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 1_600_000_000)
            if refreshToast == text {
                refreshToast = ""
            }
        }
    }
}
