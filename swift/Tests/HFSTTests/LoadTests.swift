import Foundation
import HFST
import Testing

@Suite struct LoadTests {
    // [spec:hfst:req:swift.load/test]
    @Test func aStreamReadsInStreamOrder() throws {
        let transducers = try Transducer.all(in: Fixtures.data("ordered.hfst"))
        #expect(transducers.map(\.name) == ["first", "second"])
        #expect(transducers.allSatisfy { $0.type == .optimizedLookupWeighted })
        let bare = try Transducer.all(in: Fixtures.data("ordered.bare"))
        #expect(bare.map(\.name) == ["", ""])
        #expect(bare.map(\.properties) == [[:], [:]])
    }

    // [spec:hfst:req:swift.load/test]
    @Test(arguments: read)
    func bothTypesReadWithAndWithoutHeaders(kind: String) throws {
        for suffix in ["hfst", "bare"] {
            let data = try Fixtures.data("words.\(kind).\(suffix)")
            let transducer = try Transducer(data: data)
            #expect(transducer.type.rawValue == kind)
            #expect(try transducer.lookup("cat").map(\.output) == ["dog"])
            let optimised = try OptimisedTransducer(data: data)
            #expect(optimised.type.rawValue == kind)
            #expect(try optimised.lookup("cat").map(\.output) == ["dog"])
        }
    }

    // [spec:hfst:req:swift.load/test]
    @Test func propertiesLeaveOutVersionTypeAndName() throws {
        let transducer = try Transducer(data: Fixtures.data("first.hfst"))
        #expect(transducer.name == "first")
        #expect(Set(transducer.properties.keys).isDisjoint(with: ["version", "type", "name"]))
        let container = try Transducer.all(in: Fixtures.data("animals.pmhfst"))
        #expect(container[0].name == "TOP")
        #expect(Set(container.dropFirst().map(\.name)) == ["Animal"])
    }

    // [spec:hfst:req:swift.load/test]
    @Test func everyReaderTakesAFileURL() throws {
        let first = Fixtures.url("first.hfst")
        #expect(try Transducer(contentsOf: first).name == "first")
        #expect(try Transducer.all(contentsOf: first).map(\.name) == ["first"])
        #expect(try OptimisedTransducer(contentsOf: first).name == "first")
        let tokeniser = try Tokeniser(contentsOf: Fixtures.url("tokenise.pmhfst"))
        #expect(try tokeniser.tokenise("a cat").map(\.surface) == ["a", "cat"])
        guard case .acceptor = try Dhfst(contentsOf: Fixtures.url("speller.acceptor.dhfst")) else {
            Issue.record("the speller is an acceptor")
            return
        }
    }

    // [spec:hfst:req:swift.errors/test]
    @Test func aFileThatCannotBeReadThrowsWhatDataThrows() {
        let missing = Fixtures.url("no-such-file.hfst")
        #expect(throws: CocoaError.self) { try Transducer(contentsOf: missing) }
        #expect(throws: CocoaError.self) { try Tokeniser(contentsOf: missing) }
        #expect(throws: CocoaError.self) { try Dhfst(contentsOf: missing) }
    }

    // [spec:hfst:req:swift.load/test]
    @Test func oneTransducerReadersRefuseSeveral() throws {
        let several = thrown { _ = try Transducer(data: Fixtures.data("ordered.hfst")) }
        #expect(several?.code == .transducerCount)
        #expect(several?.message.contains("holds 2") == true)
        #expect(several?.message.contains("Transducer(data:)") == true)
        let container = thrown { _ = try OptimisedTransducer(data: Fixtures.data("animals.pmhfst")) }
        #expect(container?.code == .transducerCount)
    }

    // [spec:hfst:req:swift.load/test]
    @Test func otherFormatsAreRefusedByName() throws {
        let cases = [
            "words.openfst-tropical.hfst": "openfst-tropical",
            "words.openfst-tropical.bare": "openfst-tropical",
            "words.foma.hfst": "foma",
            "words.foma.bare": "gzip",
            "words.foma.plain": "foma",
        ]
        for (name, named) in cases {
            let data = try Fixtures.data(name)
            for refused in [
                thrown { _ = try OptimisedTransducer(data: data) },
                thrown { _ = try Transducer.all(in: data) },
            ] {
                #expect(refused?.code == .unsupportedFormat, "\(name)")
                #expect(refused?.message.contains(named) == true, "\(name)")
                #expect(refused?.message.contains("divvun.hfst does not read") == true, "\(name)")
            }
        }
    }

    // [spec:hfst:req:swift.load/test]
    @Test func mixedAndBrokenStreamsAreRefused() throws {
        #expect(thrown { _ = try Transducer.all(in: Fixtures.data("mixed.hfst")) }?.code == .typeMismatch)
        let data = try Fixtures.data("words.optimized-lookup-weighted.hfst")
        for broken in [Data(), data.dropLast(9), Data("not a transducer stream".utf8)] {
            #expect(thrown { _ = try Transducer.all(in: Data(broken)) }?.code == .invalidFormat)
        }
    }

    // [spec:hfst:req:swift.load/test]
    @Test func dhfstIsNotATransducerStream() throws {
        let speller = thrown { _ = try Transducer.all(in: Fixtures.data("speller.acceptor.dhfst")) }
        #expect(speller?.code == .unsupportedFormat)
        let stream = thrown { _ = try Dhfst(data: Fixtures.data("words.optimized-lookup-weighted.hfst")) }
        #expect(stream?.message.contains("Transducer.all(in:) reads") == true)
    }

    // [spec:hfst:req:swift.load/test]
    @Test func aTokeniserReadsOnlyPmatchContainers() throws {
        for name in ["words.optimized-lookup-weighted.hfst", "speller.acceptor.dhfst"] {
            let refused = thrown { _ = try Tokeniser(data: Fixtures.data(name)) }
            #expect(refused?.code == .invalidFormat || refused?.code == .unsupportedFormat, "\(name)")
        }
        let naive = thrown { _ = try Tokeniser(data: Fixtures.data("words.optimized-lookup-weighted.hfst")) }
        #expect(naive?.message.contains("naive tokeniser") == true)
    }

    // [spec:hfst:req:swift.version/test]
    @Test func theVersionIsTheRuntimeCratesVersion() throws {
        let manifest = try String(
            contentsOf: URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent()
                .appendingPathComponent("../../../crates/hfst-swift-runtime/Cargo.toml"),
            encoding: .utf8
        )
        let line = manifest.split(separator: "\n").first { $0.hasPrefix("version = ") }
        #expect(line == "version = \"\(HFST.version)\"")
    }
}
