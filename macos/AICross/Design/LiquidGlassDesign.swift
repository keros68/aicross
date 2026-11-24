import SwiftUI
import AppKit

// MARK: - Apple Native Liquid Glass 视觉规范

enum LGTheme {
    static let emerald = Color(red: 0.14, green: 0.67, blue: 0.42)
    static let activeBlue = Color(red: 0.22, green: 0.56, blue: 0.98)
    static let warnAmber = Color(red: 0.86, green: 0.58, blue: 0.08)
    static let dangerRed = Color(red: 0.86, green: 0.26, blue: 0.24)

    static let cardRadius: CGFloat = 16
    static let controlRadius: CGFloat = 10

    static func modeColor(_ kind: ModeKind) -> Color {
        switch kind {
        case .shared: return Color(red: 0.24, green: 0.52, blue: 0.95)
        case .economy: return Color(red: 0.16, green: 0.66, blue: 0.41)
        case .blind: return Color(red: 0.54, green: 0.36, blue: 0.88)
        case .arena: return Color(red: 0.90, green: 0.48, blue: 0.16)
        case .jury: return Color(red: 0.12, green: 0.64, blue: 0.68)
        case .swarm: return Color(red: 0.80, green: 0.58, blue: 0.10)
        case .custom: return Color.secondary
        }
    }

    static func vendorColor(_ name: String) -> Color {
        let low = name.lowercased()
        if low.contains("anthropic") || low.contains("claude") {
            return Color(red: 0.82, green: 0.45, blue: 0.26)
        }
        if low.contains("openai") || low.contains("codex") || low.contains("gpt") {
            return Color(red: 0.10, green: 0.64, blue: 0.50)
        }
        if low.contains("google") || low.contains("gemini") || low.contains("agy") {
            return Color(red: 0.24, green: 0.51, blue: 0.96)
        }
        if low.contains("moonshot") || low.contains("kimi") {
            return Color(red: 0.46, green: 0.36, blue: 0.88)
        }
        if low.contains("zhipu") || low.contains("glm") || low.contains("智谱") {
            return Color(red: 0.26, green: 0.44, blue: 0.90)
        }
        if low.contains("deepseek") {
            return Color(red: 0.18, green: 0.50, blue: 0.92)
        }
        if low.contains("alibaba") || low.contains("qwen") {
            return Color(red: 0.90, green: 0.48, blue: 0.14)
        }
        return Color.secondary
    }
}

// MARK: - 柔和流体环境底色（让 Liquid Glass 折射出通透层次）

struct LiquidAmbientBackdrop: View {
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        ZStack {
            Color(nsColor: .windowBackgroundColor)

            RadialGradient(
                colors: [
                    LGTheme.emerald.opacity(scheme == .dark ? 0.10 : 0.07),
                    Color.clear
                ],
                center: .topLeading,
                startRadius: 40,
                endRadius: 560
            )

            RadialGradient(
                colors: [
                    LGTheme.activeBlue.opacity(scheme == .dark ? 0.10 : 0.06),
                    Color.clear
                ],
                center: .topTrailing,
                startRadius: 60,
                endRadius: 620
            )
        }
        .ignoresSafeArea()
    }
}

// MARK: - 通透分层内容卡片（遵循 Apple HIG：浮动层用 Liquid Glass，阅读卡片用高清晰度毛玻璃层）

struct GlassCard<Content: View>: View {
    @Environment(\.colorScheme) private var scheme
    var isActive: Bool = false
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            RoundedRectangle(cornerRadius: LGTheme.cardRadius, style: .continuous)
                .fill(scheme == .dark ? Color.white.opacity(0.055) : Color.white.opacity(0.84))
                .background(
                    .ultraThinMaterial,
                    in: RoundedRectangle(cornerRadius: LGTheme.cardRadius, style: .continuous)
                )
                .shadow(
                    color: Color.black.opacity(scheme == .dark ? 0.24 : 0.045),
                    radius: 14,
                    x: 0,
                    y: 5
                )
        }
        .overlay {
            RoundedRectangle(cornerRadius: LGTheme.cardRadius, style: .continuous)
                .strokeBorder(
                    isActive
                        ? LGTheme.activeBlue.opacity(0.85)
                        : (scheme == .dark ? Color.white.opacity(0.11) : Color.black.opacity(0.065)),
                    lineWidth: isActive ? 1.5 : 1
                )
        }
        .clipShape(RoundedRectangle(cornerRadius: LGTheme.cardRadius, style: .continuous))
    }
}

struct GlassRow<Content: View>: View {
    var showTopDivider: Bool = true
    var verticalPadding: CGFloat = 12
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if showTopDivider {
                Divider()
                    .opacity(0.55)
            }
            HStack(alignment: .center, spacing: 12) {
                content()
            }
            .padding(.horizontal, 20)
            .padding(.vertical, verticalPadding)
            .frame(minHeight: 54)
        }
    }
}

