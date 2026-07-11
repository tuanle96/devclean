// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "DevcleanMenuBar",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "DevcleanMenuBarKit", targets: ["DevcleanMenuBarKit"]),
        .executable(name: "DevcleanMenuBar", targets: ["DevcleanMenuBar"]),
    ],
    dependencies: [
        .package(url: "https://github.com/getsentry/sentry-cocoa", from: "9.13.0"),
    ],
    targets: [
        .target(
            name: "DevcleanMenuBarKit",
            dependencies: [
                .product(name: "Sentry", package: "sentry-cocoa"),
            ]
        ),
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
