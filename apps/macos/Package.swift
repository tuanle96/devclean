// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "DevcleanMenuBar",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "DevcleanMenuBarKit", targets: ["DevcleanMenuBarKit"]),
        .executable(name: "DevcleanMenuBar", targets: ["DevcleanMenuBar"]),
    ],
    targets: [
        .target(name: "DevcleanMenuBarKit"),
        .executableTarget(
            name: "DevcleanMenuBar",
            dependencies: ["DevcleanMenuBarKit"]
        ),
        .testTarget(
            name: "DevcleanMenuBarKitTests",
            dependencies: ["DevcleanMenuBarKit"]
        ),
    ]
)
