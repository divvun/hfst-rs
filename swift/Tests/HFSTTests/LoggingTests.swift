import Foundation
import HFST
import Testing

/// The events a test's calls log, kept while the handler is set.
private final class Events: @unchecked Sendable {
    private let lock = NSLock()
    private var kept: [HFSTLogEvent] = []

    func add(_ event: HFSTLogEvent) {
        lock.lock()
        kept.append(event)
        lock.unlock()
    }

    /// The events kept whose target is under `root`, which leaves out what
    /// other suites' calls, running meanwhile, log.
    func under(_ root: String) -> [HFSTLogEvent] {
        lock.lock()
        defer { lock.unlock() }
        return kept.filter { $0.target.hasPrefix(root) }
    }
}

/// The events `body` logs at `level`, kept by a handler; the handler and the
/// level go back to their defaults afterwards.
private func logged(at level: HFSTLogLevel?, _ body: () throws -> Void) rethrows -> Events {
    let events = Events()
    HFSTLogging.setHandler { events.add($0) }
    HFSTLogging.level = level
    defer {
        HFSTLogging.setHandler(nil)
        HFSTLogging.level = .warning
    }
    try body()
    return events
}

/// A transducer whose alphabet holds `@odd1@`, which divvun-fst warns it
/// does not know. `+Event` has as many bytes, so the tables still read.
private func unhandledSymbol() throws -> Data {
    let data = try Fixtures.data("identity.optimized-lookup-weighted.hfst")
    return data.replacing(Data("\0+Event\0".utf8), with: Data("\0@odd1@\0".utf8))
}

/// The tokenise fixture with its tag `+CLB` spelt `ŋʷ`, a letter and a
/// modifier letter, which the hfst library warns about when it prints the
/// tag. Both spellings are four bytes, so the tables still read.
private func modifierTag() throws -> Tokeniser {
    let data = try Fixtures.data("tokenise.pmhfst")
    return try Tokeniser(data: data.replacing(Data("\0+CLB\0".utf8), with: Data("\0ŋʷ\0".utf8)))
}

extension Data {
    func replacing(_ old: Data, with new: Data) -> Data {
        guard let found = range(of: old) else { return self }
        var changed = self
        changed.replaceSubrange(found, with: new)
        return changed
    }
}

// The handler and the level are the process's, so these run one at a time.
@Suite(.serialized) struct LoggingTests {
    // [spec:hfst:req:swift.logging/test]
    @Test func hfstEventsReachTheHandler() throws {
        let tokeniser = try modifierTag()
        let events = try logged(at: .debug) { _ = try tokeniser.format("a cat.", as: .giellaCG) }
        let event = try #require(events.under("hfst").first)
        #expect(events.under("hfst").count == 1)
        #expect(event.target == "hfst::pmatch_tokenize")
        #expect(event.level == .warning)
        #expect(event.message.contains("ŋʷ"))
    }

    // [spec:hfst:req:swift.logging/test]
    @Test func divvunFstEventsReachTheHandler() throws {
        let data = try unhandledSymbol()
        let events = try logged(at: .warning) { _ = try OptimisedTransducer(data: data) }
        let event = try #require(events.under("divvun_fst").first)
        #expect(event.target == "divvun_fst::transducer::hfst::alphabet")
        #expect(event.level == .warning)
        #expect(event.message.contains("@odd1@"))
    }

    // [spec:hfst:req:swift.logging/test]
    @Test func theLevelKeepsOutLesserEvents() throws {
        let data = try unhandledSymbol()
        let tokeniser = try modifierTag()
        for level in [HFSTLogLevel.error, nil] {
            let events = try logged(at: level) {
                _ = try OptimisedTransducer(data: data)
                _ = try tokeniser.format("a cat.", as: .giellaCG)
            }
            #expect(events.under("hfst").isEmpty && events.under("divvun_fst").isEmpty)
        }
        #expect(HFSTLogging.level == .warning)
    }
}
