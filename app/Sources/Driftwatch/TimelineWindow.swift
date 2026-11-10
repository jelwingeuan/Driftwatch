import AppKit
import SwiftUI

/// The main window: categories on the left, the change timeline, and an inspector.
struct TimelineWindow: View {
    @Bindable var model: AppModel
    @State private var categoryID: String? = "all"
    @State private var search = ""
    @State private var showInspector = true

    private var category: Category { Category.named(categoryID ?? "all") }

    var body: some View {
        NavigationSplitView {
            List(Category.all, selection: $categoryID) { category in
                Label(category.name, systemImage: category.symbol)
                    .badge(count(in: category))
            }
            .navigationSplitViewColumnWidth(min: 190, ideal: 220)
        } detail: {
            timeline
                .frame(minWidth: 360)
                .safeAreaInset(edge: .top, spacing: 0) { banners }
                .navigationTitle(category.name)
                .navigationSubtitle(subtitle)
                .searchable(text: $search, prompt: "Search changes")
                .inspector(isPresented: $showInspector) {
                    InspectorView(row: model.row(id: model.selection))
                        .inspectorColumnWidth(min: 260, ideal: 320, max: 480)
                }
                .toolbar {
                    ToolbarItemGroup {
                        if model.isScanning {
                            ProgressView().controlSize(.small)
                        }
                        Button { Task { await model.scan() } } label: {
                            Label("Scan Now", systemImage: "arrow.clockwise")
                        }
                        .help("Scan Now (⌘R)")
                        .disabled(model.isScanning)
                        Button { showInspector.toggle() } label: {
                            Label("Inspector", systemImage: "sidebar.trailing")
                        }
                        .help(showInspector ? "Hide Inspector" : "Show Inspector")
                    }
                }
        }
        .onChange(of: model.selection) { _, selection in
            if selection != nil { showInspector = true }
        }
        // Changes that arrive while the window is open count as seen.
        .onChange(of: model.history?.entries.first?.snapshotID) { model.markSeen() }
        .onAppear {
            // Show the Dock icon and menu bar (with ⌘R) while the window is open.
            NSApp.setActivationPolicy(.regular)
            NSApp.activate()
            model.markSeen()
        }
        .onDisappear { NSApp.setActivationPolicy(.accessory) }
    }

    // MARK: Timeline

    private var sections: [Entry] {
        (model.history?.entries ?? []).compactMap { entry in
            let changes = entry.distinctChanges.filter { category.contains($0) && matchesSearch($0) }
            return changes.isEmpty
                ? nil : Entry(snapshotID: entry.snapshotID, takenAt: entry.takenAt, changes: changes)
        }
    }

    @ViewBuilder private var timeline: some View {
        let sections = sections
        if !sections.isEmpty {
            List(selection: $model.selection) {
                ForEach(sections) { entry in
                    Section {
                        ForEach(entry.rows) { row in
                            ChangeRow(row: row).tag(row.id)
                        }
                    } header: {
                        HStack {
                            Text(entry.takenAt, format: .dateTime.weekday(.wide).month().day().hour().minute())
                            Spacer()
                            Text("^[\(entry.changes.count) change](inflect: true)")
                        }
                    }
                }
            }
        } else if !search.isEmpty {
            ContentUnavailableView.search(text: search)
        } else if model.history == nil, model.isScanning {
            ProgressView("Loading your timeline…")
        } else if model.history == nil {
            ContentUnavailableView {
                Label("Driftwatch couldn’t read its timeline", systemImage: "exclamationmark.triangle")
            } description: {
                Text(model.lastError ?? "")
            } actions: {
                Button("Try Again") { Task { await model.reload() } }
            }
        } else if model.history?.latest == nil {
            ContentUnavailableView {
                Label("No scans yet", systemImage: "eye")
            } description: {
                Text("Driftwatch takes a first look at your Mac, then shows you anything that changes after that.")
            } actions: {
                scanNowButton
            }
        } else {
            ContentUnavailableView {
                Label("No changes since your baseline", systemImage: "checkmark.shield")
            } description: {
                Text(emptyDescription)
            } actions: {
                scanNowButton
            }
        }
    }

