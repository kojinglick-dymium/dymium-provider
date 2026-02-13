// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "DymiumProvider",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(
            name: "DymiumProvider",
            targets: ["DymiumProvider"]
        )
    ],
    targets: [
        .executableTarget(
            name: "DymiumProvider",
            path: "DymiumProvider",
            exclude: ["Info.plist", "DymiumProvider.entitlements"],
            linkerSettings: [
                .linkedFramework("Security"),
                .linkedFramework("AppKit")
            ]
        )
    ]
)
