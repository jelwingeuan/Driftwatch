import Charts
import SwiftUI

/// The first screen: the verdict in one sentence, a spirit level that drifts with
/// it, changes per day for the last 30 days, and each area Driftwatch watches.
struct OverviewView: View {
    let model: AppModel
    let history: History
    /// Shows All changes, only those from the given day.
    let showDay: (Date) -> Void

    var body: some View {
        TimelineView(.everyMinute) { context in
            let now = context.date
            let verdict = Verdict.from(history, scanWarning: model.scanWarning(now: now), now: now)
            ScrollView {
                VStack(alignment: .leading, spacing: 28) {
                    VStack(alignment: .leading, spacing: 14) {
                        LevelBubble(verdict: verdict)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(verdict.sentence)
                                .font(.system(size: 30, design: .serif))
                                .fixedSize(horizontal: false, vertical: true)
                                .accessibilityAddTraits(.isHeader)
                            Text("Watching ^[\(history.itemsTotal) item](inflect: true) on this Mac.")
                                .foregroundStyle(.secondary)
                        }
                    }
                    DayChart(history: history, now: now, showDay: showDay)
                    watched(now: now)
                }
                .frame(maxWidth: 720, alignment: .leading)
                .padding(32)
                .frame(maxWidth: .infinity)
            }
        }
    }

    private func watched(now: Date) -> some View {
        let recent = history.changes(since: now - Verdict.window)
        return VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                Text("What Driftwatch watches").font(.headline)
                Spacer()
                Text("Changes from the last 7 days").font(.callout).foregroundStyle(.secondary)
            }
            // Two columns when there is room, so the list stays short and each row narrow.
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 330), spacing: 24)], spacing: 0) {
                ForEach(Category.all.filter { $0.id != "all" }) { category in
                    VStack(spacing: 8) {
                        Divider()
                        HStack {
                            // A fixed-width icon, so the names line up.
                            Label { Text(category.name).lineLimit(1) } icon: {
                                Image(systemName: category.symbol).frame(width: 20)
                            }
                            Spacer(minLength: 8)
                            if let items = history.items(in: category.id) {
                                Text("^[\(items) item](inflect: true)").foregroundStyle(.secondary).monospacedDigit()
                            }
                            status(category, changes: recent.count(where: category.contains))
                                .frame(width: 112, alignment: .leading)
                        }
                        .padding(.bottom, 8)
                        .accessibilityElement(children: .combine)
                    }
                }
            }
        }
    }

    /// Words stay primary; only the symbol carries a tint.
    private func status(_ category: Category, changes: Int) -> some View {
        let (text, symbol, tint): (LocalizedStringKey, String, Color) =
            if history.errors.contains(where: { $0.category == category.id }) {
                ("Can’t read all", "exclamationmark.triangle", .yellow)
            } else if changes == 0 {
                ("Nothing new", "checkmark.circle", .secondary)
            } else {
                ("^[\(changes) change](inflect: true)", "plusminus.circle", .accentColor)
            }
        return Label { Text(text) } icon: { Image(systemName: symbol).foregroundStyle(tint) }
    }
}

/// A spirit level: the bubble sits between the marks when nothing risky changed,
/// drifts right with each risky change (up to 4), and rests at the end when
/// Driftwatch can't see everything. Decoration only; the sentence says it in words.
private struct LevelBubble: View {
    let verdict: Verdict
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var drift: CGFloat {
        switch verdict {
        case .quiet: 0
        case .worthALook(let n): CGFloat(min(n, 4)) / 5
        case .cantSee: 1
        }
    }

    var body: some View {
        let amber = Color(red: 0xF2 / 255, green: 0xA3 / 255, blue: 0x3A / 255)
        ZStack {
            Capsule().fill(Color(red: 0x12 / 255, green: 0x34 / 255, blue: 0x3B / 255))
            Capsule().strokeBorder(.separator)
            HStack(spacing: 56) {
                ForEach(0..<2) { _ in Rectangle().fill(.white.opacity(0.35)).frame(width: 1, height: 16) }
            }
            Capsule()
                .fill(amber)
                .overlay(alignment: .top) {
                    Capsule().fill(.white.opacity(0.4)).frame(width: 24, height: 4).padding(.top, 3)
                }
                .frame(width: 44, height: 18)
                .offset(x: 92 * drift)
                // With Reduce Motion the bubble fades to its new place instead of travelling.
                .id(reduceMotion ? AnyHashable(verdict) : AnyHashable(0))
                .transition(.opacity)
        }
        .frame(width: 240, height: 30)
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : .spring(response: 0.4, dampingFraction: 0.8), value: verdict)
        .accessibilityHidden(true)
    }
}

/// Changes per day for the last 30 days, stacked by risk. Click a day (or use the
/// arrow keys, or VoiceOver's adjust) to see its count and open those changes.
private struct DayChart: View {
    let history: History
    let now: Date
    let showDay: (Date) -> Void
    @State private var selected: Date?
    @FocusState private var focused: Bool
    @Environment(\.colorScheme) private var colorScheme

