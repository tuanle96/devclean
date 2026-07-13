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
