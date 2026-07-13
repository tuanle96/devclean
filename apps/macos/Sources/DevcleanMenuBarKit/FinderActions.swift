import AppKit

/// Standard Mac affordances for anything with a filesystem path. Every row that
/// shows a path offers these through its context menu.
enum FinderActions {
    static func revealInFinder(_ path: String) {
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }

    static func copyPath(_ path: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(path, forType: .string)
    }
}
