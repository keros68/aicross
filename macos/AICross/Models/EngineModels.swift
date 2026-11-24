import Foundation

// MARK: - 顶栏五页导航（与原版 Windows 完全一致）

enum PageTab: String, CaseIterable, Identifiable, Sendable {
    case start = "start"
    case resources = "resources"
    case skill = "skill"
    case modes = "modes"
    case runs = "runs"

    var id: String { rawValue }

    var label: String {
        switch self {
        case .start: return "开始"
        case .resources: return "资源"
        case .skill: return "skill"
        case .modes: return "模式"
        case .runs: return "派发记录"
        }
    }

    var symbol: String {
        switch self {
        case .start: return "sparkles"
        case .resources: return "square.stack.3d.up"
        case .skill: return "puzzlepiece.extension"
        case .modes: return "slider.horizontal.below.square.and.square.filled"
        case .runs: return "clock.arrow.trianglehead.counterclockwise.rotate.90"
        }
    }
}

// MARK: - 主 Agent（镜像 `aicross hosts`）

struct HostEntry: Codable, Equatable, Identifiable, Sendable {
    var id: String
    var name: String
    var vendor: String
    var vendorSource: String
    var vendorBasis: String

    static let knownVendors: [String] = [
        "Anthropic", "OpenAI", "Google", "Moonshot", "Zhipu",
        "DeepSeek", "Alibaba", "MiniMax", "StepFun", "xAI", "其他"
    ]
}

// MARK: - 资源库（镜像 `aicross resources`）

struct DetectProgress: Codable, Equatable, Sendable {
    var agent: String
    var done: Int
    var total: Int
}

struct ResourceModelEntry: Codable, Equatable, Identifiable, Sendable {
    var id: String
    var agent: String
    var provider: String
    var vendor: String?
    var resolved_id: String
    var state: String
    var note: String
    var missing: Bool?

    var uniqueKey: String {
        "\(agent)|\(provider)|\(id)"
    }

    var canSmoke: Bool {
        state == "未验证" || state == "验证失败" || state == "冒烟超时"
    }

    var isSelectable: Bool {
        state == "可用" || state == "已验证"
    }

    var isUnavailable: Bool {
        state == "未发现" || state == "验证失败" || state == "未登录" || state == "额度用尽"
    }

    var isMissingModel: Bool {
        missing ?? (state == "未发现")
    }

    var displayState: String {
        state == "冒烟超时" ? "连接超时" : state
    }

    var resolvedDisplay: String {
        let trimmed = resolved_id.trimmingCharacters(in: .whitespacesAndNewlines)
        return (trimmed.isEmpty || trimmed == id) ? "" : trimmed
    }
}

struct ResourceAgentEntry: Codable, Equatable, Identifiable, Sendable {
    var id: String
    var name: String
    var version: String
    var state: String
    var note: String
    var models: [ResourceModelEntry]
    var untested: Bool?
    var homepage: String?
    var custom_path: String?

    var isInstalled: Bool {
        state != "未安装"
    }

    var isConfirmedInstalled: Bool {
        state != "未安装" && state != "未探测"
    }

    var isListedInMain: Bool {
        isInstalled || !models.isEmpty
    }

    var isNativeAgent: Bool {
        !models.isEmpty && models.allSatisfy { $0.provider.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    }

    var isExpandable: Bool {
        !models.isEmpty || !note.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || state != "可用"
    }

    var batchSmokeTargets: [ResourceModelEntry] {
        guard state == "可用" else { return [] }
        return models.filter { !$0.isMissingModel && $0.canSmoke }
    }
}

struct ResourcesOutput: Codable, Equatable, Sendable {
    var agents: [ResourceAgentEntry]
    var detected_at: String?

