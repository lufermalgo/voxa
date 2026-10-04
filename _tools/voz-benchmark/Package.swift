// swift-tools-version: 6.2
import PackageDescription
let package = Package(
    name: "VozBench", platforms: [.macOS(.v15)],
    dependencies: [.package(url: "https://github.com/Desert-Ant-Labs/desert-ant-core.git", exact: "3.5.0")],
    targets: [.executableTarget(name: "VozBench", dependencies: [
        .product(name: "Voz", package: "desert-ant-core")
    ])]
)
