import Foundation

enum DefaultScanLocations {
    // Keep this list in sync with DEFAULT_ROOT_CANDIDATES in src/scanner/roots.rs.
    static let relativePaths = [
        "Dev",
        "Developer",
        "Projects",
        "Code",
        "src",
        "workspace",
        "Workspaces",
        "Repos",
        "Repositories",
        "GitHub",
        "Documents/GitHub",
        "AndroidStudioProjects",
        "IdeaProjects",
    ]

    static func detect(
        homeDirectory: URL = FileManager.default.homeDirectoryForCurrentUser,
        fileManager: FileManager = .default
    ) -> [String] {
        var locations: [(display: String, canonical: String)] = []
        for relativePath in relativePaths {
            let url = homeDirectory.appendingPathComponent(relativePath, isDirectory: true)
            var isDirectory: ObjCBool = false
            guard
                fileManager.fileExists(atPath: url.path, isDirectory: &isDirectory),
                isDirectory.boolValue
            else { continue }

            let display = url.standardizedFileURL.path
            let canonical = url.resolvingSymlinksInPath().standardizedFileURL.path
            if locations.contains(where: { isEqualOrDescendant(canonical, of: $0.canonical) }) {
                continue
            }
            locations.removeAll(where: { isEqualOrDescendant($0.canonical, of: canonical) })
            locations.append((display, canonical))
        }
        return locations.map(\.display)
    }

    private static func isEqualOrDescendant(_ path: String, of root: String) -> Bool {
        path == root || path.hasPrefix(root + "/")
    }
}
