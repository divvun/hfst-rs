import Foundation
import HFST
import Testing

private func acceptor(_ name: String) throws -> DhfstAcceptor {
    guard case let .acceptor(acceptor) = try Dhfst(contentsOf: Fixtures.url(name)) else {
        throw NotAnAcceptor()
    }
    return acceptor
}

private struct NotAnAcceptor: Error {}

@Suite struct DhfstTests {
    // [spec:hfst:req:swift.dhfst/test]
    @Test func anAcceptorDescribesItself() throws {
        let speller = try acceptor("speller.acceptor.dhfst")
        #expect(speller.version == 1)
        #expect(speller.symbols.first == "@_EPSILON_SYMBOL_@")
        #expect(speller.meta == nil || speller.meta?.hasPrefix("{") == true)
        let info = speller.info
        #expect(info.slots >= info.ids + info.labelMax)
        #expect(info.arcs >= info.freeArcs)
        #expect(info == speller.info)
    }

    // [spec:hfst:req:swift.dhfst/test]
    @Test func anAcceptorReadsWordsAsGraphemes() throws {
        let graphemes = try acceptor("graphemes.acceptor.dhfst")
        #expect(graphemes.lookup("ija") == 2.0, "i, j and a, not the symbol ij")
        #expect(graphemes.lookup("\u{e9}") == 0.25)
        #expect(graphemes.lookup("e\u{301}") == nil, "only the composed form is a symbol")
        #expect(graphemes.lookup("xy") == nil, "xy is one symbol, but two graphemes")
        #expect(graphemes.lookup("") == nil)
    }

    // [spec:hfst:req:swift.dhfst/test]
    @Test func anErrorModelDescribesItself() throws {
        guard case let .errorModel(model) = try Dhfst(data: Fixtures.data("errmodel.error-model.dhfst")) else {
            Issue.record("the error model reads as one")
            return
        }
        #expect(model.version == 1)
        #expect(model.stateCount > 0)
        #expect(model.symbols.first == "@_EPSILON_SYMBOL_@")
    }

    // [spec:hfst:req:swift.dhfst/test]
    @Test func bytesThatAreNotDhfstAreRefused() throws {
        let whole = try Fixtures.data("speller.acceptor.dhfst")
        for broken in [whole.dropLast(9), whole.prefix(6), Data(), Data("not a speller".utf8)] {
            #expect(thrown { _ = try Dhfst(data: Data(broken)) }?.code == .invalidFormat)
        }
        #expect(thrown { _ = try Dhfst(data: Fixtures.data("words.foma.hfst")) }?.code == .unsupportedFormat)
    }
}
