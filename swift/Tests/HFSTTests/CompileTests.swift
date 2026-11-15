import Foundation
import HFSTCompile
import Testing

func outputs(_ t: Transducer, _ word: String) throws -> [String] {
    try t.lookup(word).map(\.output).sorted()
}

@Suite struct CompileTests {
    // [spec:hfst:req:swift.compile/test]
    @Test func regexCompilesAnExpression() throws {
        let t = try regex("{cat}:{dog} | {mouse}").value
        #expect(t.type == .openFstTropical)
        #expect(t.name == "xre(?)")
        #expect(try outputs(t, "cat") == ["dog"])
        for type in [TransducerType.foma, .optimizedLookupWeighted, .optimizedLookupUnweighted] {
            let compiled = try regex("{cat}:{dog}", options: RegexOptions(format: type)).value
            #expect(compiled.type == type)
            #expect(try outputs(compiled, "cat") == ["dog"])
        }
    }

    // [spec:hfst:req:swift.compile/test]
    @Test func severalExpressionsMakeOneTransducerOnlyDisjunct() throws {
        let refused = thrown { _ = try regex("a\nb\n") }
        #expect(refused?.code == .transducerCount)
        #expect(refused?.message.contains("disjunct") == true)
        let union = try regex("a\nb\n", options: RegexOptions(disjunct: true)).value
        #expect(try outputs(union, "a") == ["a"] && outputs(union, "b") == ["b"])
        let semicolons = RegexOptions(semicolon: true, disjunct: true)
        #expect(try outputs(regex("a ; b ;", options: semicolons).value, "b") == ["b"])
    }

    // [spec:hfst:req:swift.compile/test]
    @Test func lexcIsNamedAfterItsFirstSource() throws {
        let root = "Multichar_Symbols +N\nLEXICON Root\nNouns ;\n"
        let nouns = "LEXICON Nouns\ncat+N:cat # ;\n"
        let t = try compileLexc([root, nouns], names: ["root.lexc", "nouns.lexc"]).value
        #expect(t.name == "lexc(root.lexc)")
        #expect(try outputs(t, "cat+N") == ["cat"])
        let foma = try compileLexc(["LEXICON Root\ndog # ;\n"], options: LexcOptions(format: .foma)).value
        #expect(foma.type == .foma && foma.name == "lexc(<stdin>)")
        #expect(thrown { _ = try compileLexc([root, nouns], names: ["root.lexc"]) }?.code == .invalidArgument)
    }

    // [spec:hfst:req:swift.compile/test]
    @Test func twolcReturnsItsRuleTransducers() throws {
        let grammar = "Alphabet a b c a:b ;\nSets\nCns = a b ;\nRules\n\"R1\"\nCns:0 <=> _ c ;\n\"R2\"\na:b => c _ ;\n"
        let compiled = try compileTwolc(grammar)
        #expect(compiled.warnings.contains { $0.message.contains("Alphabet") })
        #expect(Set(compiled.value.map { String($0.name.split(separator: " ")[0]) }) == ["\"R1\"", "\"R2\""])
        let foma = try compileTwolc(grammar, options: TwolcOptions(format: .foma, quiet: true)).value
        #expect(foma.allSatisfy { $0.type == .foma })
        #expect(try pairTest(compiled.value, tests: "c:c a:b\n").map(\.test) == ["c:c a:b"])
    }

