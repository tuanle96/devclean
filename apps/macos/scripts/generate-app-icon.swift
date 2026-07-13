#!/usr/bin/env swift
import AppKit
import CoreGraphics
import Foundation

struct IconSpec {
    let filename: String
    let pixels: Int
}

let scriptURL = URL(fileURLWithPath: #filePath)
let macOSDirectory =
    scriptURL
    .deletingLastPathComponent()
    .deletingLastPathComponent()
let resourcesDirectory = macOSDirectory.appendingPathComponent("Resources", isDirectory: true)
let masterURL = resourcesDirectory.appendingPathComponent("AppIcon-master.png")
let iconsetURL = resourcesDirectory.appendingPathComponent("AppIcon.iconset", isDirectory: true)
let icnsURL = resourcesDirectory.appendingPathComponent("AppIcon.icns")

guard let master = NSImage(contentsOf: masterURL) else {
    fatalError("Missing icon master at \(masterURL.path)")
}

guard
    let representation = master.representations.first,
    representation.pixelsWide == 1024,
    representation.pixelsHigh == 1024
else {
    fatalError("AppIcon-master.png must be exactly 1024x1024 pixels")
}

let specs = [
    IconSpec(filename: "icon_16x16.png", pixels: 16),
    IconSpec(filename: "icon_16x16@2x.png", pixels: 32),
    IconSpec(filename: "icon_32x32.png", pixels: 32),
    IconSpec(filename: "icon_32x32@2x.png", pixels: 64),
    IconSpec(filename: "icon_128x128.png", pixels: 128),
    IconSpec(filename: "icon_128x128@2x.png", pixels: 256),
    IconSpec(filename: "icon_256x256.png", pixels: 256),
    IconSpec(filename: "icon_256x256@2x.png", pixels: 512),
    IconSpec(filename: "icon_512x512.png", pixels: 512),
    IconSpec(filename: "icon_512x512@2x.png", pixels: 1024),
]

let fileManager = FileManager.default
try? fileManager.removeItem(at: iconsetURL)
try fileManager.createDirectory(at: iconsetURL, withIntermediateDirectories: true)

for spec in specs {
    guard
        let bitmap = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: spec.pixels,
            pixelsHigh: spec.pixels,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        )
    else {
        fatalError("Could not allocate \(spec.pixels)x\(spec.pixels) icon bitmap")
    }

    bitmap.size = NSSize(width: spec.pixels, height: spec.pixels)
    guard let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
        fatalError("Could not create icon graphics context")
    }

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.imageInterpolation = .high
    context.cgContext.interpolationQuality = .high
    master.draw(
        in: NSRect(x: 0, y: 0, width: spec.pixels, height: spec.pixels),
        from: .zero,
        operation: .copy,
        fraction: 1
    )
    context.flushGraphics()
    NSGraphicsContext.restoreGraphicsState()

    guard let png = bitmap.representation(using: .png, properties: [:]) else {
        fatalError("Could not encode \(spec.filename)")
    }
    try png.write(to: iconsetURL.appendingPathComponent(spec.filename), options: .atomic)
}

let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconsetURL.path, "-o", icnsURL.path]
try iconutil.run()
iconutil.waitUntilExit()

guard iconutil.terminationStatus == 0 else {
    fatalError("iconutil failed with status \(iconutil.terminationStatus)")
}

print(icnsURL.path)
