import Foundation
import Observation
import os

private let log = Logger(subsystem: "com.driftwatch.app", category: "cli")

@MainActor @Observable
final class AppModel {
    private(set) var history: History?
    private(set) var isScanning = false
    private(set) var lastError: String?
    /// `Row.id` of the change shown in the window's inspector.
    var selection: String?
    private var seenSnapshotID = Int64(UserDefaults.standard.integer(forKey: "seenSnapshotID"))
    @ObservationIgnored private var timer: Timer?

    nonisolated static let scanInterval: TimeInterval = 30 * 60
    /// How long scheduled scans may leave out login & background items.
    nonisolated static let loginItemsInterval: TimeInterval = 24 * 60 * 60
    /// The timeline, with every change from the last 31 days even past the CLI's
    /// 100-scan limit: the overview's 30-day chart and 7-day verdict need them all.
    nonisolated static let historyArguments = ["history", "--json", "--days", "31"]
    private static let lastFullScanKey = "lastFullScan"

    init() {
        timer = Timer.scheduledTimer(withTimeInterval: Self.scanInterval, repeats: true) { [weak self] _ in
            Task { await self?.scan(scheduled: true) }
        }
        Task { await start() }
    }

    /// Whether a scan leaves out login & background items, whose reader asks for an
    /// admin password: scheduled scans do, unless no scan has read them in the last day.
    nonisolated static func skipsLoginItems(scheduled: Bool, lastFullScan: Date?, now: Date = .now) -> Bool {
        guard scheduled, let lastFullScan else { return false }
        return now.timeIntervalSince(lastFullScan) <= loginItemsInterval
    }

    /// A warning in plain words, or nil: the last scan (or timeline read) failed, or
    /// no scan has finished for over two intervals and none is running to catch up.
    nonisolated static func scanWarning(error: String?, lastScan: Date?, isScanning: Bool, now: Date = .now) -> String? {
        if let error { return "Scan failed. \(error)" }
        guard !isScanning, let lastScan, now.timeIntervalSince(lastScan) > 2 * scanInterval else { return nil }
        let when = lastScan.formatted(date: .abbreviated, time: .shortened)
        return "Scan is out of date. Driftwatch hasn’t finished a scan since \(when)."
    }

    func scanWarning(now: Date) -> String? {
        Self.scanWarning(error: lastError, lastScan: history?.latest?.takenAt, isScanning: isScanning, now: now)
    }

    /// Load the timeline, then scan if the last scan is missing or older than the interval.
    private func start() async {
        await reload()
        let last = history?.latest?.takenAt ?? .distantPast
        if history != nil, Date.now.timeIntervalSince(last) > Self.scanInterval {
            await scan(scheduled: true)
        }
    }

    var hasUnseenChanges: Bool {
        guard let newest = history?.entries.first?.snapshotID else { return false }
        return newest > seenSnapshotID
    }

    func markSeen() {
        guard let newest = history?.entries.first?.snapshotID, newest > seenSnapshotID else { return }
        seenSnapshotID = newest
        UserDefaults.standard.set(Int(newest), forKey: "seenSnapshotID")
    }

    /// Changes found by the latest scan (zero when it found none).
    var latestScanChangeCount: Int {
        guard let history, let entry = history.entries.first,
              entry.snapshotID == history.latest?.snapshotID else { return 0 }
        return entry.distinctChanges.count
    }

    func row(id: String?) -> Row? {
        guard let id else { return nil }
        return history?.rows.first { $0.id == id }
    }

    /// Scan Now reads everything; `scheduled` scans (on launch and on the timer) may
    /// leave out login & background items, so they don't ask for a password each time.
    func scan(scheduled: Bool = false) async {
        guard !isScanning else { return }
        isScanning = true
        defer { isScanning = false }
        let defaults = UserDefaults.standard
        let lastFullScan = defaults.object(forKey: Self.lastFullScanKey) as? Date
        let skip = Self.skipsLoginItems(scheduled: scheduled, lastFullScan: lastFullScan)
        do {
            _ = try await CLI.run(["scan", "--json"] + (skip ? ["--skip", "btm"] : []), as: ScanResult.self)
            if !skip { defaults.set(Date.now, forKey: Self.lastFullScanKey) }
            history = try await CLI.run(Self.historyArguments, as: History.self)
            lastError = nil
        } catch {
            fail(error)
        }
    }

    func reload() async {
        do {
            history = try await CLI.run(Self.historyArguments, as: History.self)
            lastError = nil
        } catch {
            fail(error)
        }
    }

    private func fail(_ error: Error) {
        log.error("\(error.localizedDescription, privacy: .public)")
        lastError = error.localizedDescription
    }
}

/// Runs the bundled `driftwatch` command-line tool and decodes its JSON.
enum CLI {
    enum Failure: LocalizedError {
        case notFound
        case timedOut
        case failed(String)

        var errorDescription: String? {
            switch self {
            case .notFound: "Driftwatch’s scanner is missing. Reinstall Driftwatch to fix this."
            case .timedOut: "It took longer than \(Int(CLI.timeout / 60)) minutes and was stopped."
            case .failed(let message): message
            }
        }
    }

    /// How long the scanner may run before it is stopped, so a stuck scan can't
    /// leave Driftwatch waiting forever.
    static let timeout: TimeInterval = 10 * 60

    /// Next to the app's own executable (named `driftwatch-cli`, since the bundle's
    /// file system may not tell `driftwatch` from `Driftwatch`), or, in a debug build
    /// only (`swift run`, `swift test`), `$DRIFTWATCH_CLI`. A release build never
    /// runs a scanner named by the environment.
    static var url: URL? {
        let bundled = Bundle.main.executableURL?.deletingLastPathComponent()
            .appendingPathComponent("driftwatch-cli")
        if let bundled, FileManager.default.isExecutableFile(atPath: bundled.path) {
            return bundled
        }
        #if DEBUG
        return ProcessInfo.processInfo.environment["DRIFTWATCH_CLI"].map(URL.init(fileURLWithPath:))
        #else
        return nil
        #endif
    }

    static func run<T: Decodable>(_ arguments: [String], as type: T.Type) async throws -> T {
        guard let url else { throw Failure.notFound }
        let data = try await withCheckedThrowingContinuation { continuation in
            // Blocking reads and waits belong on a thread outside the concurrency pool.
            DispatchQueue.global(qos: .userInitiated).async {
                continuation.resume(with: Result { try runBlocking(url, arguments) })
            }
        }
        return try decodeCLIOutput(type, from: data)
    }

    private static func runBlocking(_ url: URL, _ arguments: [String]) throws -> Data {
        let process = Process()
        process.executableURL = url
        process.arguments = arguments
        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr
        try process.run()
        // Stopping it closes its pipes, so the reads below return.
        let stop = DispatchWorkItem { process.terminate() }
        DispatchQueue.global().asyncAfter(deadline: .now() + timeout, execute: stop)
        // The CLI writes at most one short line to stderr, so reading stdout first cannot stall.
        let output = stdout.fileHandleForReading.readDataToEndOfFile()
        let errors = stderr.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        stop.cancel()
        if process.terminationReason == .uncaughtSignal, process.terminationStatus == SIGTERM {
            throw Failure.timedOut
        }
        guard process.terminationStatus == 0 else {
            let message = String(decoding: errors, as: UTF8.self)
                .trimmingCharacters(in: .whitespacesAndNewlines)
            throw Failure.failed(message.isEmpty ? "The scan stopped unexpectedly." : message)
        }
        return output
    }
}
