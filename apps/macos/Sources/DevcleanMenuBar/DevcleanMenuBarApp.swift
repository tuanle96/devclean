import DevcleanMenuBarKit
import SwiftUI

@main
struct DevcleanMenuBarApp: App {
    @StateObject private var model: AppModel
    @StateObject private var launchAtLogin: LaunchAtLoginController

    init() {
        let model = AppModel()
        let launchAtLogin = LaunchAtLoginController()
        _model = StateObject(wrappedValue: model)
        _launchAtLogin = StateObject(wrappedValue: launchAtLogin)
        model.startBackgroundMonitoring()
        launchAtLogin.enableByDefaultIfNeeded()
    }

    var body: some Scene {
        MenuBarExtra {
            MenuContentView(model: model)
        } label: {
            if #available(macOS 14.0, *) {
                Label("DevCleaner", systemImage: model.menuBarSymbol)
                    .symbolEffect(.pulse, isActive: model.isBusy)
            } else {
                Label("DevCleaner", systemImage: model.menuBarSymbol)
            }
        }
        .menuBarExtraStyle(.window)

        Settings {
            SettingsView(model: model, launchAtLogin: launchAtLogin)
        }
        .windowResizability(.contentSize)
    }
}
