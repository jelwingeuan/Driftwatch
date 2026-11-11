import Foundation
import Testing
@testable import Driftwatch

@Test func decodesHistory() throws {
    let json = """
    {"entries":[{"changes":[{"added_by":"Google","category":"launchd","confidence":"likely",\
    "field_labels":{"Label":"Name","file_sha256":"File contents"},"fields":{"Label":"a","file_sha256":"new"},\
    "key":"/a.plist","kind":"modified","old_fields":{"Label":"a","file_sha256":"old"},\
    "risk":"medium","risk_reason":"Runs with full control of your Mac (as root).",\
    "summary":"Starts a program every time you log in.","team_id":"ABCDE12345","title":"a"}],\
    "snapshot_id":4,"taken_at":"2026-11-09T15:45:29Z"}],\
    "errors":[{"category":"tcc","fields":{"error":"needs Full Disk Access"},"key":"/TCC.db",\
    "team_id":null,"title":"TCC.db"}],"items_total":3,\
    "categories":[{"category":"launchd","items":2},{"category":"tcc","items":1}],\
    "latest":{"snapshot_id":4,"taken_at":"2026-11-09T15:45:29Z"}}
    """
    let history = try decodeCLIOutput(History.self, from: Data(json.utf8))
    #expect(history.itemsTotal == 3)
    #expect(history.items(in: "launchd") == 2)
    #expect(history.items(in: "cron") == 0)
    #expect(history.latest?.takenAt == Date(timeIntervalSince1970: 1_794_239_129))
    #expect(history.needsFullDiskAccess)
    let change = try #require(history.rows.first?.change)
    #expect(change.kind == .modified)
    #expect(change.teamID == "ABCDE12345")
    // Field names keep their snake_case spelling.
    #expect(change.oldFields?["file_sha256"] == "old")
    #expect(change.label("file_sha256") == "File contents")
    #expect(change.label("unknown") == "unknown")
    #expect(change.risk == "medium")
    #expect(change.addedByText == "Added by Google (likely)")
    // Output without the explain fields (the error item) still decodes.
    #expect(history.errors.count == 1)
}

/// One scan with a launchd change for each (kind, plist path) and a BTM change
/// for each (kind, url).
private func entry(launchd: [(String, String)], btm: [(String, String)]) throws -> Entry {
    let changes = launchd.map { kind, path in
        #"{"category":"launchd","fields":{},"key":"\#(path)","kind":"\#(kind)","title":"x"}"#
    } + btm.map { kind, url in
        #"{"category":"btm","fields":{"url":"\#(url)"},"key":"uid 501: \#(url)","kind":"\#(kind)","title":"x"}"#
    }
    let json = #"{"snapshot_id":2,"taken_at":"2026-11-09T15:45:29Z","changes":[\#(changes.joined(separator: ","))]}"#
    return try decodeCLIOutput(Entry.self, from: Data(json.utf8))
}

@Test func btmRecordOfAChangedStartupItemIsNotCountedTwice() throws {
    let home = NSHomeDirectory()
    let e = try entry(
        launchd: [
            ("added", "\(home)/Library/LaunchAgents/x.plist"),
            ("added", "\(home)/Library/LaunchAgents/n.plist"),
            ("added", "/Library/LaunchAgents/My Agent.plist"),
        ],
        btm: [
            ("added", "/Users/\(getuid())/Library/LaunchAgents/x.plist"),
            ("added", "/Users/\(NSUserName())/Library/LaunchAgents/n.plist"),
            ("added", "file:///Library/LaunchAgents/My%20Agent.plist"),
            ("added", "/Applications/Y.app"),
        ]
    )
    #expect(e.distinctChanges.map(\.category) == ["launchd", "launchd", "launchd", "btm"])
    #expect(e.distinctChanges.last?.fields["url"] == "/Applications/Y.app")
    #expect(e.rows.count == 4)
}

