import Foundation
import HFSTCompile
import Testing

// The package against the hfst tools. js/test/compile-fixtures.mjs runs the
// tools on the inputs the repository's tests give them and writes
// compile-parity.json beside the fixtures: each case's call as divvun-hfst
// names it, the tool's arguments, whether the tool succeeded, and what it
// wrote. The npm and Python packages' suites run the same cases. Here each
// runs through HFSTCompile, its divvun-hfst names and keywords read as the
// Swift call takes them. A compiler, operation or writer gives the tool's
// bytes or text, byte for byte; a query gives data, which printed as the
// tool prints it is what the tool printed. Where the tool failed, the call
// throws.

nonisolated(unsafe) private let manifest: [String: Any] = {
    (try? Fixtures.json("compile-parity.json") as? [String: Any]) ?? [:]
}()

private let inputs = manifest["inputs"] as? [String: String] ?? [:]

/// Every transducer in the stream the manifest names `name`.
private func all(_ name: String) throws -> [Transducer] {
    try Transducer.all(in: Fixtures.data(inputs[name] ?? name))
}

private func one(_ name: String) throws -> Transducer {
    try Transducer(data: Fixtures.data(inputs[name] ?? name))
}

private func stream(_ transducers: [Transducer]) throws -> Data {
    try transducers.reduce(into: Data()) { $0 += try $1.data() }
}

/// A case's keyword arguments, or its compiler's options, read as the Swift
/// call takes them. Each read marks its key: `check` fails a case that
/// leaves one unread, which would pass without the option it names.
private final class Arguments {
    private let given: [String: Any]
    private var read: Set<String> = []

    init(_ given: Any?) {
        self.given = given as? [String: Any] ?? [:]
    }

    var unread: [String] { given.keys.filter { !read.contains($0) }.sorted() }

    func has(_ key: String) -> Bool { given[key] != nil }

    func value(_ key: String) -> Any? {
        read.insert(key)
        guard let value = given[key], !(value is NSNull) else { return nil }
        return value
    }

    func typed<T>(_ key: String) -> T? {
        guard let value = value(key) else { return nil }
        guard let typed = value as? T else {
            Issue.record("\(key) is \(value), not a \(T.self)")
            return nil
        }
        return typed
    }

    func bool(_ key: String) -> Bool { typed(key) ?? false }
    func int(_ key: String) -> Int? { typed(key) }
    func string(_ key: String) -> String? { typed(key) }
    func float(_ key: String) -> Float? { (typed(key) as Double?).map(Float.init) }

    func choice<T: RawRepresentable>(_ key: String) -> T? where T.RawValue == String {
        guard let name: String = typed(key) else { return nil }
        guard let chosen = T(rawValue: name) else {
            Issue.record("\(key) is \(name), not a \(T.self)")
            return nil
        }
        return chosen
    }

    /// The text of the fixture a `{"$text": name}` value names.
    func text(_ key: String) throws -> String? {
        guard let tagged: [String: String] = typed(key), let name = tagged["$text"] else { return nil }
        return try Fixtures.text(name)
    }

    /// The transducer a `{"$transducer": name}` value names.
    func transducer(_ key: String) throws -> Transducer? {
        guard let tagged: [String: String] = typed(key), let name = tagged["$transducer"] else { return nil }
        return try one(name)
    }

    /// A bound of `repeat`: a count, or `{"$float": "inf"}` for none.
    func count(_ key: String) -> RepeatCount? {
        guard let value = value(key) else { return nil }
        if let times = value as? Int { return .times(times) }
        if let tagged = value as? [String: String], tagged["$float"] == "inf" { return .unbounded }
        Issue.record("\(key) is \(value), not a count")
        return nil
    }

    /// Sets `field` to the value under `key`, when there is one.
    func set<T>(_ field: inout T, _ key: String) {
        if let value: T = typed(key) { field = value }
    }

    func set(_ field: inout TransducerType, _ key: String) {
        if let value: TransducerType = choice(key) { field = value }
    }
}

private struct Case {
    let entry: [String: Any]
    let kwargs: Arguments
    let options: Arguments

