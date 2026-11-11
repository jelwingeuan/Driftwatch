import Foundation

/// The overview's answer in one sentence: can Driftwatch see everything, and did
/// anything risky (worth a look or high risk) change in the last 7 days.
enum Verdict: Hashable {
    case cantSee
    case worthALook(Int)
    case quiet

    /// A change counts while its scan is at most 7 days (168 hours) old, inclusive.
    static let window: TimeInterval = 7 * 24 * 60 * 60

    /// Never `.quiet` unless Driftwatch can vouch for it: no timeline, no scan yet,
    /// an empty scan, unreadable items, or a failed or old scan (even while a new
    /// one runs) all give `.cantSee`.
    static func from(_ history: History?, scanWarning: String?, now: Date) -> Verdict {
        guard let history, let latest = history.latest, history.itemsTotal > 0, history.errors.isEmpty,
              scanWarning == nil, now.timeIntervalSince(latest.takenAt) <= 2 * AppModel.scanInterval
        else { return .cantSee }
        let risky = history.changes(since: now - window).count { $0.risk == "medium" || $0.risk == "high" }
        return risky > 0 ? .worthALook(risky) : .quiet
    }

    var sentence: String {
        switch self {
        case .cantSee: "Driftwatch can’t see everything right now."
        case .worthALook(1): "1 risky change in the last 7 days."
        case .worthALook(let n): "\(n) risky changes in the last 7 days."
        case .quiet: "No risky changes in the last 7 days."
        }
    }
}
