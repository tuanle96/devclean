import AppKit
import Foundation

@MainActor
enum SettingsWindowFocusCoordinator {
    static func activateAfterSettingsLink() {
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(100))
            activateWhenAvailable()
        }
    }

    static func activateWhenAvailable() {
        NSApp.activate(ignoringOtherApps: true)
        if focusExistingWindow() { return }

        Task { @MainActor in
            for delay in [50, 100, 200, 350] {
                try? await Task.sleep(for: .milliseconds(delay))
                NSApp.activate(ignoringOtherApps: true)
                if focusExistingWindow() { return }
            }
        }
    }

    @discardableResult
    static func focusExistingWindow() -> Bool {
        let windows = NSApp.windows
        guard let window = settingsWindow(in: windows) else { return false }
        for panel in menuPanels(in: windows) {
            panel.orderOut(nil)
        }
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        window.orderFrontRegardless()
        return true
    }

    static func settingsWindow(in windows: [NSWindow]) -> NSWindow? {
        windows.first { window in
            !(window is NSPanel)
                && window.canBecomeKey
                && window.styleMask.contains(.titled)
        }
    }

    static func menuPanels(in windows: [NSWindow]) -> [NSPanel] {
        windows.compactMap { $0 as? NSPanel }
    }
}
