import Foundation

/// Persists the last successful scan report so a relaunched app shows real
/// content immediately instead of blocking every tab behind the first scan.
/// This is presentation state only: cleanup always revalidates each exact path
/// in the Rust CLI, so a stale cached candidate can never be deleted.
struct ReportCache {
    struct Entry: Codable {
        let savedAt: Date
        let report: ScanReport
    }

    private let url: URL

    init(
        directory: URL = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Devclean", isDirectory: true)
    ) {
        url = directory.appendingPathComponent("last-scan.json")
    }

    func load() -> Entry? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        // A schema change simply drops the cache; the next scan rewrites it.
        return try? JSONDecoder().decode(Entry.self, from: data)
    }

    func save(report: ScanReport, at date: Date = Date()) {
        guard let data = try? JSONEncoder().encode(Entry(savedAt: date, report: report)) else {
            return
        }
        try? FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try? data.write(to: url, options: .atomic)
    }
}