    // [spec:hfst:req:swift.compile/test]
    // [spec:hfst:req:swift.compile-files/test]
    @Test func xfstReturnsWhatTheScriptLeft() throws {
        let script = "define X c ;\nregex a:b ;\nsave stack out.hfst\nprint words\n"
        let result = try compileXfst(script, files: .memory([String: Data]()))
        #expect(result.output.contains("a:b"))
        #expect(result.stack.map(\.type) == [.openFstTropical])
        #expect(try outputs(try #require(result.definitions["X"]), "c") == ["c"])
        #expect(Array(result.written.keys) == ["out.hfst"])
        let saved = try Transducer(data: try #require(result.written["out.hfst"]))
        #expect(try outputs(saved, "a") == ["b"])
        let loaded = try compileXfst(
            "load stack in.hfst\nprint words\n",
            options: XfstOptions(format: .foma),
            files: .memory(["in.hfst": try #require(result.written["out.hfst"])])
        )
        #expect(loaded.output.contains("a:b"))
        #expect(loaded.stack.map(\.type) == [.foma])
    }

    // [spec:hfst:req:swift.compile/test]
    // [spec:hfst:req:swift.compile-files/test]
    @Test func xfstReadsItsInputAndTheFileSystem() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let out = directory.appendingPathComponent("out.hfst").path
        let result = try compileXfst("regex {cat}:{dog} ;\nsave stack \(out)\napply down cat\n", input: "cat\n")
        #expect(result.output.contains("dog"))
        #expect(result.written == [out: try Data(contentsOf: URL(fileURLWithPath: out))])
    }

    // [spec:hfst:req:swift.compile-files/test]
    @Test func filesComeFromMemoryAResolverOrTheFileSystem() throws {
        let memory = CompileFiles.memory(["words.txt": "cat\ndog\n", "more.txt": "mouse\n"])
        let t = try regex("@txt\"words.txt\" | @txt\"more.txt\"", files: memory).value
        #expect(try outputs(t, "dog") == ["dog"] && outputs(t, "mouse") == ["mouse"])
        let missing = thrown { _ = try regex("@txt\"words.txt\"", files: .memory([String: Data]())) }
        #expect(missing?.code == .compileFailed)
        let asked = Asked()
        let resolved = try regex("@txt\"cows.txt\" | @txt\"cows.txt\"", files: .resolver { name in
            asked.add(name)
            return name == "cows.txt" ? Data("cow\n".utf8) : nil
        }).value
        #expect(try outputs(resolved, "cow") == ["cow"])
        #expect(asked.names == ["cows.txt"], "each file is asked for once")
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let words = directory.appendingPathComponent("words.txt")
        try Data("horse\n".utf8).write(to: words)
        #expect(try outputs(regex("@txt\"\(words.path)\"").value, "horse") == ["horse"])
    }

    // [spec:hfst:req:swift.compile/test]
    @Test func pmatchMakesAnArchiveATokeniserReads() throws {
        let script = "Define TOP [ {cat} | {dog} ] EndTag(Animal) ;\n"
        let archive = try compilePmatch(script).value
        let tokeniser = try Tokeniser(data: archive)
        #expect(try tokeniser.tokenise("a cat").map(\.surface) == ["cat"])
        let flattened = try compilePmatch(script, options: PmatchOptions(flatten: true)).value
        #expect(try Transducer.all(in: flattened)[0].name == "TOP")
    }

    // [spec:hfst:req:swift.compile-files/test]
    @Test func pmatchFindsFilesBesideItsSource() throws {
        let script = "Define TOP @txt\"animals.txt\" EndTag(Animal) ;\n"
        let beside = try compilePmatch(script, name: "lang/x.pmatch", files: .memory(["lang/animals.txt": "cow\nhorse\n"]))
        #expect(try Tokeniser(data: beside.value).tokenise("a horse").map(\.surface) == ["horse"])
        let elsewhere = thrown {
            _ = try compilePmatch(script, name: "lang/x.pmatch", files: .memory(["animals.txt": "cow\nhorse\n"]))
        }
        #expect(elsewhere?.code == .compileFailed)
    }

    // [spec:hfst:req:swift.compile/test]
    @Test func stringsAndTextCompileToOneTransducer() throws {
        let t = try compileStrings("cat:dog\t1.5\n").value
        #expect(try t.lookup("cat").map(\.weight) == [1.5])
        #expect(thrown { _ = try compileStrings("cat\ndog\n") }?.code == .transducerCount)
        let union = try compileStrings("cat\ndog\n", options: StringsOptions(format: .foma, disjunctStrings: true)).value
        #expect(try outputs(union, "dog") == ["dog"] && union.type == .foma)
        let spaced = try compileStrings("c a t\n", options: StringsOptions(hasSpaces: true, multicharSymbols: ["ca"])).value
        #expect(try outputs(spaced, "cat") == ["cat"])
        #expect(try outputs(parseAtt("0\t1\tc\td\n1\n", name: "t.att").value, "c") == ["d"])
        #expect(thrown { _ = try parseAtt("0\t1\ta\ta\n1\n--\n0\t1\tb\tb\n1\n") }?.code == .transducerCount)
        let joined = try parseAtt("0\t1\ta\ta\n1\n--\n0\t1\tb\tb\n1\n", options: AttOptions(disjunct: true)).value
        #expect(try outputs(joined, "b") == ["b"])
        let prolog = try parseProlog(
            "network(n).\narc(n, 0, 1, \"c\":\"d\").\nfinal(n, 1).\n",
            options: PrologOptions(format: .foma)
        ).value
        #expect(prolog.type == .foma)
        #expect(try outputs(prolog, "c") == ["d"])
    }

    // [spec:hfst:req:swift.compile-errors/test]
    @Test func aFailedCompileCarriesItsDiagnostics() throws {
        let source = "čáhci ]"
        let error = try #require(thrown { _ = try regex(source, name: "nouns.xre") })
        #expect(error.code == .compileFailed)
        #expect(error.message.contains("nouns.xre"))
        let diagnostic = try #require(error.diagnostics.first { $0.severity == .error })
        #expect(diagnostic.sourceName == "nouns.xre")
        #expect(diagnostic.utf8Range == 8..<9, "č and á take two bytes each")
        let range = try #require(diagnostic.range)
        #expect(source[range] == "]")
        #expect(source.distance(from: source.startIndex, to: range.lowerBound) == 6)
        #expect(diagnostic.labels.first?.range == diagnostic.range)
        #expect(!diagnostic.rendered.contains("\u{1b}["))
        #expect(diagnostic.rendered.contains("nouns.xre"))
    }

    // [spec:hfst:req:swift.compile-errors/test]
    @Test func rangesIndexTheRightSourceOnLaterLines() throws {
        let source = "Æ ø\n[ŋ |\n"
        let error = try #require(thrown { _ = try regex(source) })
        let ranges = error.diagnostics.compactMap(\.range)
        #expect(!ranges.isEmpty)
        let newline = try #require(source.firstIndex(of: "\n"))
        #expect(ranges.allSatisfy { $0.lowerBound >= newline && $0.upperBound <= source.endIndex })
    }

    // [spec:hfst:req:swift.compile-errors/test]
    @Test func aDiagnosticAboutOneExpressionIsPlacedInTheWholeSource() throws {
        let source = "{cat}\n{dog} ]\n"
        let error = try #require(thrown { _ = try regex(source, options: RegexOptions(disjunct: true)) })
        let diagnostic = try #require(error.diagnostics.first { $0.range != nil })
        let range = try #require(diagnostic.range)
        #expect(source[range] == "]")
        #expect(diagnostic.utf8Range == 12..<13)
    }

    // [spec:hfst:req:swift.compile-errors/test]
    @Test func lexcAndTwolcFailuresCarryTheirDiagnostics() throws {
        let lexc = thrown {
            _ = try compileLexc(
                ["LEXICON Root\ncat # ;\ndog Nowhere ;\n"],
                options: LexcOptions(werror: true, warnMissingLexicons: true)
            )
        }
        #expect(lexc?.diagnostics.isEmpty == false)
        let twolc = thrown { _ = try compileTwolc("Alphabet a b ;\nRules\n\"R1\"\na:b <=> _ \"c ;\n") }
        #expect(twolc?.diagnostics.contains { $0.severity == .error } == true)
    }

    // [spec:hfst:req:swift.compile-errors/test]
    @Test func warningsComeBackWithTheResult() throws {
        let source = "cát\t-1\n"
        let compiled = try compileStrings(source, name: "w.txt")
        #expect(try outputs(compiled.value, "cát") == ["cát"])
        let warning = try #require(compiled.warnings.first)
        #expect(compiled.warnings.count == 1)
        #expect(warning.severity == .warning)
        #expect(warning.sourceName == "w.txt")
        #expect(warning.utf8Range == 0..<7)
        #expect(warning.range.map { String(source[$0]) } == "cát\t-1")
        let quiet = try compileStrings(source, options: StringsOptions(warnNegativeWeights: false))
        #expect(quiet.warnings.isEmpty)
    }

    // [spec:hfst:req:swift.compile/test]
    @Test func optionsAreValues() {
        var options = RegexOptions(disjunct: true)
        #expect(options == RegexOptions(disjunct: true))
        options.semicolon = true
        #expect(options != RegexOptions(disjunct: true))
        #expect(LexcOptions().xeroxComposition, "lexc composes as Xerox does unless told not to")
        #expect(StringsOptions().epsilon == "@0@" && StringsOptions().warnNegativeWeights)
    }
}

/// The names a resolver was asked for, in order.
final class Asked: @unchecked Sendable {
    private let lock = NSLock()
    private var asked: [String] = []

    func add(_ name: String) {
        lock.lock()
        asked.append(name)
        lock.unlock()
    }

    var names: [String] {
        lock.lock()
        defer { lock.unlock() }
        return asked
    }
}