    init(_ entry: [String: Any]) {
        self.entry = entry
        kwargs = Arguments(entry["kwargs"])
        options = Arguments(entry["options"])
    }

    func string(_ key: String) -> String { entry[key] as? String ?? "" }
    func strings(_ key: String) -> [String] { entry[key] as? [String] ?? [] }
    var ok: Bool { entry["ok"] as? Bool ?? false }
    var tool: String { "hfst " + strings("tool").joined(separator: " ") }

    var bytes: Data { get throws { try Fixtures.data(string("expected")) } }
    var printed: String { get throws { try Fixtures.text(string("expected")) } }
}

private func cases(_ kinds: Set<String>) -> [Case] {
    (manifest["cases"] as? [[String: Any]] ?? [])
        .filter { kinds.contains($0["kind"] as? String ?? "") }
        .map(Case.init)
}

/// Text compared byte for byte: Swift's `==` on strings holds canonically
/// equivalent ones equal, which would hide output in another normal form.
private func same<T: Equatable>(_ ours: T, _ theirs: T) -> Bool {
    if let ours = ours as? String, let theirs = theirs as? String {
        return ours.utf8.elementsEqual(theirs.utf8)
    }
    return ours == theirs
}

/// Runs each case of `kinds`. A case the tool did not fail gives what
/// `expected` reads, unless `run` checks it itself and gives `nil`; a case
/// it failed throws. A kind with no cases is an issue: the expectations are
/// missing, not met.
private func check<T: Equatable>(
    _ kinds: Set<String>,
    _ expected: (Case) throws -> T,
    _ run: (Case) throws -> T?
) throws {
    let all = cases(kinds)
    #expect(!all.isEmpty, "compile-parity.json holds no \(kinds.sorted()) cases")
    var found: [String] = []
    for c in all {
        let got: T?
        do {
            got = try run(c)
        } catch let error as HFSTError {
            if c.ok { found.append("\(c.tool): threw \(error)") }
            continue
        }
        let unread = c.kwargs.unread + c.options.unread
        if !unread.isEmpty {
            found.append("\(c.tool): the call reads no \(unread)")
        } else if !c.ok {
            found.append("\(c.tool): the tool failed, and the package did not")
        } else if let got, case let theirs = try expected(c), !same(got, theirs) {
            found.append("\(c.tool): \(String(describing: got).prefix(300))\n  tool: \(String(describing: theirs).prefix(300))")
        }
    }
    #expect(found.isEmpty, "\(found.count) of \(all.count) cases differ:\n\(found.joined(separator: "\n"))")
}

/// A weight as the tools print a 32-bit float on its own: the fewest digits
/// that read back as it, never with an exponent.
private func shown(_ weight: Float) -> String {
    if weight.isInfinite { return weight > 0 ? "inf" : "-inf" }
    return NSDecimalNumber(string: "\(weight)", locale: Locale(identifier: "en_US_POSIX")).stringValue
}

private func fixed(_ weight: Float) -> String {
    String(format: "%.6f", Double(weight))
}

/// The AT&T text of each transducer, as the tool reads them one by one,
/// each with the `--` line that ends it.
private func attPieces(_ text: String) -> [String] {
    var pieces = [""]
    var rest = Substring(text)
    while !rest.isEmpty {
        let end = rest.firstIndex(of: "\n").map(rest.index(after:)) ?? rest.endIndex
        let line = rest[..<end]
        rest = rest[end...]
        pieces[pieces.count - 1] += line
        if line.trimmingCharacters(in: .whitespacesAndNewlines) == "--" { pieces.append("") }
    }
    return pieces.filter { !$0.isEmpty }
}

/// What `hfst lookup` prints for `reports`, in its Xerox format.
private func lookupText(_ reports: [LookupReport], cycles: Bool) -> String {
    var text = ""
    for report in reports {
        guard let results = report.results, !results.isEmpty else {
            text += "\(report.input)\t\(report.input)+?\tinf\n\n"
            continue
        }
        for result in results { text += "\(report.input)\t\(result.output)\t\(fixed(result.weight))\n" }
        text += cycles && report.infinite ? "\(report.input)\t[...cyclic...]\n\n" : "\n"
    }
    return text
}

