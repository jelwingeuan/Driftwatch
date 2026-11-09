import Foundation
import SwiftUI

// The JSON the `driftwatch` CLI prints (`scan --json`, `history --json`).
// Keys are mapped by hand: `.convertFromSnakeCase` would also rename the keys
// inside `fields`.

struct ScanResult: Decodable {
    let snapshotID: Int64
    let takenAt: Date
    let baseline: Bool
    let changes: [Change]

    enum CodingKeys: String, CodingKey {
        case snapshotID = "snapshot_id", takenAt = "taken_at", baseline, changes
    }
}

struct History: Decodable {
    struct Scan: Decodable {
        let snapshotID: Int64
        let takenAt: Date

        enum CodingKeys: String, CodingKey {
            case snapshotID = "snapshot_id", takenAt = "taken_at"
        }
    }

    /// An item in the latest scan that could not be read.
    struct Problem: Decodable {
        let category: String
        let fields: [String: String]
    }

    let latest: Scan?
    let itemsTotal: Int
    let errors: [Problem]
    let entries: [Entry]

    enum CodingKeys: String, CodingKey {
        case latest, itemsTotal = "items_total", errors, entries
    }

    /// The privacy (TCC) database could not be opened, which means no Full Disk Access.
    var needsFullDiskAccess: Bool { errors.contains { $0.category == "tcc" } }

    /// Every change, newest scan first.
    var rows: [Row] { entries.flatMap(\.rows) }
}

/// One scan that changed something compared with the scan before it.
struct Entry: Decodable, Identifiable {
    let snapshotID: Int64
    let takenAt: Date
    let changes: [Change]

    var id: Int64 { snapshotID }
    var rows: [Row] { distinctChanges.map { Row(snapshotID: snapshotID, takenAt: takenAt, change: $0) } }

    /// The changes minus login & background (BTM) records that repeat a startup
    /// item changed in the same scan: BTM also lists every launch agent/daemon
    /// plist, so one item would otherwise show and count twice. A BTM change is
    /// dropped only when its url is the same plist file, changed the same way.
    var distinctChanges: [Change] {
        let launchd = Set(changes.filter { $0.category == "launchd" }.map { [$0.kind.rawValue, $0.key] })
        return changes.filter { change in
            guard change.category == "btm", let path = change.btmPath else { return true }
            return !launchd.contains([change.kind.rawValue, path])
        }
    }

    enum CodingKeys: String, CodingKey {
        case snapshotID = "snapshot_id", takenAt = "taken_at", changes
    }
}

struct Change: Decodable, Hashable {
    enum Kind: String, Decodable {
        case added, removed, modified
    }

    let kind: Kind
    let category: String
    let key: String
    let title: String
    let fields: [String: String]
    let oldFields: [String: String]?
    let teamID: String?
    // From the CLI's explain step; optional so older output still decodes.
    let summary: String?
    let risk: String?
    let riskReason: String?
    let addedBy: String?
    let confidence: String?
    let fieldLabels: [String: String]?

    enum CodingKeys: String, CodingKey {
        case kind, category, key, title, fields, oldFields = "old_fields", teamID = "team_id"
        case summary, risk, riskReason = "risk_reason", addedBy = "added_by", confidence
        case fieldLabels = "field_labels"
    }

    /// Why the item could not be read, if it could not.
    var error: String? { fields["error"] }

    /// The plain label for a raw field name.
    func label(_ field: String) -> String { fieldLabels?[field] ?? field }

    /// "Added by Google", with "likely" when only guessed from a name or path.
    var addedByText: String? {
        guard let addedBy else { return nil }
        return confidence == "likely" ? "Added by \(addedBy) (likely)" : "Added by \(addedBy)"
    }

    /// Hashes mean nothing to a person; they stay in Details.
    static let hashFields: Set<String> = ["file_sha256", "cdhash", "sha1"]

    /// The file a BTM record's url names, spelled the way the launchd scan spells
    /// paths: file:// and %-escapes decoded, and this user's "/Users/<uid or name>/"
    /// turned into the real home folder.
    var btmPath: String? {
        guard let url = fields["url"] else { return nil }
        let path = (url.hasPrefix("file://") ? URL(string: url)?.path : url.removingPercentEncoding) ?? url
        for alias in ["/Users/\(getuid())/", "/Users/\(NSUserName())/"] where path.hasPrefix(alias) {
            return NSHomeDirectory() + "/" + path.dropFirst(alias.count)
        }
        return path
    }
}

/// A change together with the scan that found it.
struct Row: Identifiable, Hashable {
    let snapshotID: Int64
    let takenAt: Date
    let change: Change

    var id: String { "\(snapshotID)\u{1F}\(change.category)\u{1F}\(change.key)" }
}

func decodeCLIOutput<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
    let decoder = JSONDecoder()
    decoder.dateDecodingStrategy = .iso8601
    return try decoder.decode(type, from: data)
}

extension Change.Kind {
    var label: String {
        switch self {
        case .added: "Added"
        case .removed: "Removed"
        case .modified: "Changed"
        }
    }

    var symbol: String {
        switch self {
        case .added: "plus.circle"
        case .removed: "minus.circle"
        case .modified: "pencil.circle"
        }
    }

    var tint: Color {
        switch self {
        case .added: .green
        case .removed: .red
        case .modified: .orange
        }
    }
}

/// A sidebar entry. `id` is the CLI's category name ("all" for everything).
struct Category: Identifiable, Hashable {
    let id: String
    let name: String
    let symbol: String

    static let all: [Category] = [
        Category(id: "all", name: "All changes", symbol: "clock.arrow.circlepath"),
        Category(id: "launchd", name: "Startup items", symbol: "power"),
        Category(id: "btm", name: "Login & background", symbol: "person.crop.circle.badge.clock"),
        Category(id: "cron", name: "Scheduled", symbol: "calendar.badge.clock"),
        Category(id: "shell", name: "Shell startup", symbol: "terminal"),
        Category(id: "sysext", name: "System extensions", symbol: "puzzlepiece.extension"),
        Category(id: "tcc", name: "Privacy permissions", symbol: "hand.raised"),
        Category(id: "certs", name: "Certificates", symbol: "checkmark.seal"),
        Category(id: "network", name: "Network", symbol: "network"),
        Category(id: "ssh", name: "SSH", symbol: "key"),
        Category(id: "apps", name: "Apps", symbol: "app"),
    ]

    /// The display entry for a CLI category name.
    static func named(_ category: String) -> Category {
        all.first { $0.id == category }
            ?? Category(id: category, name: category, symbol: "questionmark.circle")
    }

    func contains(_ change: Change) -> Bool {
        id == "all" || id == change.category
    }
}
