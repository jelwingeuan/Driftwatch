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
    var rows: [Row] { changes.map { Row(snapshotID: snapshotID, takenAt: takenAt, change: $0) } }

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

    enum CodingKeys: String, CodingKey {
        case kind, category, key, title, fields, oldFields = "old_fields", teamID = "team_id"
    }

    /// Why the item could not be read, if it could not.
    var error: String? { fields["error"] }
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

/// A sidebar entry. `id` is the CLI's category name ("all" for everything);
/// `alsoShows` lists other CLI categories that belong under it.
struct Category: Identifiable, Hashable {
    let id: String
    let name: String
    let symbol: String
    var alsoShows: Set<String> = []

    static let all: [Category] = [
        Category(id: "all", name: "All changes", symbol: "clock.arrow.circlepath"),
        Category(id: "launchd", name: "Startup items", symbol: "power", alsoShows: ["btm"]),
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
        id == "all" || id == change.category || alsoShows.contains(change.category)
    }
}