/// The divvun-hfst operation `method` with `args`, on `t`.
private func apply(_ method: String, _ args: Arguments, to t: Transducer) throws {
    switch method {
    case "affix_guessify":
        if args.has("direction") || args.has("weight") {
            try t.affixGuessify(direction: args.choice("direction") ?? .suffix, weight: args.float("weight") ?? 1)
        } else {
            try t.affixGuessify()
        }
    case "determinize": try t.determinize(encodeWeights: args.bool("encode_weights"))
    case "edit_metadata":
        try t.editMetadata(adding: args.typed("add") ?? [:], truncateLength: args.int("truncate_length") ?? 0)
    case "eliminate_flags": try t.eliminateFlags()
    case "expand_equivalences": try t.expandEquivalences(from: args.string("from_"), to: args.string("to"))
    case "insert_freely": try t.insertFreely(args.string("symbol_pair") ?? "")
    case "invert": try t.invert()
    case "kill_paths": try t.killPaths(symbol: args.string("symbol"))
    case "minimize": try t.minimize(encodeWeights: args.bool("encode_weights"))
    case "preprocess_for_optimized_lookup_format": try t.preprocessForOptimizedLookupFormat()
    case "project":
        if let tape: Tape = args.choice("project") { try t.project(tape) } else { try t.project() }
    case "prune_alphabet": try t.pruneAlphabet(force: args.bool("force"))
    case "push_labels":
        if let push: PushDirection = args.choice("push") { try t.pushLabels(to: push) } else { try t.pushLabels() }
    case "push_weights":
        if let push: PushDirection = args.choice("push") { try t.pushWeights(to: push) } else { try t.pushWeights() }
    case "realign": try t.realign()
    case "remove_epsilons": try t.removeEpsilons()
    case "repeat": try t.repeat(from: args.count("from_") ?? 0, to: args.count("to") ?? .unbounded)
    case "reverse": try t.reverse()
    case "reweight":
        try t.reweight(
            addition: args.float("addition") ?? 0,
            multiplier: args.float("multiplier") ?? 1,
            function: args.choice("function"),
            symbol: args.string("symbol")
        )
    case "set_name":
        let given: [String] = args.typed("$args") ?? []
        try t.setName(given.first ?? "")
    case "substitute":
        try t.substitute(
            fromLabel: args.string("from_label"),
            toLabel: args.string("to_label"),
            toTransducer: args.transducer("to_transducer"),
            fromFile: args.text("from_file"),
            fileName: args.string("file_name") ?? "",
            inOrder: args.bool("in_order"),
            compose: args.bool("compose")
        )
    default:
        Issue.record("no operation \(method)")
    }
}

/// The divvun-hfst binary operation `method` with `args`, on `t` and `other`.
private func apply(_ method: String, _ args: Arguments, to t: Transducer, with other: Transducer) throws {
    switch method {
    case "compose":
        try t.compose(
            other,
            doNotHarmonize: args.bool("do_not_harmonize"),
            harmonizeFlags: args.bool("harmonize_flags"),
            xeroxComposition: args.bool("xerox_composition")
        )
    case "concatenate": try t.concatenate(other, harmonizeFlags: args.bool("harmonize_flags"))
    case "conjunct": try t.conjunct(other, harmonizeFlags: args.bool("harmonize_flags"))
    case "subtract": try t.subtract(other, harmonizeFlags: args.bool("harmonize_flags"))
    case "disjunct": try t.disjunct(other)
    case "priority_disjunct": try t.priorityDisjunct(other)
    case "shuffle": try t.shuffle(other)
    default: Issue.record("no binary operation \(method)")
    }
}

/// The fixture texts a case compiles, each under its name.
private func sources(_ c: Case) throws -> [(name: String, text: String)] {
    try c.strings("sources").map { ($0, try Fixtures.text($0)) }
}

private func source(_ c: Case) throws -> (name: String, text: String) {
    let all = try sources(c)
    try #require(all.count == 1, "\(c.tool) compiles \(all.count) sources")
    return all[0]
}