// MARK: - 页顶大字统计玻璃条（对齐原版 StatCards）

struct StatCellItem: Identifiable {
    let id = UUID()
    var label: String
    var value: String
    var isAlert: Bool = false
    var filterTag: String? = nil
}

struct GlassStatStrip: View {
    var items: [StatCellItem]
    var activeFilter: String? = nil
    var onSelectFilter: ((String?) -> Void)? = nil

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(items.enumerated()), id: \.element.id) { idx, item in
                if idx > 0 {
                    Divider()
                        .frame(height: 40)
                        .opacity(0.5)
                }

                if let tag = item.filterTag, let onSelectFilter {
                    let selected = activeFilter == tag
                    Button {
                        onSelectFilter(selected ? nil : tag)
                    } label: {
                        cellBody(item, isSelected: selected)
                    }
                    .buttonStyle(.plain)
                } else {
                    cellBody(item, isSelected: false)
                }
            }
        }
        .frame(maxWidth: .infinity)
        .glassEffect(.regular, in: .rect(cornerRadius: LGTheme.cardRadius))
    }

    private func cellBody(_ item: StatCellItem, isSelected: Bool) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(item.value)
                .font(.system(size: 24, weight: .semibold, design: .rounded))
                .monospacedDigit()
                .foregroundStyle(item.isAlert ? LGTheme.dangerRed : .primary)
                .lineLimit(1)
            Text(item.label)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 20)
        .padding(.vertical, 14)
        .background(isSelected ? LGTheme.activeBlue.opacity(0.12) : Color.clear)
        .overlay(alignment: .bottom) {
            if isSelected {
                Capsule()
                    .fill(LGTheme.activeBlue)
                    .frame(height: 2.5)
                    .padding(.horizontal, 14)
            }
        }
        .contentShape(Rectangle())
    }
}

// MARK: - 状态胶囊与步骤序号圆章

enum PillTone {
    case ok, warn, alert, active, muted
}

struct GlassPill: View {
    let text: String
    var tone: PillTone = .muted
    var monospaced: Bool = false

    private var fg: Color {
        switch tone {
        case .ok: return LGTheme.emerald
        case .warn: return LGTheme.warnAmber
        case .alert: return LGTheme.dangerRed
        case .active: return LGTheme.activeBlue
        case .muted: return .secondary
        }
    }

    private var bg: Color {
        switch tone {
        case .ok: return LGTheme.emerald.opacity(0.14)
        case .warn: return LGTheme.warnAmber.opacity(0.15)
        case .alert: return LGTheme.dangerRed.opacity(0.14)
        case .active: return LGTheme.activeBlue.opacity(0.14)
        case .muted: return Color.primary.opacity(0.06)
        }
    }

    var body: some View {
        Text(text)
            .font(monospaced ? .system(size: 11.5, weight: .medium, design: .monospaced) : .system(size: 12, weight: .medium))
            .foregroundStyle(fg)
            .padding(.horizontal, 9)
            .padding(.vertical, 3.5)
            .background(bg, in: Capsule())
            .overlay {
                Capsule()
                    .strokeBorder(fg.opacity(0.22), lineWidth: 0.8)
            }
    }
}

struct StepBadgeView: View {
    let number: Int
    let isDone: Bool

    var body: some View {
        ZStack {
            if isDone {
                Circle()
                    .fill(LGTheme.emerald.opacity(0.16))
                    .overlay(Circle().strokeBorder(LGTheme.emerald.opacity(0.45), lineWidth: 1))
                Image(systemName: "checkmark")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(LGTheme.emerald)
            } else {
                Circle()
                    .fill(Color.primary.opacity(0.07))
                    .overlay(Circle().strokeBorder(Color.primary.opacity(0.12), lineWidth: 1))
                Text("\(number)")
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .monospacedDigit()
                    .foregroundStyle(.secondary)
            }
        }
        .frame(width: 28, height: 28)
    }
}

// MARK: - 模式与 Agent 品牌图标

struct ModeIconBadge: View {
    let kind: ModeKind
    var size: CGFloat = 36

    var body: some View {
        let tint = LGTheme.modeColor(kind)
        ZStack {
            RoundedRectangle(cornerRadius: size * 0.28, style: .continuous)
                .fill(tint.opacity(0.14))
                .overlay(
                    RoundedRectangle(cornerRadius: size * 0.28, style: .continuous)
                        .strokeBorder(tint.opacity(0.32), lineWidth: 1)
                )
            Image(systemName: kind.symbol)
                .font(.system(size: size * 0.44, weight: .semibold))
                .foregroundStyle(tint)
        }
        .frame(width: size, height: size)
    }
}

struct AgentBrandIcon: View {
    let agentId: String
    var size: CGFloat = 24
    @Environment(\.colorScheme) private var scheme

