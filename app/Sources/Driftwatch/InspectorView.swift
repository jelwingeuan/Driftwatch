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
                            Text(key).font(.headline)
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

            Section(change.kind == .removed ? "Last known values" : "Values") {
                ForEach(change.fields.keys.sorted(), id: \.self) { key in
                    LabeledContent(key) {
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