@Suite struct CompileParityTests {
    // [spec:hfst:req:swift.compile-parity/test]
    @Test func eachTypeIsWrittenAsFst2fstWritesIt() throws {
        for name in ["words", "words.foma", "words.olw", "words.olu"] {
            #expect(try one(name).data() == Fixtures.data(inputs[name] ?? name), "\(name)")
        }
        try check(["to_bytes"], { try $0.bytes }) { c in
            let type = try #require(TransducerType(rawValue: c.string("type")))
            return try one(c.string("input")).data(type: type, useBackendFormat: c.entry["use_backend_format"] as? Bool ?? false)
        }
    }

    // [spec:hfst:req:swift.compile-parity/test]
    @Test func operationsWriteWhatTheToolsWrite() throws {
        try check(["unary"], { try $0.bytes }) { c in
            let transducers = try all(c.string("input"))
            for t in transducers { try apply(c.string("method"), c.kwargs, to: t) }
            return try stream(transducers)
        }
        try check(["binary"], { try $0.bytes }) { c in
            let t = try one(c.string("first"))
            let other = c.entry["second"] is String ? try one(c.string("second")) : t
            try apply(c.string("method"), c.kwargs, to: t, with: other)
            return try t.data()
        }
        try check(["compose_intersect"], { try $0.bytes }) { c in
            let t = try one(c.string("input"))
            try t.composeIntersect(
                c.strings("rules").map(one),
                invert: c.kwargs.bool("invert"),
                fast: c.kwargs.bool("fast"),
                encodeWeights: c.kwargs.bool("encode_weights"),
                harmonize: c.kwargs.bool("harmonize"),
                lexiconName: c.kwargs.string("lexicon_name") ?? "",
                rulesName: c.kwargs.string("rules_name") ?? ""
            )
            return try t.data()
        }
        try check(["guessify"], { try $0.bytes }) { c in
            let t = try one(c.string("input"))
            let generator =
                c.kwargs.has("default_penalty") || c.kwargs.has("do_not_compile_generator")
                ? try t.guessify(
                    defaultPenalty: c.kwargs.float("default_penalty") ?? 1,
                    doNotCompileGenerator: c.kwargs.bool("do_not_compile_generator")
                )
                : try t.guessify()
            return try t.data() + (generator?.data() ?? Data())
        }
    }