    private struct Bar: Hashable {
        let day: Date
        /// An index into `risks`.
        let risk: Int
    }

    private static let risks = ["Low risk", "Worth a look", "High risk"]

    var body: some View {
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: now)
        let start = calendar.date(byAdding: .day, value: -29, to: today)!
        let end = calendar.date(byAdding: .day, value: 1, to: today)!
        let counts = Dictionary(
            history.rows.filter { $0.takenAt >= start }.map { row in
                (Bar(day: calendar.startOfDay(for: row.takenAt), risk: Self.risk(row.change)), 1)
            },
            uniquingKeysWith: +
        )
        // Day, then low to high risk, so low risk stacks at the bottom.
        let bars = counts.sorted { ($0.key.day, $0.key.risk) < ($1.key.day, $1.key.risk) }
        let days = Set(counts.keys.map(\.day)).sorted()
        let detail = detail(counts, days: days)

        VStack(alignment: .leading, spacing: 8) {
            Text("Last 30 days").font(.headline)
            if days.isEmpty {
                Text("No changes in the last 30 days.").foregroundStyle(.secondary)
            } else {
                Chart {
                    if let selected {
                        // The whole day, so it shows on both sides of a bar that fills the height.
                        RectangleMark(x: .value("Day", selected, unit: .day), width: .ratio(1))
                            .foregroundStyle(Color.primary.opacity(0.1))
                    }
                    ForEach(bars, id: \.key) { bar in
                        BarMark(x: .value("Day", bar.key.day, unit: .day), y: .value("Changes", bar.value))
                            .foregroundStyle(by: .value("Risk", Self.risks[bar.key.risk]))
                            .opacity(selected == nil || selected == bar.key.day ? 1 : 0.4)
                    }
                }
                .chartForegroundStyleScale(domain: Self.risks, range: [Color.secondary, medium, .red])
                .chartXScale(domain: start...end)
                .chartOverlay { proxy in
                    GeometryReader { geometry in
                        Rectangle().fill(.clear).contentShape(Rectangle())
                            .onTapGesture { location in
                                guard let plot = proxy.plotFrame,
                                      let date: Date = proxy.value(atX: location.x - geometry[plot].origin.x),
                                      (start..<end).contains(date) else { return }
                                selected = calendar.startOfDay(for: date)
                                focused = true
                            }
                    }
                }
                .frame(height: 120)
                .focusable()
                .focused($focused)
                .onKeyPress(.leftArrow) { step(forward: false, in: days); return .handled }
                .onKeyPress(.rightArrow) { step(forward: true, in: days); return .handled }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("Changes per day, last 30 days")
                .accessibilityValue(detail)
                .accessibilityAdjustableAction { step(forward: $0 == .increment, in: days) }

                HStack {
                    Text(selected == nil ? "Select a day to see what changed." : detail)
                        .foregroundStyle(.secondary)
                    Spacer()
                    if let selected, days.contains(selected) {
                        Button("Show These Changes") { showDay(selected) }
                    }
                }
            }
        }
    }

    /// Readable on both backgrounds: darker in light mode, the level's amber in dark.
    private var medium: Color {
        colorScheme == .dark
            ? Color(red: 0xF2 / 255, green: 0xA3 / 255, blue: 0x3A / 255)
            : Color(red: 0xB8 / 255, green: 0x6E / 255, blue: 0x00 / 255)
    }

    private static func risk(_ change: Change) -> Int {
        switch change.risk {
        case "high": 2
        case "medium": 1
        default: 0
        }
    }

    /// The selected day in words, or a summary of the 30 days.
    private func detail(_ counts: [Bar: Int], days: [Date]) -> String {
        guard let selected else {
            let total = counts.values.reduce(0, +)
            return "\(total) \(total == 1 ? "change" : "changes") on \(days.count) \(days.count == 1 ? "day" : "days")."
        }
        let count = { (risk: Int) in counts[Bar(day: selected, risk: risk)] ?? 0 }
        let total = count(0) + count(1) + count(2)
        var parts = [total == 0 ? "no changes" : "\(total) \(total == 1 ? "change" : "changes")"]
        if count(1) > 0 { parts.append("\(count(1)) worth a look") }
        if count(2) > 0 { parts.append("\(count(2)) high risk") }
        return "\(selected.formatted(.dateTime.weekday(.wide).day().month(.wide))): \(parts.joined(separator: ", "))."
    }

    /// Selects the next (or previous) day that has changes.
    private func step(forward: Bool, in days: [Date]) {
        let next = forward
            ? days.first { $0 > (selected ?? .distantPast) }
            : days.last { $0 < (selected ?? .distantFuture) }
        if let next { selected = next }
    }
}
