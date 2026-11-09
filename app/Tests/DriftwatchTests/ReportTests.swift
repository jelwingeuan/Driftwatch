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
    "latest":{"snapshot_id":4,"taken_at":"2026-11-09T15:45:29Z"}}
    """
    let history = try decodeCLIOutput(History.self, from: Data(json.utf8))
    #expect(history.itemsTotal == 3)
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

@Test func btmRecordOfAChangedStartupItemIsNotCountedTwice() throws {
    let json = """
    {"snapshot_id":2,"taken_at":"2026-11-09T15:45:29Z","changes":[\
    {"category":"btm","fields":{"url":"/Users/501/Library/LaunchAgents/x.plist"},"key":"uid 501: 8.x",\
    "kind":"added","old_fields":null,"team_id":null,"title":"x"},\
    {"category":"btm","fields":{"url":"/Applications/Y.app"},"key":"uid 501: 2.y",\
    "kind":"added","old_fields":null,"team_id":null,"title":"Y"},\
    {"category":"launchd","fields":{"Label":"x"},"key":"/Users/me/Library/LaunchAgents/x.plist",\
    "kind":"added","old_fields":null,"team_id":null,"title":"x"}]}
    """
    let entry = try decodeCLIOutput(Entry.self, from: Data(json.utf8))
    #expect(entry.distinctChanges.map(\.category) == ["btm", "launchd"])
    #expect(entry.rows.count == 2)
}

@Test func decodesScanAndEmptyHistory() throws {
    let scan = """
    {"baseline":true,"changes":[],"snapshot_id":1,"taken_at":"2026-11-09T15:22:02Z"}
    """
    #expect(try decodeCLIOutput(ScanResult.self, from: Data(scan.utf8)).baseline)
    let empty = #"{"entries":[],"errors":[],"items_total":0,"latest":null}"#
    #expect(try decodeCLIOutput(History.self, from: Data(empty.utf8)).latest == nil)
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