    // [spec:hfst:req:swift.compile-parity/test]
    @Test func writersWriteWhatTheToolsWrite() throws {
        try check(["text"], { try $0.printed }) { c in
            let t = try one(c.string("input"))
            let args = c.kwargs
            switch c.string("method") {
            case "to_att":
                return try t.attText(
                    printWeights: args.bool("print_weights"),
                    doNotPrintWeights: args.bool("do_not_print_weights"),
                    useNumbers: args.bool("use_numbers")
                )
            case "to_dot": return try t.dotText()
            case "to_pckimmo": return try t.pckimmoText()
            case "to_prolog": return try t.prologText()
            default:
                Issue.record("no writer \(c.string("method"))")
                return nil
            }
        }
        try check(["dhfst"], { try $0.bytes }) { c in
            let t = try Transducer(data: Fixtures.data(c.string("source")))
            let args = c.kwargs
            let sourceName = args.string("source_name") ?? ""
            switch c.string("method") {
            case "to_dhfst_acceptor":
                return try t.dhfstAcceptorData(quick: args.bool("quick"), sourceName: sourceName)
            case "to_dhfst_error_model":
                if args.has("max_fallback_depth") {
                    return try t.dhfstErrorModelData(maxFallbackDepth: args.int("max_fallback_depth"), sourceName: sourceName)
                }
                return try t.dhfstErrorModelData(sourceName: sourceName)
            default:
                Issue.record("no writer \(c.string("method"))")
                return nil
            }
        }
        try check(["bhfst"], { try $0.bytes }) { c in
            try packBhfst(
                acceptor: Fixtures.data(c.string("acceptor")),
                errorModel: Fixtures.data(c.string("error_model")),
                metadata: Fixtures.text(c.string("metadata")),
                indexXML: c.kwargs.bool("index_xml"),
                spellerConfig: c.kwargs.text("speller_config")
            )
        }
        let scratch = FileManager.default.temporaryDirectory
            .appendingPathComponent("divvun-hfst-thfst-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: scratch) }
        try check(["thfst"], { try $0.bytes }) { c in
            let expected = c.string("expected")
            let directory = scratch.appendingPathComponent((expected as NSString).lastPathComponent)
            try one(c.string("input")).writeThfst(to: directory)
            for member in ["alphabet", "index", "transition"] {
                let written = try Data(contentsOf: directory.appendingPathComponent(member))
                #expect(try written == Fixtures.data("\(expected)/\(member)"), "\(c.tool) \(member)")
            }
            return nil
        }
    }

    // [spec:hfst:req:swift.compile-parity/test]
    @Test func compilersWriteWhatTheToolsWrite() throws {
        try check(["regex"], { try $0.bytes }) { c in
            let (name, text) = try source(c)
            var options = RegexOptions()
            c.options.set(&options.format, "format")
            c.options.set(&options.semicolon, "semicolon")
            c.options.set(&options.disjunct, "disjunct")
            c.options.set(&options.doNotHarmonize, "do_not_harmonize")
            c.options.set(&options.harmonizeFlags, "harmonize_flags")
            c.options.set(&options.doNotMinimize, "do_not_minimize")
            c.options.set(&options.encodeWeights, "encode_weights")
            c.options.set(&options.xeroxComposition, "xerox_composition")
            c.options.set(&options.flagIsEpsilon, "flag_is_epsilon")
            return try regex(text, options: options, name: name).value.data()
        }
        try check(["lexc"], { try $0.bytes }) { c in
            let named = try sources(c)
            var options = LexcOptions()
            c.options.set(&options.format, "format")
            c.options.set(&options.alignStrings, "align_strings")
            c.options.set(&options.withFlags, "with_flags")
            c.options.set(&options.minimizeFlags, "minimize_flags")
            c.options.set(&options.renameFlags, "rename_flags")
            c.options.set(&options.quiet, "quiet")
            return try compileLexc(named.map(\.text), options: options, names: named.map(\.name)).value.data()
        }
        try check(["twolc"], { try $0.bytes }) { c in
            var options = TwolcOptions()
            c.options.set(&options.format, "format")
            c.options.set(&options.resolve, "resolve")
            c.options.set(&options.dontResolveRight, "dont_resolve_right")
            c.options.set(&options.quiet, "quiet")
            return try stream(compileTwolc(c.string("text"), options: options).value)
        }
        try check(["pmatch2fst"], { try $0.bytes }) { c in
            let (name, text) = try source(c)
            var options = PmatchOptions()
            c.options.set(&options.flatten, "flatten")
            return try compilePmatch(text, options: options, name: name).value
        }
        try check(["strings"], { try $0.bytes }) { c in
            let (name, text) = try source(c)
            var options = StringsOptions()
            c.options.set(&options.format, "format")
            c.options.set(&options.disjunctStrings, "disjunct_strings")
            c.options.set(&options.norm, "norm")
            c.options.set(&options.pairstrings, "pairstrings")
            c.options.set(&options.hasSpaces, "has_spaces")
            if options.disjunctStrings {
                return try compileStrings(text, options: options, name: name).value.data()
            }
            let lines = text.split(separator: "\n").map { String($0) + "\n" }
            return try stream(lines.map { try compileStrings($0, options: options, name: name).value })
        }
        try check(["att"], { try $0.bytes }) { c in
            let (name, text) = try source(c)
            var options = AttOptions()
            c.options.set(&options.format, "format")
            c.options.set(&options.disjunct, "disjunct")
            if options.disjunct {
                return try parseAtt(text, options: options, name: name).value.data()
            }
            return try stream(attPieces(text).map { try parseAtt($0, options: options, name: name).value })
        }
        try check(["prolog"], { try $0.bytes }) { c in
            let (name, text) = try source(c)
            var options = PrologOptions()
            c.options.set(&options.format, "format")
            return try parseProlog(text, options: options, name: name).value.data()
        }
    }

    // [spec:hfst:req:swift.compile-parity/test]
    @Test func xfstScriptsRunAsTheToolRunsThem() throws {
        try check(["xfst"], { try $0.bytes }) { c in
            var options = XfstOptions()
            c.options.set(&options.format, "format")
            c.options.set(&options.printWeight, "print_weight")
            let expected = c.string("expected")
            let result = try compileXfst(
                c.string("script"),
                options: options,
                name: "s.xfst",
                files: .memory([String: Data]())
            )
            #expect(try same(result.output, Fixtures.text("\(expected)/stdout")), "\(c.tool)")
            #expect(result.written.keys.sorted() == c.strings("written"), "\(c.tool)")
            for name in c.strings("written") {
                #expect(try result.written[name] == Fixtures.data("\(expected)/\(name)"), "\(c.tool) \(name)")
            }
            #expect(result.warnings.contains { $0.message.contains("empty stack") }, "\(c.tool)")
            return nil
        }
    }

    // [spec:hfst:req:swift.compile-parity/test]
    @Test func extractPathsFindsWhatFst2stringsPrints() throws {
        try check(["extract_paths"], { try $0.printed }) { c in
            let args = c.kwargs
            let paths = try one(c.string("input")).extractPaths(
                maxStrings: args.int("max_strings"),
                nbest: args.int("nbest"),
                beam: args.float("beam"),
                obeyFlags: args.bool("obey_flags"),
                maxInLength: args.int("max_in_length"),
                inPrefix: args.string("in_prefix") ?? ""
            )
            return paths.map { path in
                let identity = path.pairs.allSatisfy { $0.input == $0.output }
                let side = identity ? path.input : "\(path.input):\(path.output)"
                return "\(side)\t\(shown(path.weight))\n"
            }.joined()
        }
    }

    // [spec:hfst:req:swift.compile-parity/test]
    @Test func lookupsReportWhatTheToolsPrint() throws {
        try check(["lookup_report"], { try $0.printed }) { c in
            let reports = try lookupReport(
                c.strings("inputs").map(one),
                inputs: c.strings("lines"),
                beam: c.kwargs.float("beam"),
                showFlags: c.kwargs.bool("show_flags")
            )
            return lookupText(reports, cycles: true)
        }
        try check(["flookup_report"], { try $0.printed }) { c in
            let reports = try flookupReport(
                c.strings("inputs").map(one),
                inputs: c.strings("lines"),
                invert: c.kwargs.bool("invert")
            )
            return lookupText(reports, cycles: false)
        }
        try check(["optimized_lookup_report"], { try $0.printed }) { c in
            let transducers = try c.strings("inputs").map(one)
            let weighted = transducers.first?.type == .optimizedLookupWeighted
            let reports = try optimizedLookupReport(
                transducers,
                words: c.strings("lines"),
                unique: c.kwargs.bool("unique"),
                analyses: c.kwargs.int("analyses"),
                beam: c.kwargs.float("beam")
            )
            var text = ""
            for report in reports {
                let word = report.input
                guard let analyses = report.analyses else {
                    text += "\(word)\t\(word)\t+?\n\n\n\n"
                    continue
                }
                if analyses.isEmpty {
                    text += "\(word)\t\(word)\t+?" + (weighted ? "\n\n" : "\n\n\n\n")
                    continue
                }
                for analysis in analyses {
                    text += "\(word)\t\(analysis.output)" + (weighted ? "\t\(shown(analysis.weight))" : "") + "\n"
                }
                text += "\n"
            }
            return text
        }
    }

    // [spec:hfst:req:swift.compile-parity/test]
    @Test func reportsAreWhatTheToolsPrint() throws {
        // compare exits 1 for transducers that differ, so a case it printed
        // for is a comparison made.
        let compared = cases(["compare"])
        #expect(!compared.isEmpty)
        for c in compared {
            let first = try one(c.string("first"))
            let second = try one(c.string("second"))
            let printed = try c.printed
            if printed.isEmpty {
                #expect(thrown { _ = try first.compare(second) } != nil, "\(c.tool)")
                continue
            }
            let equal = try first.compare(second)
            let names = [(first, "first"), (second, "second")].map { t, key in
                t.name.isEmpty ? inputs[c.string(key)] ?? "" : t.name
            }
            #expect(same("\(names[0]) \(equal ? "==" : "!=") \(names[1])\n", printed), "\(c.tool)")
        }
        try check(["dump_alphabets"], { try $0.printed }) { c in
            let alphabets = try one(c.string("input")).dumpAlphabets()
            return (alphabets.declared + alphabets.seen).map { "\($0)\n" }.joined()
        }
        try check(["check_alpha"], { try $0.printed }) { c in
            let first = try one(c.string("first"))
            let second = try one(c.string("second"))
            let comparison = try first.checkAlpha(second)
            let printed = try c.printed
            let firstOnly = comparison.firstUsed.filter { !comparison.secondUsed.contains($0) }
            let secondOnly = comparison.secondDeclared.filter { !comparison.firstDeclared.contains($0) }
            #expect(comparison.mismatch)
            #expect(printed.contains("In first \(first.name) but not in second \(second.name):\(firstOnly.joined(separator: ", "))\n"))
            #expect(printed.contains("Second \(second.name) has but first \(first.name) does not: \(secondOnly.joined(separator: ", "))\n"))
            return nil
        }
        try check(["summarize"], { try $0.printed }) { c in
            let summary = try one(c.string("input")).summarize(verbose: true)
            let printed = try c.printed
            #expect(printed.contains("# of states: \(summary.states)\n"), "\(c.tool)")
            #expect(printed.contains("# of arcs: \(summary.arcs)\n"), "\(c.tool)")
            return nil
        }
    }

    // [spec:hfst:req:swift.compile-parity/test]
    @Test func textQueriesAreWhatTheToolsPrint() throws {
        try check(["grep"], { try $0.printed }) { c in
            let report = try grep(
                c.string("pattern"),
                in: c.string("text"),
                fixedStrings: c.kwargs.bool("fixed_strings"),
                wordRegexp: c.kwargs.bool("word_regexp"),
                invertMatch: c.kwargs.bool("invert_match")
            )
            return report.lines.map { "\($0.text)\n" }.joined()
        }
        try check(["pmatch"], { try $0.printed }) { c in
            let tokeniser = try Tokeniser(data: Fixtures.data(inputs[c.string("archive")] ?? ""))
            let newline = c.kwargs.bool("newline")
            var text = ""
            for piece in try tokeniser.pmatch(c.string("text"), newline: newline, locate: c.kwargs.bool("locate")) {
                if let marked = piece.marked {
                    text += marked + "\n" + (newline ? "" : "\n")
                } else if let located = piece.located, !located.isEmpty {
                    for match in located {
                        text += "\(match.start)|\(match.length)|\(match.output)|\(match.tag)|\(shown(match.weight))\n"
                    }
                    text += "\n"
                }
            }
            return text
        }
        try check(["guess"], { try $0.printed }) { c in
            let both = try all(c.string("guesser"))
            try #require(both.count == 2, "\(c.tool): a guesser and its generator")
            return try both[0].guess(c.strings("words"), generator: both[1]).map { report in
                report.guesses.map { "\(report.input)\t\($0)\n" }.joined() + "\n"
            }.joined()
        }
        try check(["naive_tokeniser"], { try $0.printed }) { c in
            try one(c.string("input")).naiveTokeniser().format(c.string("text"), as: .segment)
        }
        let pairTests = cases(["pair_test"])
        #expect(!pairTests.isEmpty)
        for c in pairTests {
            let results = try pairTest(
                all(c.string("rules")),
                tests: c.string("tests"),
                negativeTest: c.kwargs.bool("negative_test")
            )
            #expect(results.contains(where: \.failed) != c.ok, "\(c.tool)")
            let printed = try c.printed
            for failure in results.flatMap(\.failures) {
                #expect(printed.contains("Rule \(failure.rule) fails:"), "\(c.tool)")
            }
        }
    }
}
