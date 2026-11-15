import Foundation
import HFSTCompile
import Testing

private func words(_ type: String = "openfst-tropical") throws -> Transducer {
    try Transducer(contentsOf: Fixtures.url("words.\(type).hfst"))
}

private func paths(_ t: Transducer) throws -> [String] {
    try t.extractPaths().map { "\($0.input)\t\($0.output)\t\($0.weight)" }.sorted()
}

@Suite struct OperationsTests {
    // [spec:hfst:req:swift.transducer/test]
    @Test func transducersOfEveryTypeRead() throws {
        let cases: [(String, TransducerType)] = [
            ("words.openfst-tropical.hfst", .openFstTropical),
            ("words.openfst-tropical.bare", .openFstTropical),
            ("words.foma.hfst", .foma),
            ("words.foma.plain", .foma),
            ("words.optimized-lookup-weighted.hfst", .optimizedLookupWeighted),
            ("words.optimized-lookup-unweighted.bare", .optimizedLookupUnweighted),
        ]
        for (name, type) in cases {
            let transducer = try Transducer(data: Fixtures.data(name))
            #expect(transducer.type == type, "\(name)")
            #expect(try outputs(transducer, "cat") == ["dog"], "\(name)")
            #expect(try outputs(transducer, "mouse") == ["mice", "mouse"], "\(name)")
        }
    }

    // [spec:hfst:req:swift.transducer/test]
    @Test func aThfstDirectoryReads() throws {
        let read = try Transducer.all(contentsOf: Fixtures.url("words.thfst"))
        #expect(read.count == 1)
        #expect(read[0].type == .optimizedLookupWeighted)
        #expect(try outputs(read[0], "cat") == ["dog"])
        #expect(try Transducer(contentsOf: Fixtures.url("words.thfst")).lookup("cat") == read[0].lookup("cat"))
    }

    // [spec:hfst:req:swift.transducer/test]
    @Test func copyIsATransducerOfItsOwn() throws {
        let original = try words()
        let copy = try original.copy()
        try copy.invert()
        #expect(try outputs(original, "cat") == ["dog"])
        #expect(try outputs(copy, "dog") == ["cat"])
    }

    // [spec:hfst:req:swift.operations/test]
    @Test func operationsChangeInPlace() throws {
        let t = try words()
        try t.invert()
        #expect(try outputs(t, "dog") == ["cat"])
        try t.convert(to: .foma)
        #expect(t.type == .foma)
        try t.minimize()
        try t.determinize(encodeWeights: true)
        try t.setName("renamed")
        #expect(t.name == "renamed")
        try t.editMetadata(adding: ["source": "test"])
        #expect(t.properties["source"] == "test")
        try t.project(.input)
        #expect(try outputs(t, "dog") == ["dog"])
    }

    // [spec:hfst:req:swift.operations/test]
    @Test func binaryOperationsTakeAnotherTransducer() throws {
        let t = try regex("{cat}:{dog}").value
        try t.compose(regex("{dog}:{cow}").value)
        #expect(try outputs(t, "cat") == ["cow"])
        let u = try regex("a").value
        try u.concatenate(regex("b").value)
        #expect(try outputs(u, "ab") == ["ab"])
        try u.disjunct(regex("c").value)
        #expect(try outputs(u, "c") == ["c"])
        try u.subtract(regex("c").value)
        #expect(try outputs(u, "c").isEmpty)
        try u.conjunct(regex("{ab}").value)
        #expect(try outputs(u, "ab") == ["ab"])
    }

    // [spec:hfst:req:swift.operations/test]
    @Test func guessifyReturnsTheGenerator() throws {
        let analyser = try regex("{cat}:{cat%+N} | {dogs}:{dog%+N%+Pl}").value
        let generator = try analyser.guessify(defaultPenalty: 1)
        #expect(generator != nil)
        #expect(analyser.type == .optimizedLookupWeighted)
        #expect(try analyser.guessify(doNotCompileGenerator: true) == nil)
    }

    // [spec:hfst:req:swift.operations/test]
    @Test func theAlgebraRefusesOptimizedLookupAndStaysUsable() throws {
        let ol = try words("optimized-lookup-weighted")
        let refused = thrown { try ol.compose(try words("optimized-lookup-weighted")) }
        #expect(refused != nil)
        #expect(try outputs(ol, "cat") == ["dog"])
    }

    // [spec:hfst:req:swift.operations/test]
    @Test func optionValuesAreChecked() throws {
        let t = try words()
        #expect(thrown { try t.repeat(from: -1) }?.code == .invalidArgument)
        #expect(thrown { try t.setName("x", truncateLength: -1) }?.code == .invalidArgument)
        #expect(thrown { _ = try t.guess(["cat"], maxNumberOfGuesses: -1) }?.code == .invalidArgument)
    }

    // [spec:hfst:req:swift.operations/test]
    @Test func queriesReportWhatTheToolsReport() throws {
        let t = try words()
        let summary = try t.summarize(verbose: true)
        #expect(summary.type == .openFstTropical)
        #expect(summary.states > 0 && summary.arcs > 0)
        #expect(summary.details != nil)
        #expect(try t.summarize().details == nil)
        let alphabets = try t.dumpAlphabets()
        #expect(alphabets.seen.contains("c"))
        let comparison = try t.checkAlpha(regex("x").value)
        #expect(comparison.mismatch)
        #expect(try t.compare(t.copy()))
        #expect(try t.compare(t), "a transducer is equivalent to itself")
        let extracted = try t.extractPaths(maxStrings: 1)
        #expect(extracted.count == 1)
    }

