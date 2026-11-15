// swift-tools-version:6.0
import Foundation
import PackageDescription

// [spec:hfst:def:swift.package]
// The products sit over one binary target, the XCFramework swift/build.sh
// makes, which holds the static library of both products' Rust glue. The
// targets that hold the Swift UniFFI generates are imported internally by
// the products, so none of the generated API is public.
let package = Package(
    name: "HFST",
    platforms: [.iOS(.v13), .macOS(.v11)],
    products: [
        // [spec:hfst:req:swift.runtime-product]
        // HFST reaches HFSTGlue alone, so an app linking it reaches none of
        // the compile glue, and its linker drops the compilers.
        .library(name: "HFST", targets: ["HFST"]),
        .library(name: "HFSTCompile", targets: ["HFSTCompile"]),
    ],
    targets: [
        .binaryTarget(name: "HFSTFFI", path: "HFSTFFI.xcframework"),
        .target(name: "HFSTGlue", dependencies: ["HFSTFFI"]),
        .target(name: "HFST", dependencies: ["HFSTGlue"]),
        .target(name: "HFSTCompileGlue", dependencies: ["HFSTFFI", "HFSTGlue"]),
        .target(name: "HFSTCompile", dependencies: ["HFST", "HFSTGlue", "HFSTCompileGlue"]),
    ]
)

// The tests and what they link are in the repository and not in the
// archive a version is published as. LinkProbe is the linking test's second
// Rust library, which swift/build.sh builds from swift/link-probe on its own.
if FileManager.default.fileExists(atPath: Context.packageDirectory + "/Tests") {
    package.targets += [
        .binaryTarget(name: "LinkProbeFFI", path: "LinkProbeFFI.xcframework"),
        .target(name: "LinkProbe", dependencies: ["LinkProbeFFI"], path: "Tests/LinkProbe"),
        .testTarget(name: "HFSTTests", dependencies: ["HFST", "HFSTCompile", "LinkProbe"]),
    ]
}
