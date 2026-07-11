import DevcleanMenuBarKit
import SwiftUI

@main
struct DevcleanMenuBarApp: App {
    @StateObject private var model: AppModel

    init() {
        let model = AppModel()
        _model = StateObject(wrappedValue: model)
        model.startBackgroundMonitoring()
    }

    var body: some Scene {
        MenuBarExtra {
            MenuContentView(model: model)
        } label: {
            Label("Devclean", systemImage: model.menuBarSymbol)
        }
        .menuBarExtraStyle(.window)

          Settings {
              SettingsView(model: model)
          }
    }
}