    private var assetName: String? {
        let low = agentId.lowercased()
        if low.contains("claude") || low.contains("anthropic") { return "agent_claude" }
        if low.contains("codex") || low.contains("openai") { return "agent_codex" }
        if low.contains("kimi") || low.contains("moonshot") { return "agent_kimicode" }
        if low == "pi" || low.hasPrefix("pi:") { return "agent_pi" }
        if low.contains("agy") || low.contains("antigravity") { return "agent_antigravity" }
        if low.contains("opencode") { return "agent_opencode" }
        if low.contains("qwen") || low.contains("alibaba") { return "agent_qwen" }
        if low.contains("grok") || low.contains("xai") { return "agent_grok" }
        if low.contains("mimo") { return "agent_mimocode" }
        return nil
    }

    var body: some View {
        if let asset = assetName, NSImage(named: asset) != nil {
            ZStack {
                RoundedRectangle(cornerRadius: size * 0.26, style: .continuous)
                    .fill(scheme == .dark ? Color.white.opacity(0.92) : Color.primary.opacity(0.05))
                Image(asset)
                    .resizable()
                    .scaledToFit()
                    .padding(size * 0.16)
            }
            .frame(width: size, height: size)
        } else {
            let tint = LGTheme.vendorColor(agentId)
            let initial = String(agentId.trimmingCharacters(in: .whitespaces).prefix(1)).uppercased()
            ZStack {
                RoundedRectangle(cornerRadius: size * 0.26, style: .continuous)
                    .fill(tint.opacity(0.16))
                    .overlay(
                        RoundedRectangle(cornerRadius: size * 0.26, style: .continuous)
                            .strokeBorder(tint.opacity(0.3), lineWidth: 1)
                    )
                Text(initial.isEmpty ? "A" : initial)
                    .font(.system(size: size * 0.48, weight: .bold, design: .rounded))
                    .foregroundStyle(tint)
            }
            .frame(width: size, height: size)
        }
    }
}

// MARK: - 页面子工具栏与提示横幅

struct PageToolbarBar<Leading: View, Trailing: View>: View {
    @ViewBuilder var leading: () -> Leading
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        HStack(spacing: 12) {
            HStack(spacing: 10) {
                leading()
            }
            Spacer(minLength: 12)
            HStack(spacing: 8) {
                trailing()
            }
        }
        .padding(.horizontal, 28)
        .padding(.top, 6)
        .padding(.bottom, 10)
    }
}

struct HostVendorSubbar: View {
    @ObservedObject var store: AppStore
    let host: HostEntry

    var body: some View {
        HStack(spacing: 10) {
            AgentBrandIcon(agentId: host.id, size: 22)
            Text(host.name)
                .font(.system(size: 14, weight: .semibold))
            Text("·")
                .foregroundStyle(.tertiary)
            Text("厂商：\(host.vendor)")
                .font(.system(size: 13))
                .foregroundStyle(host.vendor == "未知" ? LGTheme.warnAmber : .secondary)
                .help(host.vendorBasis)

            Menu {
                Button {
                    Task { await store.setHostVendor(vendor: "auto") }
                } label: {
                    if host.vendorSource != "用户设定" {
                        Label("自动推定", systemImage: "checkmark")
                    } else {
                        Text("自动推定")
                    }
                }
                Divider()
                ForEach(HostEntry.knownVendors, id: \.self) { vendor in
                    Button {
                        Task { await store.setHostVendor(vendor: vendor) }
                    } label: {
                        if host.vendorSource == "用户设定" && host.vendor == vendor {
                            Label(vendor, systemImage: "checkmark")
                        } else {
                            Text(vendor)
                        }
                    }
                }
            } label: {
                HStack(spacing: 4) {
                    Text(host.vendorSource == "用户设定" ? "手动设定" : "改厂商")
                        .font(.system(size: 12, weight: .medium))
                    Image(systemName: "chevron.up.chevron.down")
                        .font(.system(size: 10, weight: .semibold))
                }
                .padding(.horizontal, 9)
                .padding(.vertical, 4)
            }
            .buttonStyle(.glass)
            .controlSize(.small)
        }
    }
}

struct GlassNoticeBanner: View {
    let icon: String
    let message: String
    var isAlert: Bool = false
    var actionTitle: String? = nil
    var action: (() -> Void)? = nil
    var onDismiss: (() -> Void)? = nil

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(isAlert ? LGTheme.dangerRed : LGTheme.emerald)

            Text(message)
                .font(.system(size: 13.5))
                .foregroundStyle(.primary)
                .textSelection(.enabled)

            Spacer(minLength: 8)

            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .buttonStyle(.glassProminent)
                    .tint(LGTheme.emerald)
                    .controlSize(.small)
            }

            if let onDismiss {
                Button(action: onDismiss) {
                    Image(systemName: "xmark")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 11)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }
}