    private var scanNowButton: some View {
        Button("Scan Now") { Task { await model.scan() } }
            .disabled(model.isScanning)
    }

    private var emptyDescription: String {
        let total = model.history?.itemsTotal ?? 0
        return category.id == "all"
            ? "Driftwatch is watching \(total) items on your Mac. Anything that changes will show up here."
            : "Nothing in \(category.name.lowercased()) has changed since your first scan."
    }

    @ViewBuilder private var banners: some View {
        VStack(spacing: 0) {
            if model.history?.needsFullDiskAccess == true {
                Banner(
                    symbol: "lock.shield",
                    text: "Allow Full Disk Access so Driftwatch can see which apps have privacy permissions.",
                    buttonTitle: "Open System Settings"
                ) {
                    let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")!
                    NSWorkspace.shared.open(url)
                }
            }
            if model.history != nil {
                TimelineView(.everyMinute) { context in
                    if let warning = model.scanWarning(now: context.date) {
                        Banner(symbol: "exclamationmark.triangle", text: warning)
                    }
                }
            }
        }
    }

    private var subtitle: String {
        guard let last = model.history?.latest?.takenAt else { return "" }
        return "Last scan \(last.formatted(date: .abbreviated, time: .shortened))"
    }

    private func matchesSearch(_ change: Change) -> Bool {
        search.isEmpty
            || change.title.localizedCaseInsensitiveContains(search)
            || change.key.localizedCaseInsensitiveContains(search)
            || (change.summary ?? "").localizedCaseInsensitiveContains(search)
    }

    private func count(in category: Category) -> Int {
        (model.history?.entries ?? []).reduce(0) { total, entry in
            total + entry.distinctChanges.count(where: category.contains)
        }
    }
}

/// The kind of change as a symbol whose shape (not only its color) tells it apart.
/// Items that could not be read get a warning symbol instead (see `Change.status`).
struct KindIcon: View {
    let change: Change

    var body: some View {
        let status = change.status
        Image(systemName: status.symbol)
            .foregroundStyle(status.tint)
            .accessibilityLabel(status.label)
    }
}

private struct ChangeRow: View {
    let row: Row

    var body: some View {
        let change = row.change
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            KindIcon(change: change).imageScale(.large)
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline) {
                    Text(change.title).lineLimit(1).truncationMode(.middle)
                    Spacer()
                    Text(Category.named(change.category).name)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                if let line = change.subtitle {
                    Text(line)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                        .truncationMode(.middle)
                }
                if change.risk == "medium" || change.risk == "high" || change.addedBy != nil {
                    HStack(spacing: 8) {
                        RiskBadge(change: change)
                        if let addedBy = change.addedByText {
                            Text(addedBy).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }
}

/// A risk hint for medium and high risk only: symbol and words, never colour alone.
struct RiskBadge: View {
    let change: Change

    var body: some View {
        if let style {
            Label(style.text, systemImage: style.symbol)
                .font(.caption.weight(.medium))
                .foregroundStyle(style.tint)
                .help(change.riskReason ?? "")
                .accessibilityLabel([style.text, change.riskReason].compactMap { $0 }.joined(separator: ": "))
        }
    }

    private var style: (text: String, symbol: String, tint: Color)? {
        switch change.risk {
        case "high": ("High risk", "exclamationmark.octagon.fill", .red)
        case "medium": ("Worth a look", "exclamationmark.triangle", .orange)
        default: nil
        }
    }
}

private struct Banner: View {
    let symbol: String
    let text: String
    var buttonTitle: String?
    var action: () -> Void = {}

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: symbol)
                    .imageScale(.large)
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                Text(text)
                    .font(.callout)
                Spacer(minLength: 0)
                if let buttonTitle {
                    Button(buttonTitle, action: action)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            Divider()
        }
        .background(.bar)
    }
}
