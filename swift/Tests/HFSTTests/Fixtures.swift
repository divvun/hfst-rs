import Foundation
import HFST
import Testing

/// The transducers, tokenisers and DHFST files the npm package's tests read,
/// and the native library's results for them, from the directory
/// `DIVVUN_HFST_FIXTURES` names. On macOS, `node js/test/fixtures.mjs` builds
/// them into a directory of the run's own when it names none; the iOS
/// simulator has no `node`, so there `xcodebuild` passes the directory as
/// `TEST_RUNNER_DIVVUN_HFST_FIXTURES`.
enum Fixtures {
    static let directory: URL = {
        if let given = ProcessInfo.processInfo.environment["DIVVUN_HFST_FIXTURES"], !given.isEmpty {
            return URL(fileURLWithPath: given, isDirectory: true)
        }
        #if os(macOS)
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let built = FileManager.default.temporaryDirectory
            .appendingPathComponent("divvun-hfst-fixtures-\(ProcessInfo.processInfo.processIdentifier)")
        let node = Process()
        node.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        node.arguments = ["node", "js/test/fixtures.mjs", built.path]
        node.currentDirectoryURL = root
        node.standardOutput = FileHandle.nullDevice
        try? node.run()
        node.waitUntilExit()
        return built
        #else
        fatalError("set DIVVUN_HFST_FIXTURES, as TEST_RUNNER_DIVVUN_HFST_FIXTURES for xcodebuild")
        #endif
    }()

    static func url(_ name: String) -> URL {
        directory.appendingPathComponent(name)
    }

    static func data(_ name: String) throws -> Data {
        try Data(contentsOf: url(name))
    }

    static func text(_ name: String) throws -> String {
        try String(contentsOf: url(name), encoding: .utf8)
    }

    static func json(_ name: String) throws -> Any {
        try JSONSerialization.jsonObject(with: data(name), options: [.fragmentsAllowed])
    }
}

/// The transducer types `HFST` reads, as the fixtures name them.
let read = ["optimized-lookup-weighted", "optimized-lookup-unweighted"]

/// The `HFSTError` `body` throws, or `nil` when it throws nothing; any other
/// error is an issue.
func thrown(_ body: () throws -> Void) -> HFSTError? {
    do {
        try body()
        return nil
    } catch let error as HFSTError {
        return error
    } catch {
        Issue.record("threw \(error), not an HFSTError")
        return nil
    }
}