    var needsFirstDetect: Bool {
        (detected_at ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

struct PruneResult: Codable, Sendable {
    var agents: [ResourceAgentEntry]
    var detected_at: String?
    var removed: [String]?
}

// MARK: - 模式与席位（镜像 `aicross config show` / `modes.json`）

struct SeatFallback: Codable, Equatable, Hashable, Sendable {
    var agent: String
    var provider: String?
    var model: String
}

struct Seat: Codable, Equatable, Hashable, Sendable {
    var agent: String
    var provider: String?
    var model: String
    var tier: String
    var thinking: String
    var fallback: SeatFallback?
    var origin: String?
}

struct EffectiveSeat: Codable, Equatable, Sendable {
    var agent: String
    var provider: String?
    var model: String
    var tier: String?
    var thinking: String?
    var fallback: SeatFallback?
    var origin: String?
    var state: String?
    var invalid: String?
}

struct ModeSeatNode: Codable, Equatable, Sendable {
    var role: String
    var visibility: String
    var write: String
    var focus: String?
    var focusOptions: [String]?
    var presetThinking: String?
    var seat: Seat?
    var effective: EffectiveSeat?
    var source: String

    var isMissing: Bool {
        source == "未指定" || effective == nil
    }

    var ruleViolation: String {
        (effective?.invalid ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var invalidStateLabel: String {
        let rule = ruleViolation
        if !rule.isEmpty { return "不能放在此位置" }
        let st = effective?.state ?? ""
        let bad: Set<String> = ["未发现", "验证失败", "未登录", "不在资源库"]
        return bad.contains(st) ? st : ""
    }

    var hasAlert: Bool {
        isMissing || !invalidStateLabel.isEmpty
    }

    var isRecommended: Bool {
        source == "位置" && seat?.origin == "recommended"
    }

    var isPresetFixed: Bool {
        source == "预设"
    }
}

struct ModeCount: Codable, Equatable, Sendable {
    var role: String
    var value: Int
    var min: Int
    var max: Int
    var `default`: Int
}

struct ModeEntry: Codable, Equatable, Identifiable, Sendable {
    var id: String
    var name: String
    var description: String?
    var count: ModeCount?
    var builtin: Bool
    var `default`: Bool
    var stages: [[ModeSeatNode]]

    var allSeats: [(stage: Int, index: Int, node: ModeSeatNode)] {
        var list: [(Int, Int, ModeSeatNode)] = []
        for (sIdx, stageNodes) in stages.enumerated() {
            for (nIdx, node) in stageNodes.enumerated() {
                list.append((sIdx + 1, nIdx + 1, node))
            }
        }
        return list
    }

    var isReady: Bool {
        let seats = allSeats
        return !seats.isEmpty && seats.allSatisfy { !$0.node.hasAlert }
    }
}

enum ModeKind: String, Sendable {
    case shared, economy, blind, arena, jury, swarm, custom

    static func from(id: String, name: String) -> ModeKind {
        switch id.trimmingCharacters(in: .whitespacesAndNewlines) {
        case "builtin-shared": return .shared
        case "builtin-economy": return .economy
        case "builtin-blind": return .blind
        case "builtin-arena": return .arena
        case "builtin-jury": return .jury
        case "builtin-swarm": return .swarm
        default: break
        }
        switch name.trimmingCharacters(in: .whitespacesAndNewlines) {
        case "共享模式": return .shared
        case "经济模式": return .economy
        case "盲审模式": return .blind
        case "擂台模式": return .arena
        case "评审模式": return .jury
        case "蜂群模式": return .swarm
        default: return .custom
        }
    }

    var symbol: String {
        switch self {
        case .shared: return "bubble.left.and.bubble.right.fill"
        case .economy: return "sparkles.rectangle.stack.fill"
        case .blind: return "eye.slash.fill"
        case .arena: return "trophy.fill"
        case .jury: return "scalemass.fill"
        case .swarm: return "square.grid.2x2.fill"
        case .custom: return "slider.horizontal.3"
        }
    }

    var fitSummary: String {
        switch self {
        case .shared: return "适合问答、读代码、出主意；一个模型看完整对话后作答"
        case .economy: return "适合要改代码的日常任务；高档模型出方案，低档模型改，另一家验收"
        case .blind: return "适合核对方案、改动、结论等关键产出；几家各自审查，互相看不到"
        case .arena: return "适合想要几个版本挑一个；几家各做一份，选定一份采纳"
        case .jury: return "适合分头查问题；几家分别从安全、性能、需求覆盖角度审查"
        case .swarm: return "适合能拆成互不重叠几块的大任务；各家并行完成后依次合并"
        case .custom: return ""
        }
    }
}

extension ModeEntry {
    var kind: ModeKind {
        builtin ? ModeKind.from(id: id, name: name) : .custom
    }

    var writesFiles: Bool {
        kind == .swarm || stages.contains { stage in stage.contains { $0.write != "只读" } }
    }

    var writeBadgeText: String {
        writesFiles ? "改项目文件" : "只读"
    }

    var guideText: String {
        if builtin, kind != .custom {
            return kind.fitSummary
        }
        return (description ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

// MARK: - 派发设置与快照

struct DispatchSettings: Codable, Equatable, Sendable {
    var timeoutSecs: Int
    var execTimeoutSecs: Int
    var idleTimeoutSecs: Int
    var concurrency: Int
    var maxRequests: Int
    var swarmLimit: Int
    var consoleRunLimit: Int
    var fixRoundLimit: Int
    var redact: Bool
    var writeProjectDispatch: Bool
    var ccSwitchPath: String
    var validationCommand: String
    var acpEnabled: Bool
    var classificationModel: String
    var classifyProposals: Bool
}

struct AcpAdapterReport: Codable, Equatable, Identifiable, Sendable {
    var id: String { agent }
    var agent: String
    var state: String
    var reason: String?
}

struct DispatchSettingsView: Codable, Equatable, Sendable {
    var dispatch: DispatchSettings
    var path: String
    var acp: [AcpAdapterReport]?
}

struct DispatchSaveResult: Codable, Equatable, Sendable {
    var dispatch: DispatchSettings
    var note: String?
    var acp: [AcpAdapterReport]?
}

struct AcpInstallResult: Codable, Equatable, Sendable {
    var acp: [AcpAdapterReport]
}

struct ConfigShow: Codable, Equatable, Sendable {
    var dispatch: DispatchSettings
    var defaultMode: String
    var modes: [ModeEntry]
}

struct ConfigSnapshot: Codable, Equatable, Sendable {
    var resources: ResourcesOutput
    var config: ConfigShow
}

struct DefaultModeResult: Codable, Equatable, Sendable {
    var defaultMode: String
    var modes: [ModeEntry]
}

struct UnverifiedAgentCount: Codable, Equatable, Identifiable, Sendable {
    var id: String { agent }
    var agent: String
    var count: Int
}

struct RecommendResult: Codable, Equatable, Sendable {
    var defaultMode: String
    var modes: [ModeEntry]
    var notes: [String]
    var unverified: [UnverifiedAgentCount]

    enum CodingKeys: String, CodingKey {
        case defaultMode, modes, notes, unverified
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        defaultMode = try c.decode(String.self, forKey: .defaultMode)
        modes = try c.decode([ModeEntry].self, forKey: .modes)
        if let arr = try? c.decode([String].self, forKey: .notes) {
            notes = arr
        } else if let single = try? c.decode(String.self, forKey: .notes), !single.isEmpty {
            notes = [single]
        } else {
            notes = []
        }
        unverified = (try? c.decodeIfPresent([UnverifiedAgentCount].self, forKey: .unverified)) ?? []
    }
}

// MARK: - Skill 与自定义短命令

struct SkillLocation: Codable, Equatable, Identifiable, Sendable {
    var id: String { "\(scope):\(path)" }
    var scope: String // "global" | "project"
    var path: String
    var kind: String // "none" | "ours" | "external"
    var version: String
    var linkTarget: String?
    var readByThisHost: Bool?
    var writeTarget: Bool?
    var outdated: Bool?
    var manager: String?
}

struct SkillHostStatus: Codable, Equatable, Sendable {
    var host: String
    var locations: [SkillLocation]
    var duplicate: Bool
    var updateAvailable: Bool
}

struct SkillStatus: Codable, Equatable, Sendable {
    var bundledVersion: String
    var hosts: [SkillHostStatus]
}

struct SlashCommand: Codable, Equatable, Identifiable, Sendable {
    var id: String { name }
    var name: String
    var mode: String
    var modeName: String
    var note: String
    var path: String
    var state: String
    var trigger: String
    var stale: Bool?
}

struct SlashModeEntry: Codable, Equatable, Identifiable, Sendable {
    var id: String { mode }
    var mode: String
    var modeName: String
    var description: String?
    var argForm: String
    var suggestedName: String
    var command: SlashCommand?
}

struct SlashList: Codable, Equatable, Sendable {
    var host: String
    var invoke: String
    var commands: [SlashCommand]
    var modes: [SlashModeEntry]?
    var supported: Bool?
}

// MARK: - 自定义模式保存（镜像 `preset_save`）

struct PresetNodePayload: Codable, Equatable, Sendable {
    var role: String
    var visibility: String
    var write: String
    var thinking: String?
}

struct PresetStagePayload: Codable, Equatable, Sendable {
    var nodes: [PresetNodePayload]
}

struct PresetPayload: Codable, Equatable, Sendable {
    var id: String?
    var name: String
    var description: String?
    var stages: [PresetStagePayload]
}

struct PresetSaveResult: Codable, Sendable {
    var modes: [ModeEntry]
}

// MARK: - 使用记录与用途标注（镜像 `model_track`）

struct KindOption: Codable, Equatable, Identifiable, Sendable {
    var id: String
    var name: String
}

struct TrackCounts: Codable, Equatable, Sendable {
    var kind: String
    var kindName: String
    var runs: Int
    var firstPass: Int
    var fixedPass: Int
    var notPassed: Int
    var failed: Int
    var noAcceptance: Int
    var unjudged: Int
    var adopted: Int
    var judged: Int
    var passRate: Int?
    var firstPassRate: Int?
}

struct TrackModel: Codable, Equatable, Identifiable, Sendable {
    var id: String { model }
    var model: String
    var tags: [String]
    var tagNames: [String]
    var note: String
    var total: TrackCounts
    var byKind: [TrackCounts]
}

struct ModelTrack: Codable, Equatable, Sendable {
    var minJudged: Int
    var kindList: [KindOption]
    var models: [TrackModel]
}

struct ModelNoteValue: Codable, Equatable, Sendable {
    var model: String
    var tags: [String]
    var tagNames: [String]
    var note: String
}

// MARK: - 派发记录列表与查看页（镜像 `list_runs` / `load_run_view`）

struct RunListItem: Codable, Equatable, Identifiable, Sendable {
    var id: String { runId }
    var runId: String
    var presetName: String
    var status: String
    var startedAt: String
    var taskTitle: String
}

struct RunViewArtifacts: Codable, Equatable, Sendable {
    var worktree: String
    var diff: String
    var changedFileCount: Int
}

struct RunViewNode: Codable, Equatable, Identifiable, Sendable {
    var id: String { nodeId }
    var nodeId: String
    var stage: Int
    var role: String
    var agent: String
    var provider: String
    var model: String
    var status: String
    var errorKind: String
    var adopted: Bool
    var hasResult: Bool
    var text: String
    var skippedReason: String
    var modelReported: String
    var tokensIn: Int?
    var tokensOut: Int?
    var tokensCache: Int?
    var ms: Int?
    var fallbackFrom: String
    var fallbackTo: String
    var quotaResetsAt: String
    var swarmTitle: String
    var artifacts: RunViewArtifacts?
    var independence: String
    var reviewStructured: Bool
    var reviewErrors: [String]
    var reviewSuggestions: [String]

    var isIdentityMismatch: Bool {
        let rep = modelReported.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !rep.isEmpty, !model.isEmpty else { return false }
        if errorKind == "identity_mismatch" { return true }
        let reqTail = model.lowercased().split(separator: "/").last.map(String.init) ?? model.lowercased()
        let repTail = rep.lowercased().split(separator: "/").last.map(String.init) ?? rep.lowercased()
        return !(repTail.contains(reqTail) || reqTail.contains(repTail))
    }
}

struct RunView: Codable, Equatable, Sendable {
    var runId: String
    var graphId: String
    var project: String
    var presetName: String
    var graphTitle: String
    var taskTitle: String
    var status: String
    var reason: String
    var startedAt: String
    var updatedAt: String
    var leftover: [String]
    var nodes: [RunViewNode]
}

// MARK: - 人选下拉选项辅助

struct SeatOption: Identifiable, Equatable, Hashable, Sendable {
    var id: String { key }
    var key: String
    var agent: String
    var agentName: String
    var provider: String?
    var model: String
    var state: String
    var disabled: Bool
    var untested: Bool

    static func makeKey(agent: String, provider: String?, model: String) -> String {
        let p = (provider ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return "\(agent)|\(p)|\(model)"
    }

    var shortLabel: String {
        if let p = provider, !p.isEmpty {
            return "\(model)（\(agentName) · \(p)）"
        }
        return "\(model)（\(agentName)）"
    }
}

enum DisplayHelpers {
    private static let familyMap: [String: String] = [
        "anthropic": "Claude",
        "openai": "GPT",
        "google": "Gemini",
        "moonshot": "Kimi",
        "zhipu": "GLM",
        "智谱": "GLM",
        "deepseek": "DeepSeek",
        "alibaba": "Qwen",
        "阿里": "Qwen"
    ]

    static func familyName(of vendor: String?) -> String {
        let v = (vendor ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        if v.isEmpty || v == "其他" || v == "未知" { return "" }
        return familyMap[v.lowercased()] ?? v
    }

    struct WorkerSummary: Identifiable, Equatable {
        var id: String
        var name: String
        var usableCount: Int
        var pendingCount: Int
        var families: [String]
        var isHost: Bool

        var detailText: String {
            let countPart = pendingCount > 0
                ? "可用 \(usableCount) 个，未验证 \(pendingCount) 个"
                : "可用 \(usableCount) 个"
            if families.isEmpty { return countPart }
            return "\(countPart) · 含 \(families.joined(separator: "、"))"
        }
    }

    static func buildWorkers(from resources: ResourcesOutput?, hostId: String) -> [WorkerSummary] {
        guard let agents = resources?.agents else { return [] }
        return agents.filter(\.isConfirmedInstalled).map { agent in
            let usable = agent.models.filter(\.isSelectable)
            let pending = agent.models.filter { $0.state == "未验证" }
            var counts: [String: Int] = [:]
            for m in usable + pending {
                let fam = familyName(of: m.vendor)
                if !fam.isEmpty { counts[fam, default: 0] += 1 }
            }
            let sortedFamilies = counts.sorted {
                if $0.value != $1.value { return $0.value > $1.value }
                return $0.key < $1.key
            }.map(\.key)
            return WorkerSummary(
                id: agent.id,
                name: agent.name.isEmpty ? agent.id : agent.name,
                usableCount: usable.count,
                pendingCount: pending.count,
                families: sortedFamilies,
                isHost: agent.id == hostId
            )
        }
    }

    static func buildSeatOptions(from resources: ResourcesOutput?) -> [SeatOption] {
        guard let agents = resources?.agents else { return [] }
        var list: [SeatOption] = []
        for agent in agents where agent.state == "可用" {
            let agentName = agent.name.isEmpty ? agent.id : agent.name
            for m in agent.models where !m.isMissingModel {
                let aId = m.agent.isEmpty ? agent.id : m.agent
                let prov = m.provider.trimmingCharacters(in: .whitespacesAndNewlines)
                list.append(SeatOption(
                    key: SeatOption.makeKey(agent: aId, provider: prov.isEmpty ? nil : prov, model: m.id),
                    agent: aId,
                    agentName: agentName,
                    provider: prov.isEmpty ? nil : prov,
                    model: m.id,
                    state: m.displayState,
                    disabled: !m.isSelectable,
                    untested: agent.untested == true
                ))
            }
        }
        return list
    }

    static func invokePrefix(hostId: String, fromSlash: String?) -> String {
        let trimmed = (fromSlash ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty { return trimmed }
        switch hostId {
        case "claude", "hermes", "qoder", "qoder-cn": return "/ai-cross"
        case "codex": return "$ai-cross"
        case "kimi", "pi": return "/skill:ai-cross"
        default: return "用 ai-cross"
        }
    }

    static func formatRunStatus(_ status: String) -> String {
        switch status {
        case "running": return "运行中"
        case "done", "ok": return "已完成"
        case "failed", "error": return "失败"
        case "rejected": return "已拒绝"
        case "cancelled": return "已取消"
        default: return status
        }
    }

    static func formatNodeStatus(_ status: String) -> String {
        switch status {
        case "done", "ok": return "完成"
        case "running", "pending": return "运行中"
        case "failed", "error": return "失败"
        case "quota_exhausted": return "额度用尽"
        case "draft": return "待执行"
        default: return status
        }
    }

    static func formatTimestamp(_ iso: String) -> String {
        let trimmed = iso.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "—" }
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let parsed = f.date(from: trimmed) ?? ISO8601DateFormatter().date(from: trimmed)
        guard let date = parsed else {
            return String(trimmed.prefix(19)).replacingOccurrences(of: "T", with: " ")
        }
        let out = DateFormatter()
        out.locale = Locale(identifier: "zh_CN")
        if Calendar.current.isDateInToday(date) {
            out.dateFormat = "今天 HH:mm"
        } else {
            out.dateFormat = "MM-dd HH:mm"
        }
        return out.string(from: date)
    }

    static func formatDuration(ms: Int?) -> String {
        guard let ms, ms > 0 else { return "—" }
        if ms < 1000 { return "\(ms)ms" }
        let secs = Double(ms) / 1000.0
        if secs < 60 { return String(format: "%.1fs", secs) }
        let m = Int(secs) / 60
        let s = Int(secs) % 60
        return "\(m)分\(s)秒"
    }
}
