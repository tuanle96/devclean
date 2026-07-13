import Combine
import Foundation
import ServiceManagement

public enum LaunchAtLoginStatus: Equatable, Sendable {
    case notRegistered
    case enabled
    case requiresApproval
    case notFound
}

@MainActor
protocol LaunchAtLoginServicing: AnyObject {
    var status: LaunchAtLoginStatus { get }

    func register() throws
    func unregister() throws
    func openSystemSettings()
}

@MainActor
final class SystemLaunchAtLoginService: LaunchAtLoginServicing {
    private let service = SMAppService.mainApp

    var status: LaunchAtLoginStatus {
        switch service.status {
        case .notRegistered:
            .notRegistered
        case .enabled:
            .enabled
        case .requiresApproval:
            .requiresApproval
        case .notFound:
            .notFound
        @unknown default:
            .notFound
        }
    }

    func register() throws {
        guard status != .enabled, status != .requiresApproval else { return }
        try service.register()
    }

    func unregister() throws {
        guard status == .enabled || status == .requiresApproval else { return }
        try service.unregister()
    }

    func openSystemSettings() {
        SMAppService.openSystemSettingsLoginItems()
    }
}

@MainActor
public final class LaunchAtLoginController: ObservableObject {
    @Published public private(set) var status: LaunchAtLoginStatus
    @Published public private(set) var errorMessage: String?

    private let service: any LaunchAtLoginServicing
    private let defaults: UserDefaults

    public convenience init(defaults: UserDefaults = .standard) {
        self.init(service: SystemLaunchAtLoginService(), defaults: defaults)
    }

    init(service: any LaunchAtLoginServicing, defaults: UserDefaults) {
        self.service = service
        self.defaults = defaults
        status = service.status
    }

    public var isEnabledOrPendingApproval: Bool {
        status == .enabled || status == .requiresApproval
    }

    public func enableByDefaultIfNeeded() {
        guard defaults.object(forKey: PreferenceKeys.launchAtLogin) != nil else {
            setEnabled(true)
            return
        }
        let shouldBeEnabled = defaults.bool(forKey: PreferenceKeys.launchAtLogin)
        guard shouldBeEnabled != isEnabledOrPendingApproval else {
            refresh()
            return
        }
        setEnabled(shouldBeEnabled)
    }

    public func setEnabled(_ enabled: Bool) {
        errorMessage = nil
        do {
            if enabled {
                try service.register()
            } else {
                try service.unregister()
            }
            defaults.set(enabled, forKey: PreferenceKeys.launchAtLogin)
        } catch {
            errorMessage = "Could not update Launch at Login: \(error.localizedDescription)"
        }
        refresh()
    }

    public func refresh() {
        status = service.status
    }

    public func openSystemSettings() {
        service.openSystemSettings()
    }
}
