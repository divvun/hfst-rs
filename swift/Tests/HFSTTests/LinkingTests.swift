import Foundation
import HFSTCompile
import LinkProbe
import Testing

/// HFST and HFSTCompile in one binary with a second Rust library, built on
/// its own by the same toolchain with the same uniffi and tracing, with LTO
/// and without the prelink, so std's symbols stay global in it. On the iOS
/// simulator this is the link of an app that uses both packages.
struct LinkingTests {
    // [spec:hfst:req:swift.linking+1/test]
    @Test func hfstLinksBesideAnotherRustLibraryAndEachKeepsItsOwnTracing() throws {
        #expect(LinkProbe.wordCounts(text: "a b a") == ["a=2", "b=1"])
        let tally = Tally(words: ["x"])
        #expect(tally.add(word: "y") == 2)

        let compiled = try regex("{cat}:{dog}").value
        #expect(try compiled.lookup("cat").map(\.output) == ["dog"])
        try compiled.convert(to: .optimizedLookupWeighted)
        let fast = try OptimisedTransducer(data: compiled.data())
        #expect(try fast.lookup("cat").map(\.output) == ["dog"])

        // Each library dispatches to its own tracing: a warning hfst logs
        // reaches nothing of the probe's, whose subscriber sees its own
        // events alone. Had the two shared one tracing, whichever set the
        // global subscriber first would see the other's events.
        _ = try OptimisedTransducer(data: unhandledSymbol())
        #expect(LinkProbe.targetsSeen() == ["hfst_link_probe"])
    }

    // [spec:hfst:req:swift.linking+1/test]
    @Test func aPanicUnwindsInTheLibraryItHappensIn() throws {
        #expect(throws: (any Error).self) { try LinkProbe.panics(message: "unwound") }
        #expect(LinkProbe.wordCounts(text: "after") == ["after=1"])
        let error = try #require(thrown { _ = try Transducer(data: Data("not a transducer stream".utf8)) })
        #expect(error.code == .invalidFormat)
    }
}
