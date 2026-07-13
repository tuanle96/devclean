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
            Label("DevCleaner", systemImage: model.menuBarSymbol)
        }
        .menuBarExtraStyle(.window)

        Settings {
            SettingsView(model: model, launchAtLogin: launchAtLogin)
        }
        .windowResizability(.contentSize)
    }
}
