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

    static let scanInterval: TimeInterval = 30 * 60

    init() {
        timer = Timer.scheduledTimer(withTimeInterval: Self.scanInterval, repeats: true) { [weak self] _ in
            Task { await self?.scan() }
        }
        Task { await start() }
    }

    /// Load the timeline, then scan if the last scan is missing or older than the interval.
    private func start() async {
        await reload()
        let last = history?.latest?.takenAt ?? .distantPast
        if history != nil, Date.now.timeIntervalSince(last) > Self.scanInterval {
            await scan()
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

    func scan() async {
        guard !isScanning else { return }
        isScanning = true
        defer { isScanning = false }
        do {
            _ = try await CLI.run(["scan", "--json"], as: ScanResult.self)
            history = try await CLI.run(["history", "--json"], as: History.self)
            lastError = nil
        } catch {
            fail(error)
        }
    }

    func reload() async {
        do {
            history = try await CLI.run(["history", "--json"], as: History.self)
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
        case failed(String)

        var errorDescription: String? {
            switch self {
            case .notFound: "Driftwatch’s scanner is missing. Reinstall Driftwatch to fix this."
            case .failed(let message): message
            }
        }
    }

    /// Next to the app's own executable (named `driftwatch-cli`, since the bundle's
    /// file system may not tell `driftwatch` from `Driftwatch`), or `$DRIFTWATCH_CLI`
    /// when run with `swift run`.
    static var url: URL? {
        let bundled = Bundle.main.executableURL?.deletingLastPathComponent()
            .appendingPathComponent("driftwatch-cli")
        if let bundled, FileManager.default.isExecutableFile(atPath: bundled.path) {
            return bundled
        }
        return ProcessInfo.processInfo.environment["DRIFTWATCH_CLI"].map(URL.init(fileURLWithPath:))
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
        // The CLI writes at most one short line to stderr, so reading stdout first cannot stall.
        let output = stdout.fileHandleForReading.readDataToEndOfFile()
        let errors = stderr.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            let message = String(decoding: errors, as: UTF8.self)
                .trimmingCharacters(in: .whitespacesAndNewlines)
            throw Failure.failed(message.isEmpty ? "The scan stopped unexpectedly." : message)
        }
        return output
    }
}
