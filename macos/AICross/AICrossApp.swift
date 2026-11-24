import SwiftUI
import AppKit

// MARK: - 主窗口 Liquid Glass 壳层（对齐原版 Windows 顶栏结构 + Apple 原生液态玻璃交互）

struct MainShellView: View {
    @ObservedObject var store: AppStore
    @Namespace private var tabGlassNamespace

    var body: some View {
        ZStack {
            LiquidAmbientBackdrop()

            VStack(spacing: 0) {
                liquidGlassTopBar

                Group {
                    if let runId = store.activeRunId {
                        RunViewerView(store: store, runId: runId)
                    } else {
                        switch store.tab {
                        case .start:
                            StartPageView(store: store)
                        case .resources:
                            ResourcesPageView(store: store)
                        case .skill:
                            SkillPageView(store: store)
                        case .modes:
                            ModesPageView(store: store)
                        case .runs:
                            RunsPageView(store: store)
                        }
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .sheet(isPresented: $store.showSettingsSheet) {
            SettingsSheetView(store: store) {
                store.showSettingsSheet = false
            }
        }
    }

    // MARK: - 顶栏：左（品牌 + 主 agent 选择）、中（五页 Liquid Glass 导航）、右（刷新 + 设置）

    private var liquidGlassTopBar: some View {
        HStack(spacing: 14) {
            // 给左上角红黄绿窗口控制圆点留出舒适原生安全间距
            HStack(spacing: 10) {
                brandBadge

                if !store.hosts.isEmpty {
                    hostSelectorMenu
                } else if store.hostsLoaded {
                    GlassPill(text: "无主 agent", tone: .muted)
                }
            }
            .padding(.leading, 76)

            Spacer(minLength: 12)

            // 居中五页分段导航（原生 GlassEffectContainer + glassEffectID 流体过渡）
            GlassEffectContainer(spacing: 6) {
                HStack(spacing: 4) {
                    ForEach(PageTab.allCases) { item in
                        let isSelected = (store.activeRunId == nil && store.tab == item)
                            || (store.activeRunId != nil && item == .runs)

                        Button {
                            withAnimation(.smooth(duration: 0.24)) {
                                store.selectTab(item)
                            }
                        } label: {
                            HStack(spacing: 6) {
                                Image(systemName: item.symbol)
                                    .font(.system(size: 12, weight: isSelected ? .semibold : .medium))
                                Text(item.label)
                                    .font(.system(size: 13, weight: isSelected ? .semibold : .medium))
                            }
                            .foregroundStyle(isSelected ? .primary : .secondary)
                            .padding(.horizontal, 13)
                            .padding(.vertical, 6.5)
                            .contentShape(Capsule())
                            .background {
                                if isSelected {
                                    Capsule()
                                        .fill(LGTheme.emerald.opacity(0.16))
                                        .glassEffect(.regular, in: .capsule)
                                        .glassEffectID("activePageTab", in: tabGlassNamespace)
                                }
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(4)
                .glassEffect(.regular, in: .capsule)
            }

            Spacer(minLength: 12)

            // 右侧状态提示、全局刷新与派发设置按钮
            HStack(spacing: 8) {
                if !store.refreshToast.isEmpty {
                    Text(store.refreshToast)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(.secondary)
                        .transition(.opacity)
                }

                Button {
                    Task { await store.reloadAll(showToast: true) }
                } label: {
                    Image(systemName: "arrow.clockwise")
                        .font(.system(size: 13, weight: .medium))
                        .rotationEffect(.degrees(store.isRefreshing ? 360 : 0))
                        .animation(
                            store.isRefreshing
                                ? .linear(duration: 0.8).repeatForever(autoreverses: false)
                                : .default,
                            value: store.isRefreshing
                        )
                        .frame(width: 18, height: 18)
                }
                .buttonStyle(.glass)
                .disabled(store.isRefreshing)
                .help("刷新（⌘R）")

                Button {
                    store.showSettingsSheet = true
                } label: {
                    Image(systemName: "gearshape")
                        .font(.system(size: 13, weight: .medium))
                        .frame(width: 18, height: 18)
                }
                .buttonStyle(.glass)
                .help("派发设置（⌘,）")
            }
            .padding(.trailing, 20)
        }
        .frame(height: 54)
        .padding(.bottom, 2)
    }

    private var brandBadge: some View {
        HStack(spacing: 7) {
            if NSImage(named: "BrandLogo") != nil {
                Image("BrandLogo")
                    .resizable()
                    .scaledToFit()
                    .frame(width: 22, height: 22)
                    .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
            } else {
                Image(systemName: "point.3.connected.trianglepath.dotted")
                    .font(.system(size: 15, weight: .bold))
                    .foregroundStyle(LGTheme.emerald)
            }
            Text("aicross")
                .font(.system(size: 14.5, weight: .bold, design: .rounded))
        }
    }

    private var hostSelectorMenu: some View {
        Menu {
            Section("切换主 agent") {
                ForEach(store.hosts) { h in
                    Button {
                        store.selectHost(h.id)
                    } label: {
                        if h.id == store.selectedHostId {
                            Label("\(h.name)（\(h.vendor)）", systemImage: "checkmark")
                        } else {
                            Text("\(h.name)（\(h.vendor)）")
                        }
                    }
                }
            }
        } label: {
            HStack(spacing: 6) {
                if let cur = store.currentHost {
                    AgentBrandIcon(agentId: cur.id, size: 17)
                    Text(cur.name)
                        .font(.system(size: 12.5, weight: .medium))
                        .lineLimit(1)
                } else {
                    Text("选择主 agent")
                        .font(.system(size: 12.5, weight: .medium))
                }
                Image(systemName: "chevron.up.chevron.down")
                    .font(.system(size: 9.5, weight: .semibold))
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
        }
        .buttonStyle(.glass)
        .controlSize(.small)
        .help("选择日常在里面对话的主 agent")
    }
}

// MARK: - AppKit 窗口与菜单栏控制器（规避 macOS 27 SwiftUI AppGraph 循环 Bug，冷启动 <0.8s）

@MainActor
final class AICrossWindowController {
    static let shared = AICrossWindowController()

    private var mainWindow: NSWindow?
    private var statusItem: NSStatusItem?
    private let popover = NSPopover()

    func showMainWindow() {
        if let win = mainWindow {
            NSApp.activate(ignoringOtherApps: true)
            win.makeKeyAndOrderFront(nil)
            return
        }

        let rootView = MainShellView(store: AppStore.shared)
        let hostingController = NSHostingController(rootView: rootView)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1140, height: 760),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "aicross"
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.isMovableByWindowBackground = true
        window.isReleasedWhenClosed = false
        window.contentMinSize = NSSize(width: 920, height: 620)
        window.toolbarStyle = .unified
        window.contentViewController = hostingController
        window.setContentSize(NSSize(width: 1160, height: 780))
        window.setFrameAutosaveName("AICrossLiquidGlassWindow")
        if !window.setFrameUsingName("AICrossLiquidGlassWindow") {
            window.setContentSize(NSSize(width: 1160, height: 780))
            window.center()
        }

        self.mainWindow = window
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    func updateStatusItem(visible: Bool) {
        if visible {
            guard statusItem == nil else { return }
            let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
            if let btn = item.button {
                let img = NSImage(
                    systemSymbolName: "point.3.connected.trianglepath.dotted",
                    accessibilityDescription: "aicross"
                )
                img?.isTemplate = true
                btn.image = img
                btn.target = self
                btn.action = #selector(toggleStatusPopover(_:))
            }

            popover.behavior = .transient
            popover.animates = true
            popover.contentSize = NSSize(width: 320, height: 280)
            popover.contentViewController = NSHostingController(
                rootView: MenuBarPopoverView(
                    store: AppStore.shared,
                    onOpenMainWindow: { [weak self] in
                        self?.popover.performClose(nil)
                        self?.showMainWindow()
                    },
                    onOpenRunInMain: { [weak self] runId in
                        self?.popover.performClose(nil)
                        self?.showMainWindow()
                        Task { await AppStore.shared.openRun(runId) }
                    }
                )
            )
            self.statusItem = item
        } else {
            popover.performClose(nil)
            if let item = statusItem {
                NSStatusBar.system.removeStatusItem(item)
                statusItem = nil
            }
        }
    }

    @objc private func toggleStatusPopover(_ sender: AnyObject?) {
        guard let btn = statusItem?.button else { return }
        if popover.isShown {
            popover.performClose(sender)
        } else {
            Task { await AppStore.shared.refreshRunsList() }
            popover.show(relativeTo: btn.bounds, of: btn, preferredEdge: .minY)
            popover.contentViewController?.view.window?.makeKey()
        }
    }
}

@MainActor
final class AICrossAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        installMainMenu()

        let store = AppStore.shared
        store.onMenuBarVisibilityChanged = { visible in
            AICrossWindowController.shared.updateStatusItem(visible: visible)
        }
        AICrossWindowController.shared.updateStatusItem(visible: store.showMenuBarIcon)
        AICrossWindowController.shared.showMainWindow()

        let openRunArg = Self.parseOpenRunArgument()
        Task {
            await store.bootstrapIfNeeded(initialOpenRunId: openRunArg)
        }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag {
            AICrossWindowController.shared.showMainWindow()
        }
        return true
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    private static func parseOpenRunArgument() -> String? {
        let args = CommandLine.arguments
        if let idx = args.firstIndex(of: "--open-run"), idx + 1 < args.count {
            return args[idx + 1]
        }
        return nil
    }

    // MARK: - 原生 macOS 菜单栏与快捷键（⌘1–⌘5, ⌘R, ⌘,, ⌘W, ⌘Q, 标准编辑）

    private func installMainMenu() {
        let mainMenu = NSMenu()

        // 1. 应用菜单
        let appMenuItem = NSMenuItem()
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "关于 aicross", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(.separator())
        let prefsItem = NSMenuItem(title: "派发设置…", action: #selector(openPreferencesAction), keyEquivalent: ",")
        prefsItem.target = self
        appMenu.addItem(prefsItem)
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "隐藏 aicross", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        let hideOthers = NSMenuItem(title: "隐藏其他", action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        hideOthers.keyEquivalentModifierMask = [.command, .option]
        appMenu.addItem(hideOthers)
        appMenu.addItem(withTitle: "全部显示", action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: "")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "退出 aicross", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appMenuItem.submenu = appMenu
        mainMenu.addItem(appMenuItem)

        // 2. 编辑菜单（支持输入框复制粘贴全选撤销）
        let editMenuItem = NSMenuItem()
        let editMenu = NSMenu(title: "编辑")
        editMenu.addItem(withTitle: "撤销", action: Selector(("undo:")), keyEquivalent: "z")
        editMenu.addItem(withTitle: "重做", action: Selector(("redo:")), keyEquivalent: "Z")
        editMenu.addItem(.separator())
        editMenu.addItem(withTitle: "剪切", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "拷贝", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "粘贴", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(withTitle: "全选", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        editMenuItem.submenu = editMenu
        mainMenu.addItem(editMenuItem)

        // 3. 导航菜单（⌘1–⌘5 与 ⌘R）
        let navMenuItem = NSMenuItem()
        let navMenu = NSMenu(title: "显示")
        for (idx, tab) in PageTab.allCases.enumerated() {
            let item = NSMenuItem(title: tab.label, action: #selector(switchTabAction(_:)), keyEquivalent: "\(idx + 1)")
            item.tag = idx
            item.target = self
            navMenu.addItem(item)
        }
        navMenu.addItem(.separator())
        let refreshItem = NSMenuItem(title: "刷新状态", action: #selector(refreshAction), keyEquivalent: "r")
        refreshItem.target = self
        navMenu.addItem(refreshItem)
        navMenuItem.submenu = navMenu
        mainMenu.addItem(navMenuItem)

        // 4. 窗口菜单
        let windowMenuItem = NSMenuItem()
        let windowMenu = NSMenu(title: "窗口")
        let showMain = NSMenuItem(title: "主窗口", action: #selector(reopenMainWindowAction), keyEquivalent: "0")
        showMain.target = self
        windowMenu.addItem(showMain)
        windowMenu.addItem(withTitle: "最小化", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(withTitle: "关闭窗口", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        windowMenuItem.submenu = windowMenu
        mainMenu.addItem(windowMenuItem)
        NSApp.windowsMenu = windowMenu

        NSApp.mainMenu = mainMenu
    }

    @objc private func openPreferencesAction() {
        AICrossWindowController.shared.showMainWindow()
        AppStore.shared.showSettingsSheet = true
    }

    @objc private func switchTabAction(_ sender: NSMenuItem) {
        let tabs = PageTab.allCases
        guard sender.tag >= 0, sender.tag < tabs.count else { return }
        AICrossWindowController.shared.showMainWindow()
        AppStore.shared.selectTab(tabs[sender.tag])
    }

    @objc private func refreshAction() {
        Task { await AppStore.shared.reloadAll(showToast: true) }
    }

    @objc private func reopenMainWindowAction() {
        AICrossWindowController.shared.showMainWindow()
    }
}

@main
enum AICrossMain {
    @MainActor private static let delegate = AICrossAppDelegate()

    static func main() {
        MainActor.assumeIsolated {
            let app = NSApplication.shared
            app.delegate = delegate
            app.run()
        }
    }
}
