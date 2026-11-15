import Foundation
import HFST
import Testing

/// What `hfst lookup -X print-space` printed, one list of results an input:
/// each result's output, its symbols and its weight as printed. An input
/// with no results prints one line of weight `inf`.
private func printed(_ text: String) -> [[Summary]] {
    var blocks: [[Summary]] = []
    var block: [Summary] = []
    for line in text.split(separator: "\n", omittingEmptySubsequences: false).dropLast() {
        if line.isEmpty {
            blocks.append(block)
            block = []
            continue
        }
        let fields = line.split(separator: "\t", omittingEmptySubsequences: false)
        let weight = String(fields[2])
        if weight != "inf" {
            let symbols = fields[1].split(separator: " ").map(String.init)
            block.append(Summary(output: symbols.joined(), symbols: symbols, weight: weight))
        }
    }
    return blocks
}

private struct Summary: Hashable, Comparable {
    let output: String
    let symbols: [String]
    let weight: String

    init(output: String, symbols: [String], weight: String) {
        self.output = output
        self.symbols = symbols
        self.weight = weight
    }

    init(_ result: LookupResult) {
        self.init(output: result.output, symbols: result.symbols, weight: String(format: "%.6f", result.weight))
    }

    static func < (a: Summary, b: Summary) -> Bool {
        (a.output, a.weight, a.symbols.joined(separator: " ")) < (b.output, b.weight, b.symbols.joined(separator: " "))
    }
}

@Suite struct LookupTests {
    // [spec:hfst:req:swift.lookup/test]
    @Test(arguments: ["words", "flags", "ties"], read)
    func lookupFindsWhatHfstLookupPrints(source: String, kind: String) throws {
        let transducer = try Transducer(contentsOf: Fixtures.url("\(source).\(kind).hfst"))
        let inputs = try Fixtures.text("lookup-inputs.txt")
            .split(separator: "\n", omittingEmptySubsequences: false)
            .dropLast()
            .map(String.init)
        for (limit, name) in [(nil, "lookup.\(source).\(kind).txt"), (1, "lookup.\(source).\(kind).limit1.txt")] as [(Int?, String)] {
            let expected = try printed(Fixtures.text(name))
            #expect(expected.count == inputs.count)
            for (given, wanted) in zip(inputs, expected) {
                let results = try transducer.lookup(given, limit: limit)
                #expect(results.map(Summary.init).sorted() == wanted.sorted(), "\(given) \(String(describing: limit))")
                let keys = results.map { ($0.weight, $0.output, $0.symbols.joined(separator: " ")) }
                #expect(keys.elementsEqual(keys.sorted(by: <), by: ==), "\(given)")
            }
        }
    }

    // [spec:hfst:req:swift.lookup/test]
    @Test func resultsAreValues() throws {
        let transducer = try Transducer(contentsOf: Fixtures.url("flags.optimized-lookup-weighted.hfst"))
        let results = try transducer.lookup("talo")
        try #require(results.count == 2)
        let (nominative, genitive) = (results[0], results[1])
        #expect(nominative.output == "talo+N+Nom")
        #expect(nominative.symbols == ["t", "a", "l", "o", "+N", "+Nom"])
        #expect(nominative.weight == 0.5)
        #expect(genitive.weight == 1.5)
        #expect(try transducer.lookup("talo")[0] == nominative)
        #expect(nominative != genitive)
    }

    // [spec:hfst:req:swift.lookup/test]
    @Test func aLimitStopsAfterThatManyPaths() throws {
        let many = try Transducer(contentsOf: Fixtures.url("many.optimized-lookup-weighted.hfst"))
        #expect(try many.lookup("aaaaaaa").count == 16384)
        #expect(try many.lookup("aaaaaaa", limit: 5).count == 5)
        #expect(try many.lookup("aaaaaaa", limit: 0).isEmpty)
        #expect(try many.lookup("aaaaaaa", limit: .max).count == 16384)
    }

    // [spec:hfst:req:swift.lookup/test]
    @Test func aTimeLimitKeepsWhatItFound() throws {
        let many = try Transducer(contentsOf: Fixtures.url("many.optimized-lookup-weighted.hfst"))
        let every = Set(try many.lookup("aaaaaaa").map(\.output))
        let timed = try many.lookup("aaaaaaa", timeLimit: 0.001)
        #expect(timed.count < every.count)
        #expect(timed.allSatisfy { every.contains($0.output) && $0.symbols.count == 7 })
        let words = try Transducer(contentsOf: Fixtures.url("words.optimized-lookup-weighted.hfst"))
        #expect(try words.lookup("mouse", timeLimit: 0) == words.lookup("mouse"))
    }

    // [spec:hfst:req:swift.errors/test]
    @Test func limitsAreChecked() throws {
        let transducer = try Transducer(contentsOf: Fixtures.url("words.optimized-lookup-weighted.hfst"))
        #expect(thrown { _ = try transducer.lookup("cat", limit: -1) }?.code == .invalidArgument)
        for bad in [-1.0, .nan, .infinity] {
            #expect(thrown { _ = try transducer.lookup("cat", timeLimit: bad) }?.code == .invalidArgument)
        }
        #expect(try transducer.lookup("cat", timeLimit: 1).map(\.output) == ["dog"])
        let optimised = try OptimisedTransducer(contentsOf: Fixtures.url("words.optimized-lookup-weighted.hfst"))
        #expect(thrown { _ = try optimised.lookup("cat", limit: -1) }?.code == .invalidArgument)
    }

    // [spec:hfst:req:swift.lookup/test]
    @Test(arguments: read)
    func inputWithNoAnalysesGivesNone(kind: String) throws {
        let transducer = try Transducer(contentsOf: Fixtures.url("words.\(kind).hfst"))
        let optimised = try OptimisedTransducer(contentsOf: Fixtures.url("words.\(kind).hfst"))
        for given in ["", "mice", "zzz", "ølø"] {
            #expect(try transducer.lookup(given).isEmpty)
            #expect(try optimised.lookup(given).isEmpty)
        }
    }

    // [spec:hfst:req:swift.lookup/test]
    @Test func theOptimisedLookupKeepsTheLightest() throws {
        let optimised = try OptimisedTransducer(contentsOf: Fixtures.url("words.optimized-lookup-weighted.hfst"))
        let every = try optimised.lookup("mouse")
        #expect(every.map(\.output) == ["mouse", "mice"])
        #expect(every.map(\.weight) == [1, 2])
        #expect(try optimised.lookup("mouse", limit: 1) == Array(every.prefix(1)))
        #expect(try optimised.lookup("mouse", limit: 0).isEmpty)
        #expect(try optimised.lookup("mouse", limit: .max) == every)
    }

    // [spec:hfst:req:swift.lookup/test]
    @Test func theLookupsDifferWhereDivvunspellDoes() throws {
        let url = Fixtures.url("ties.optimized-lookup-weighted.hfst")
        let hfst = try Transducer(contentsOf: url).lookup("a").map(\.output)
        let optimised = try OptimisedTransducer(contentsOf: url).lookup("a").map(\.output)
        #expect(hfst.filter { $0 == "xy" }.count == 2, "hfst gives a result for each path")
        #expect(optimised.filter { $0 == "xy" }.count == 1, "divvunspell gives each output once")
    }
}