    // [spec:hfst:req:swift.operations/test]
    @Test func lookupToolsReportEachLine() throws {
        let t = try words()
        let reports = try lookupReport([t], inputs: ["cat", "zzz"])
        #expect(reports.count == 2)
        #expect(reports[0].results?.map(\.output) == ["dog"])
        let ol = try words("optimized-lookup-weighted")
        let optimized = try optimizedLookupReport([ol], words: ["cat"])
        #expect(optimized.first?.analyses?.map(\.output) == ["dog"])
    }

    // [spec:hfst:req:swift.operations/test]
    @Test func grepPmatchAndNaiveTokenising() throws {
        let found = try grep("{cat}", in: "a cat\na dog\n")
        #expect(found.matched)
        #expect(found.lines.map(\.number) == [1])
        let tokeniser = try Tokeniser(contentsOf: Fixtures.url("tokenise.pmhfst"))
        let pieces = try tokeniser.pmatch("The cat saw a dog\n")
        #expect(pieces.count == 1 && pieces[0].marked != nil)
        let located = try tokeniser.pmatch("The cat\n", locate: true)
        #expect(located.first?.located?.isEmpty == false)
        let naive = try regex("{cat}:{cat%+N}").value.naiveTokeniser()
        #expect(try naive.tokenise("a cat").map(\.surface).contains("cat"))
    }

    // [spec:hfst:req:swift.writers/test]
    @Test func dataWritesEveryType() throws {
        let types = TransducerType.allCases
        for type in types {
            let name = "words.\(type.rawValue).hfst"
            let t = try Transducer(data: Fixtures.data(name))
            #expect(try t.data() == Fixtures.data(name), "\(name)")
            for to in types {
                #expect(try Transducer(data: t.data(type: to)).type == to, "\(name) \(to)")
            }
            let bare = try t.data(useBackendFormat: true)
            #expect(!bare.starts(with: Data("HFST\0".utf8)))
        }
    }

    // [spec:hfst:req:swift.writers/test]
    @Test func textIsWhatFst2txtWrites() throws {
        let t = try regex("{ab}:{c}::0.5").value
        #expect(try t.attText() == "0\t1\ta\tc\t0.500000\n1\t2\tb\t@0@\t0.000000\n2\t0.000000\n")
        #expect(try t.attText(doNotPrintWeights: true) == "0\t1\ta\tc\n1\t2\tb\t@0@\n2\n")
        #expect(try t.prologText().hasPrefix("network(NO_NAME_1).\n"))
        #expect(try t.dotText().hasPrefix("// This graph generated with hfst-fst2txt\n"))
        _ = try t.pckimmoText()
    }

    // [spec:hfst:req:swift.writers/test]
    @Test func dhfstAcceptorsAndErrorModels() throws {
        let lexicon = try regex("{cat} | {dog}").value
        guard case let .acceptor(acceptor) = try Dhfst(data: lexicon.dhfstAcceptorData(sourceName: "words.hfst")) else {
            Issue.record("an acceptor reads as one")
            return
        }
        #expect(acceptor.lookup("cat") == 0 && acceptor.lookup("cow") == nil)
        #expect(acceptor.meta?.contains("\"source\":\"words.hfst\"") == true)
        let errors = try regex("[a:b | b:a | a | b]*").value
        for depth in [4, 1, nil] as [Int?] {
            guard case .errorModel = try Dhfst(data: errors.dhfstErrorModelData(maxFallbackDepth: depth)) else {
                Issue.record("an error model reads as one")
                return
            }
        }
        #expect(thrown { _ = try regex("{cat}:{dog}").value.dhfstAcceptorData() } != nil)
    }

    // [spec:hfst:req:swift.writers/test]
    @Test func writeThfstWritesTheDirectory() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let t = try regex("{cat}:{dog}::0.5").value
        let thfst = directory.appendingPathComponent("words.thfst")
        try t.writeThfst(to: thfst)
        #expect(try FileManager.default.contentsOfDirectory(atPath: thfst.path).sorted() == ["alphabet", "index", "transition"])
        let read = try Transducer(contentsOf: thfst)
        #expect(try read.lookup("cat").map { "\($0.output) \($0.weight)" } == ["dog 0.5"])
        let file = directory.appendingPathComponent("a file")
        try Data().write(to: file)
        #expect(thrown { try t.writeThfst(to: file.appendingPathComponent("x.thfst")) } != nil)
    }

    // [spec:hfst:req:swift.writers/test]
    @Test func packBhfstPacksASpeller() throws {
        let lexicon = try regex("{cat} | {dog}").value
        let errors = try regex("[a:b | b:a | a | b]*").value.dhfstErrorModelData()
        for acceptor in [try lexicon.dhfstAcceptorData(), try lexicon.data()] {
            let packed = try packBhfst(acceptor: acceptor, errorModel: errors, metadata: "{\"id\": \"x\"}")
            #expect(packed.starts(with: [0xFF, 0x42, 0x4F, 0x58]))
        }
        let refused = thrown {
            _ = try packBhfst(acceptor: lexicon.dhfstAcceptorData(), errorModel: errors, spellerConfig: "{}")
        }
        #expect(refused?.code == .invalidArgument)
    }
}