@Test func btmChangeIsShownUnlessSamePlistAndSameKind() throws {
    let e = try entry(
        launchd: [
            ("added", "/Library/LaunchAgents/x.plist"),
            ("added", "/Library/LaunchAgents/y.plist"),
            ("added", "\(NSHomeDirectory())/Library/LaunchAgents/z.plist"),
        ],
        btm: [
            // Same file name, other folder.
            ("added", "/Library/LaunchDaemons/x.plist"),
            // Same plist, other kind.
            ("removed", "/Library/LaunchAgents/y.plist"),
            // Another user's home folder.
            ("added", "/Users/\(getuid() + 1)/Library/LaunchAgents/z.plist"),
        ]
    )
    #expect(e.distinctChanges.count == 6)
}

@Test func decodesScanAndEmptyHistory() throws {
    let scan = """
    {"baseline":true,"changes":[],"snapshot_id":1,"taken_at":"2026-11-09T15:22:02Z"}
    """
    #expect(try decodeCLIOutput(ScanResult.self, from: Data(scan.utf8)).baseline)
    // Output from before the per-category counts still decodes.
    let empty = #"{"entries":[],"errors":[],"items_total":0,"latest":null}"#
    let history = try decodeCLIOutput(History.self, from: Data(empty.utf8))
    #expect(history.latest == nil)
    #expect(history.items(in: "launchd") == nil)
}

/// The Rust debug build, when it has been built (`cargo build` at the repo root).
private let cliBuild = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()
    .appendingPathComponent("../../../target/debug/driftwatch")
    .standardizedFileURL

@Test(.enabled(if: FileManager.default.isExecutableFile(atPath: cliBuild.path)))
func decodesRealHistoryOutput() async throws {
    setenv("DRIFTWATCH_CLI", cliBuild.path, 1)
    let history = try await CLI.run(["history", "--json"], as: History.self)
    #expect(history.itemsTotal >= 0)
}

@Test func scheduledScansSkipLoginItemsForADayAfterAFullScan() {
    let now = Date(timeIntervalSince1970: 1_794_239_129)
    let hour: TimeInterval = 60 * 60
    // Scan Now always reads them.
    #expect(!AppModel.skipsLoginItems(scheduled: false, lastFullScan: now, now: now))
    // Never read yet, or read over a day ago: the scheduled scan reads them too.
    #expect(!AppModel.skipsLoginItems(scheduled: true, lastFullScan: nil, now: now))
    #expect(!AppModel.skipsLoginItems(scheduled: true, lastFullScan: now - 25 * hour, now: now))
    #expect(AppModel.skipsLoginItems(scheduled: true, lastFullScan: now - 23 * hour, now: now))
}

@Test func aFailedOrOldScanShowsAWarning() {
    let now = Date(timeIntervalSince1970: 1_794_239_129)
    let minute: TimeInterval = 60
    #expect(AppModel.scanWarning(error: nil, lastScan: now - 59 * minute, isScanning: false, now: now) == nil)
    let failed = AppModel.scanWarning(error: "It took longer than 10 minutes and was stopped.", lastScan: now, isScanning: false, now: now)
    #expect(failed == "Scan failed. It took longer than 10 minutes and was stopped.")
    let old = AppModel.scanWarning(error: nil, lastScan: now - 61 * minute, isScanning: false, now: now)
    #expect(old?.hasPrefix("Scan is out of date.") == true)
    // A running scan is about to catch up.
    #expect(AppModel.scanWarning(error: nil, lastScan: now - 61 * minute, isScanning: true, now: now) == nil)
}

