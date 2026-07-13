import SwiftUI

/// Presentation helpers shared across the menu bar UI: relative dates, safety-hold
/// expiry, section accent colors, and the disk capacity bar model. Keeping these in
/// one place stops the views from re-deriving the same formatting inline.
enum UIFormatting {
    /// Short relative phrase like "2 min ago" for a unix timestamp.
    static func relativePast(_ unix: UInt64, now: Date = Date()) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(unix))
        return date.formatted(.relative(presentation: .named))
    }

    static func relativePast(_ date: Date, now: Date = Date()) -> String {
        date.formatted(.relative(presentation: .named))
    }

    /// Whole days between now and a future expiry timestamp (clamped at 0).
    static func daysUntil(_ expiresAtUnix: UInt64, now: Date = Date()) -> Int {
        let expires = Date(timeIntervalSince1970: TimeInterval(expiresAtUnix))
        let seconds = expires.timeIntervalSince(now)
        return max(0, Int((seconds / 86_400).rounded(.up)))
    }

    /// Human phrase for how long a safety hold has left before it can be purged.
    static func expiryText(_ expiresAtUnix: UInt64, now: Date = Date()) -> String {
        let days = daysUntil(expiresAtUnix, now: now)
        switch days {
        case 0: return "Expires today"
        case 1: return "Expires tomorrow"
        default: return "Expires in \(days) days"
        }
    }

    /// Orange when the hold is about to be purged, otherwise the calm archive teal.
    static func expiryColor(_ expiresAtUnix: UInt64, now: Date = Date()) -> Color {
        daysUntil(expiresAtUnix, now: now) <= 1 ? .orange : .teal
    }

    /// Relative age of an artifact ("built 2 months ago") — the most persuasive
    /// reason to reclaim it. Nil when the scanner reported no modification time.
    static func ageText(_ modifiedAtUnix: UInt64?, now: Date = Date()) -> String? {
        guard let modifiedAtUnix else { return nil }
        return "built " + relativePast(modifiedAtUnix, now: now)
    }

    /// Directory names that describe a role inside a project rather than the
    /// project itself — "services/target" belongs to the folder above
    /// "services", not to "services".
    private static let genericProjectDirectories: Set<String> = [
        "services", "service", "backend", "frontend", "packages", "apps",
        "libs", "modules", "crates", "server", "client", "ios", "android", "src",
    ]

    /// Top-level container folders that are never a project name themselves,
    /// so a generic member directly under one keeps its own name.
    private static let containerDirectories: Set<String> = [
        "dev", "projects", "code", "repos", "work", "sites",
    ]

    /// Name of the project that owns an artifact, best source first: the
    /// scanner-provided review root, then a recognized workspace root
    /// containing the path, then the parent directory — skipping one generic
    /// member folder ("services", "backend", …) when a real owner sits above.
    static func projectName(
        forPath path: String,
        projectRoot: String? = nil,
        workspaceRoots: [String] = []
    ) -> String {
        if let projectRoot, !projectRoot.isEmpty {
            return URL(fileURLWithPath: projectRoot).lastPathComponent
        }
        let containingWorkspaces = workspaceRoots.filter {
            path == $0 || path.hasPrefix($0 + "/")
        }
        if let workspace = containingWorkspaces.max(by: { $0.count < $1.count }) {
            return URL(fileURLWithPath: workspace).lastPathComponent
        }
        let parent = URL(fileURLWithPath: path).deletingLastPathComponent()
        let parentName = parent.lastPathComponent
        // ponytail: skip a single generic level; deeper monorepo nesting is the
        // workspace roots' job.
        if genericProjectDirectories.contains(parentName.lowercased()) {
            let grandparent = parent.deletingLastPathComponent()
            let grandparentName = grandparent.lastPathComponent
            let isContainer =
                containerDirectories.contains(grandparentName.lowercased())
                || grandparent.path == "/"
                || grandparent.path == FileManager.default.homeDirectoryForCurrentUser.path
            if !isContainer, !grandparentName.isEmpty {
                return grandparentName
            }
        }
        return parentName
    }
}

/// Small capsule for row metadata like artifact age or hold expiry.
struct MetaChip: View {
    let text: String
    var tint: Color = .secondary

    var body: some View {
        Text(text)
            .font(.caption2.weight(.medium))
            .foregroundStyle(tint)
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(.quaternary.opacity(0.6), in: Capsule())
            .fixedSize()
    }
}

/// One slice of the disk capacity bar shown in the header.
struct CapacitySegment: Identifiable {
    let id: String
    let label: String
    let bytes: UInt64
    let color: Color
}

enum CapacityBar {
    /// Builds honest disk segments (reclaimable · held · other used · free) from the
    /// volume totals. Returns nil when total capacity is unknown so the header can
    /// fall back to the plain headline.
    static func segments(
        total: UInt64?,
        free: UInt64?,
        held: UInt64,
        reclaimable: UInt64
    ) -> [CapacitySegment]? {
        guard let total, total > 0, let free else { return nil }
        // Held and reclaimable artifacts both occupy disk now, so both are part of
        // "used". Clamp every slice into [0, total] so the segments always sum to
        // exactly `total` and the bar can never overflow its track — even when scan
        // roots sit on another volume and report more than this volume's used space.
        let clampedFree = min(free, total)
        let used = total - clampedFree
        let clampedHeld = min(held, used)
        let clampedReclaimable = min(reclaimable, used - clampedHeld)
        let usedByOthers = used - clampedHeld - clampedReclaimable
        return [
            CapacitySegment(
                id: "reclaimable", label: "cleanable", bytes: clampedReclaimable, color: .accentColor),
            CapacitySegment(id: "held", label: "held", bytes: clampedHeld, color: .teal),
            CapacitySegment(
                id: "other",
                label: "used",
                bytes: usedByOthers,
                color: Color(nsColor: .quaternaryLabelColor)
            ),
            CapacitySegment(id: "free", label: "free", bytes: clampedFree, color: Color(nsColor: .separatorColor)),
        ].filter { $0.bytes > 0 }
    }
}
