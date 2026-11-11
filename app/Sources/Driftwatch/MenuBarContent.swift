import AppKit
import SwiftUI

/// The popover under the menu bar icon.
struct MenuBarContent: View {
    let model: AppModel
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    if model.history?.latest != nil {
                        TimelineView(.everyMinute) { context in
                            let now = context.date
                            Text(Verdict.from(model.history, scanWarning: model.scanWarning(now: now), now: now).sentence)
                                .font(.system(.title3, design: .serif))
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    } else {
                        Text("Driftwatch").font(.headline)
                    }
                    status.font(.subheadline).foregroundStyle(.secondary)
                }
                Spacer()
                if model.isScanning {
                    ProgressView().controlSize(.small)
                }
            }

            TimelineView(.everyMinute) { context in
                if let warning = model.scanWarning(now: context.date) {
                    Label(warning, systemImage: "exclamationmark.triangle")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
            }

            if let warning = model.history?.editWarning {
                Label(warning, systemImage: "exclamationmark.triangle")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }

            Divider()

            let recent = Array((model.history?.rows ?? []).prefix(5))
            if recent.isEmpty {
                Text("No changes since your baseline.")
                    .foregroundStyle(.secondary)
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Recent changes").font(.subheadline).foregroundStyle(.secondary)
                    ForEach(recent) { row in
                        Button {
                            model.selection = row.id
                            showMainWindow(openWindow)
                        } label: {
                            RecentRow(row: row)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            Divider()

            HStack {
                Button("Scan Now") { Task { await model.scan() } }
                    .disabled(model.isScanning)
                Spacer()
                Button("Open Driftwatch") { showMainWindow(openWindow) }
                    .keyboardShortcut(.defaultAction)
            }
            Button("Quit Driftwatch") { NSApp.terminate(nil) }
                .keyboardShortcut("q")
        }
        .padding()
        .frame(width: 320)
        .onAppear { model.markSeen() }
    }

    @ViewBuilder private var status: some View {
        if model.isScanning {
            Text("Scanning your Mac…")
        } else if let last = model.history?.latest?.takenAt {
            VStack(alignment: .leading, spacing: 2) {
                TimelineView(.everyMinute) { _ in
                    Text("Scanned \(last.formatted(.relative(presentation: .named)))")
                }
                Text(changeCount)
            }
        } else {
            Text("Not scanned yet")
        }
    }

    private var changeCount: String {
        switch model.latestScanChangeCount {
        case 0: "No changes in the last scan"
        case 1: "1 change in the last scan"
        case let n: "\(n) changes in the last scan"
        }
    }
}

private struct RecentRow: View {
    let row: Row

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            KindIcon(change: row.change)
            VStack(alignment: .leading, spacing: 1) {
                Text(row.change.title).lineLimit(1).truncationMode(.middle)
                Text(Category.named(row.change.category).name)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
    }
}