@Test func unreadableItemsShowTheirSummaryAndNeverTheRedRemovedSymbol() throws {
    func change(_ kind: String) throws -> Change {
        let json = #"{"category":"btm","fields":{"error":"sfltool failed (needs admin rights)"},"key":"sfltool dumpbtm","kind":"\#(kind)","title":"Couldn’t read login items","summary":"This part needs your admin password."}"#
        return try decodeCLIOutput(Change.self, from: Data(json.utf8))
    }
    let failed = try change("added")
    #expect(failed.subtitle == "This part needs your admin password.")
    #expect(failed.status.symbol == "exclamationmark.triangle")
    let resolved = try change("removed")
    #expect(resolved.subtitle == "This problem has gone away.")
    #expect(resolved.status.symbol == "checkmark.circle")
    let removed = try decodeCLIOutput(
        Change.self, from: Data(#"{"category":"cron","fields":{},"key":"job","kind":"removed","title":"job"}"#.utf8))
    #expect(removed.status.symbol == "minus.circle")
}

/// A history whose latest scan is `now`, with one scan per (age, risks) pair.
private func history(
    now: Date, scans: [(age: TimeInterval, risks: [String])], errors: [History.Problem] = [], itemsTotal: Int = 412
) throws -> History {
    let entries = try scans.enumerated().map { i, scan in
        let changes = try scan.risks.enumerated().map { j, risk in
            let json = #"{"category":"launchd","fields":{},"key":"/\#(i)-\#(j).plist","kind":"added","title":"x","risk":"\#(risk)"}"#
            return try decodeCLIOutput(Change.self, from: Data(json.utf8))
        }
        return Entry(snapshotID: Int64(10 - i), takenAt: now - scan.age, changes: changes)
    }
    return History(latest: .init(snapshotID: 10, takenAt: now), itemsTotal: itemsTotal, categories: nil,
                   errors: errors, entries: entries)
}

@Test func verdictCountsRiskyChangesFromTheLastSevenDays() throws {
    let now = Date(timeIntervalSince1970: 1_794_239_129)
    let day: TimeInterval = 24 * 60 * 60
    // Low risk counts for nothing; high risk 8 days ago is outside the window.
    let quiet = try history(now: now, scans: [(1 * day, ["low"]), (8 * day, ["high"])])
    #expect(Verdict.from(quiet, scanWarning: nil, now: now) == .quiet)
    let look = try history(now: now, scans: [(1 * day, ["medium", "low"]), (5 * day, ["high"])])
    #expect(Verdict.from(look, scanWarning: nil, now: now) == .worthALook(2))
    // Inclusive: exactly 7 days old still counts, a second more does not.
    let edge = try history(now: now, scans: [(7 * day, ["high"])])
    #expect(Verdict.from(edge, scanWarning: nil, now: now) == .worthALook(1))
    let past = try history(now: now, scans: [(7 * day + 1, ["high"])])
    #expect(Verdict.from(past, scanWarning: nil, now: now) == .quiet)
}

@Test func verdictIsNeverQuietWhenDriftwatchCantSeeEverything() throws {
    let now = Date(timeIntervalSince1970: 1_794_239_129)
    let quiet = try history(now: now, scans: [])
    #expect(Verdict.from(quiet, scanWarning: nil, now: now) == .quiet)
    // No timeline (not read yet, or it failed to decode), no scan yet, or an empty scan.
    #expect(Verdict.from(nil, scanWarning: nil, now: now) == .cantSee)
    let noScan = History(latest: nil, itemsTotal: 0, categories: nil, errors: [], entries: [])
    #expect(Verdict.from(noScan, scanWarning: nil, now: now) == .cantSee)
    #expect(Verdict.from(try history(now: now, scans: [], itemsTotal: 0), scanWarning: nil, now: now) == .cantSee)
    // A failed scan.
    let failed = AppModel.scanWarning(error: "The scan stopped unexpectedly.", lastScan: now, isScanning: false, now: now)
    #expect(Verdict.from(quiet, scanWarning: failed, now: now) == .cantSee)
    // An old scan, even while a new scan runs (when there is no warning yet).
    #expect(Verdict.from(quiet, scanWarning: nil, now: now + 61 * 60) == .cantSee)
    // Unreadable items win over risky changes.
    let unreadable = try history(
        now: now, scans: [(60, ["high"])], errors: [.init(category: "tcc", fields: ["error": "needs Full Disk Access"])])
    #expect(Verdict.from(unreadable, scanWarning: nil, now: now) == .cantSee)
}

@Test func verdictSentencesNameTheWindow() {
    #expect(Verdict.quiet.sentence == "No risky changes in the last 7 days.")
    #expect(Verdict.worthALook(1).sentence == "1 risky change in the last 7 days.")
    #expect(Verdict.worthALook(2).sentence == "2 risky changes in the last 7 days.")
    #expect(Verdict.cantSee.sentence == "Driftwatch can’t see everything right now.")
}
