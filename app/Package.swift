// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "Driftwatch",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(name: "Driftwatch"),
        .testTarget(name: "DriftwatchTests", dependencies: ["Driftwatch"]),
    ]
)
