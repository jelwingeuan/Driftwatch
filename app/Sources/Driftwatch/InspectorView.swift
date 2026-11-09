import AppKit
import SwiftUI

/// Everything Driftwatch knows about the selected change.
struct InspectorView: View {
    let row: Row?
    @State private var showDetails = false

    var body: some View {
        if let row {
            content(row)
        } else {
            ContentUnavailableView(
                "No Selection",
                systemImage: "sidebar.trailing",
                description: Text("Select a change to see what it is.")
            )
        }
    }

    private func content(_ row: Row) -> some View {
        let change = row.change
        return Form {
            Section {
                VStack(alignment: .leading, spacing: 6) {
                    Text(change.title)
                        .font(.title3.weight(.semibold))
                        .textSelection(.enabled)
                    if let summary = change.summary {
                        Text(summary).textSelection(.enabled)
                    }
                    RiskBadge(change: change)
                    if change.risk == "medium" || change.risk == "high", let reason = change.riskReason {
                        Text(reason).foregroundStyle(.secondary)
                    }
                    if let addedBy = change.addedByText {
                        Text(addedBy).foregroundStyle(.secondary)
                    }
                    Label(change.kind.label, systemImage: change.kind.symbol)
                        .foregroundStyle(change.kind.tint)
                    Label(Category.named(change.category).name, systemImage: Category.named(change.category).symbol)
                        .foregroundStyle(.secondary)
                    Text("Found \(row.takenAt.formatted(date: .abbreviated, time: .shortened))")
                        .foregroundStyle(.secondary)
                }
                if let error = change.error {
                    Label(error, systemImage: "exclamationmark.triangle")
                }
                if let url = fileURL(change) {
                    Button("Show in Finder") {
                        NSWorkspace.shared.activateFileViewerSelecting([url])
                    }
                }
            }

            if let old = change.oldFields {
                Section("What changed") {
                    ForEach(changedKeys(old, change.fields), id: \.self) { key in
                        VStack(alignment: .leading, spacing: 4) {
                            Text(change.label(key)).font(.headline)
                            if Change.hashFields.contains(key) {
                                Text("Changed").foregroundStyle(.secondary)
                            } else {
                                Grid(alignment: .leadingFirstTextBaseline, verticalSpacing: 2) {
                                    GridRow {
                                        Text("Before").foregroundStyle(.secondary)
                                        Text(old[key] ?? "(none)").textSelection(.enabled)
                                    }
                                    GridRow {
                                        Text("Now").foregroundStyle(.secondary)
                                        Text(change.fields[key] ?? "(none)").textSelection(.enabled)
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Section(change.kind == .removed ? "Last known values" : "Values") {
                ForEach(change.fields.keys.filter { !Change.hashFields.contains($0) }.sorted(), id: \.self) { key in
                    LabeledContent(change.label(key)) {
                        Text(change.fields[key] ?? "").textSelection(.enabled)
                    }
                }
            }

            Section {
                DisclosureGroup("Details", isExpanded: $showDetails) {
                    LabeledContent("Category", value: change.category)
                    LabeledContent("Key") { Text(change.key).textSelection(.enabled) }
                    LabeledContent("Team ID", value: change.teamID ?? "None")
                    LabeledContent("Scan", value: "#\(row.snapshotID)")
                    ForEach(change.fields.keys.sorted(), id: \.self) { key in
                        LabeledContent(key) { Text(change.fields[key] ?? "").textSelection(.enabled) }
                    }
                    if let old = change.oldFields {
                        ForEach(changedKeys(old, change.fields), id: \.self) { key in
                            LabeledContent("\(key) before") { Text(old[key] ?? "(none)").textSelection(.enabled) }
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
    }

    /// The item's file, when its key is a path that still exists.
    private func fileURL(_ change: Change) -> URL? {
        guard change.key.hasPrefix("/"), FileManager.default.fileExists(atPath: change.key) else { return nil }
        return URL(fileURLWithPath: change.key)
    }

    private func changedKeys(_ old: [String: String], _ new: [String: String]) -> [String] {
        Set(old.keys).union(new.keys).filter { old[$0] != new[$0] }.sorted()
    }
}
