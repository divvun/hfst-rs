import Foundation
import os
internal import HFSTGlue

// [spec:hfst:req:swift.logging+1]
/// A level of the library's log events, least severe first.
public enum HFSTLogLevel: Int, Sendable, Hashable, Comparable, CaseIterable {
    case trace, debug, info, warning, error

    public static func < (a: HFSTLogLevel, b: HFSTLogLevel) -> Bool {
        a.rawValue < b.rawValue
    }

    init(_ level: HFSTGlue.LogLevel) {
        self = switch level {
        case .trace: .trace
        case .debug: .debug
        case .info: .info
        case .warning: .warning
        case .error: .error
        }
    }

    var glue: HFSTGlue.LogLevel {
        switch self {
        case .trace: .trace
        case .debug: .debug
        case .info: .info
        case .warning: .warning
        case .error: .error
        }
    }

    /// The unified log's type for the level, which has none below debug.
    var type: OSLogType {
        switch self {
        case .trace, .debug: .debug
        case .info: .info
        case .warning: .default
        case .error: .error
        }
    }
}

// [spec:hfst:req:swift.logging+1]
/// One event the library logged.
public struct HFSTLogEvent: Sendable, Hashable {
    public let level: HFSTLogLevel
    /// Where in the library it comes from, such as `hfst::pmatch_core`, or
    /// `divvun_fst` for the engine behind the optimised lookup.
    public let target: String
    /// The event's message, followed by its other fields as `name=value`.
    public let message: String
}

// [spec:hfst:req:swift.logging+1]
/// Where the library's log events go. Each goes to the unified log, under
/// the subsystem `no.divvun.hfst` with its target as the category, and to
/// the handler, when one is set. Messages are logged as private data, which
/// the unified log shows only to a debugger, since they can quote a word.
public enum HFSTLogging {
    /// The least level that is logged, or `nil` for none: `.warning` unless
    /// set. An event below it is never formatted.
    public static var level: HFSTLogLevel? {
        get { logLevel().map(HFSTLogLevel.init) }
        set { setLogLevel(level: newValue?.glue) }
    }

    /// Sends each event to `handler` as well as to the unified log, or to
    /// the unified log alone for `nil`. The handler is called on the thread
    /// that logged the event, in the middle of the call that logged it.
    public static func setHandler(_ handler: (@Sendable (HFSTLogEvent) -> Void)?) {
        sink.handler = handler
    }

    /// Sets the library's sink, once; every call into the glue does this
    /// first.
    package static func install() {
        _ = installed
    }
}

private let sink = Sink()

private let installed: Void = setLogSink(sink: sink)

/// The sink the library's events reach.
private final class Sink: LogSink, @unchecked Sendable {
    private let lock = NSLock()
    private var logs: [String: OSLog] = [:]
    private var handlerStore: (@Sendable (HFSTLogEvent) -> Void)?

    var handler: (@Sendable (HFSTLogEvent) -> Void)? {
        get { locked { handlerStore } }
        set { locked { handlerStore = newValue } }
    }

    /// `body` with the lock held. `NSLock.withLock` needs iOS 16.
    private func locked<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }

    private func log(for target: String) -> OSLog {
        locked {
            if let log = logs[target] { return log }
            let log = OSLog(subsystem: "no.divvun.hfst", category: target)
            logs[target] = log
            return log
        }
    }

    func log(level: HFSTGlue.LogLevel, target: String, message: String) {
        let level = HFSTLogLevel(level)
        os_log("%@", log: log(for: target), type: level.type, message)
        handler?(HFSTLogEvent(level: level, target: target, message: message))
    }
}
