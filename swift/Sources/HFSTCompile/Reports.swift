import Foundation
internal import HFSTCompileGlue

// [spec:hfst:req:swift.operations]
/// How a lookup report reads each line: as text, as symbols separated by
/// spaces, or in Apertium stream format.
public enum LookupInputFormat: String, Sendable, Hashable, CaseIterable {
    case text, spaced, apertium
}

// [spec:hfst:req:swift.operations]
/// How `lookupReport` combines the results of several transducers.
public enum LookupCascade: String, Sendable, Hashable, CaseIterable {
    case union
    case priorityUnion = "priority-union"
    case composition
}

// [spec:hfst:req:swift.operations]
/// What `hfst lookup` or `hfst flookup` reports for one line.
public struct LookupReport: Sendable, Hashable {
    /// The line as it was looked up; an Apertium line loses its escapes and
    /// markup.
    public let input: String
    /// An Apertium line's bracketed markup, or `""`.
    public let markup: String
    /// The results the tool prints, in its order: none for a word with no
    /// analysis, and `nil` for a line with a symbol no transducer knows,
    /// which was not looked up.
    public let results: [HFST.LookupResult]?
    /// Whether the lookup was infinitely ambiguous, and so cut short.
    public let infinite: Bool

    init(_ report: HFSTCompileGlue.LookupReport) {
        input = report.input
        markup = report.markup
        results = report.results?.map(HFST.LookupResult.init)
        infinite = report.infinite
    }
}

// [spec:hfst:req:swift.operations]
/// An analysis `hfst optimized-lookup` prints.
public struct Analysis: Sendable, Hashable {
    public let output: String
    public let weight: Float
}

// [spec:hfst:req:swift.operations]
/// What `hfst optimized-lookup` reports for a word.
public struct OptimizedLookupReport: Sendable, Hashable {
    public let input: String
    /// The analyses it prints, in its order; `nil` for a word with a symbol
    /// no transducer knows.
    public let analyses: [Analysis]?
}

// [spec:hfst:req:swift.operations]
/// Where a rule stopped accepting a pair test.
public struct RuleFailure: Sendable, Hashable {
    public let rule: String
    /// How many of the test's pairs the rule accepted before it stopped.
    public let recognizedPrefix: Int
}

// [spec:hfst:req:swift.operations]
/// `hfst pair-test`'s report on one test.
public struct PairTestResult: Sendable, Hashable {
    /// The test as the tool shows it: the pair string, or `INPUT : OUTPUT`.
    public let test: String
    public let positive: Bool
    /// The test's pairs, word boundaries included.
    public let pairs: [SymbolPair]
    /// A positive test some rule rejected, or a negative test none did.
    public let failed: Bool
    /// For a positive test, each rule that rejected it.
    public let failures: [RuleFailure]
}

// [spec:hfst:req:swift.operations]
/// A line `hfst grep` prints.
public struct GrepLine: Sendable, Hashable {
    /// Its number in the text, from 1.
    public let number: Int
    /// The line, its matches marked.
    public let text: String
}

// [spec:hfst:req:swift.operations]
/// What `hfst grep` reports on a text.
public struct GrepReport: Sendable, Hashable {
    public let lines: [GrepLine]
    /// Whether the text counts as matching: some line matched, or with
    /// `invertMatch`, none did.
    public let matched: Bool
}

// [spec:hfst:req:swift.operations]
/// What `hfst lookup` reports for each of `inputs` with the stream
/// `transducers`. `inputFormat` reads each line as text, as symbols spaced
/// apart, or as Apertium stream format. `cascade` combines the results of
/// several transducers. Each other option is the tool's: `maxNumber` results
/// a word in optimized lookup, epsilon cycles followed `cycles` times,
/// results within `beam` of the lightest, `timeCutoff` seconds a word, flags
/// obeyed unless not `obeyFlags` and shown with `showFlags`.
public func lookupReport(
    _ transducers: [Transducer],
    inputs: [String],
    inputFormat: LookupInputFormat = .text,
    epsilonFormat: String = "",
    maxNumber: Int? = nil,
    cycles: Int = 5,
    beam: Float? = nil,
    timeCutoff: TimeInterval = 0,
    cascade: LookupCascade = .union,
    obeyFlags: Bool = true,
    showFlags: Bool = false
) throws(HFSTError) -> [LookupReport] {
    let settings = LookupSettings(
        inputFormat: inputFormat.rawValue,
        epsilonFormat: epsilonFormat,
        maxNumber: maxNumber.map(Int64.init),
        cycles: Int64(cycles),
        beam: beam,
        timeCutoff: timeCutoff,
        cascade: cascade.rawValue,
        obeyFlags: obeyFlags,
        showFlags: showFlags
    )
    return try calling {
        try HFSTCompileGlue.lookupReport(transducers: transducers.map(\.glue), inputs: inputs, settings: settings)
    }.map(LookupReport.init)
}

// [spec:hfst:req:swift.operations]
/// What `hfst flookup` reports for each of `inputs` with the stream
/// `transducers`. Like the tool, it looks up in each transducer inverted
/// unless `invert`, and then refuses optimized lookup unless `forceOL`. The
/// other options are `lookupReport`'s.
public func flookupReport(
    _ transducers: [Transducer],
    inputs: [String],
    inputFormat: LookupInputFormat = .text,
    epsilonFormat: String = "",
    cycles: Int = 5,
    beam: Float? = nil,
    timeCutoff: TimeInterval = 0,
    invert: Bool = false,
    forceOL: Bool = false,
    obeyFlags: Bool = true,
    showFlags: Bool = false
) throws(HFSTError) -> [LookupReport] {
    let settings = FlookupSettings(
        inputFormat: inputFormat.rawValue,
        epsilonFormat: epsilonFormat,
        cycles: Int64(cycles),
        beam: beam,
        timeCutoff: timeCutoff,
        invert: invert,
        forceOl: forceOL,
        obeyFlags: obeyFlags,
        showFlags: showFlags
    )
    return try calling {
        try HFSTCompileGlue.flookupReport(transducers: transducers.map(\.glue), inputs: inputs, settings: settings)
    }.map(LookupReport.init)
}

// [spec:hfst:req:swift.operations]
/// What `hfst optimized-lookup` reports for each of `words` with the union
/// of the stream `transducers`: each analysis once with `unique`, at most
/// `analyses` of them, `timeCutoff` seconds a word, only those within `beam`
/// of the lightest, and with `fast` every analysis as found.
public func optimizedLookupReport(
    _ transducers: [Transducer],
    words: [String],
    unique: Bool = false,
    analyses: Int? = nil,
    timeCutoff: TimeInterval = 0,
    beam: Float? = nil,
    fast: Bool = false
) throws(HFSTError) -> [OptimizedLookupReport] {
    try calling {
        try HFSTCompileGlue.optimizedLookupReport(
            transducers: transducers.map(\.glue),
            words: words,
            unique: unique,
            analyses: analyses.map { Int32(clamping: $0) },
            timeCutoff: timeCutoff,
            beam: beam,
            fast: fast
        )
    }.map { report in
        OptimizedLookupReport(
            input: report.input,
            analyses: report.analyses?.map { Analysis(output: $0.output, weight: $0.weight) }
        )
    }
}

// [spec:hfst:req:swift.operations]
/// What `hfst pair-test` reports with the twolc rule transducers `rules` on
/// `tests`: each line a pair string, a negative test with `negativeTest`,
/// or with `xeroxMode` the text of a twolc source holding test cases. Blank
/// and `!` comment lines are not tests.
public func pairTest(
    _ rules: [Transducer],
    tests: String,
    negativeTest: Bool = false,
    xeroxMode: Bool = false
) throws(HFSTError) -> [PairTestResult] {
    try calling {
        try HFSTCompileGlue.pairTest(
            rules: rules.map(\.glue),
            tests: tests,
            negativeTest: negativeTest,
            xeroxMode: xeroxMode
        )
    }.map { result in
        PairTestResult(
            test: result.test,
            positive: result.positive,
            pairs: result.pairs.map(SymbolPair.init),
            failed: result.failed,
            failures: result.failures.map {
                RuleFailure(rule: $0.rule, recognizedPrefix: Int($0.recognizedPrefix))
            }
        )
    }
}

// [spec:hfst:req:swift.operations]
/// What `hfst grep` reports for `pattern`, a Xerox regular expression or
/// with `fixedStrings` a string, on `text`: the lines that match, or with
/// `invertMatch` those that do not, their matches marked with `{{{ }}}` or
/// with `color` colour codes, at most `maxCount` of them unless it is 0.
/// `wordRegexp` matches only between spaces, `lineRegexp` whole lines.
public func grep(
    _ pattern: String,
    in text: String,
    fixedStrings: Bool = false,
    color: Bool = false,
    wordRegexp: Bool = false,
    lineRegexp: Bool = false,
    invertMatch: Bool = false,
    maxCount: Int = 0
) throws(HFSTError) -> GrepReport {
    let report = try calling {
        try HFSTCompileGlue.grep(
            pattern: pattern,
            text: text,
            fixedStrings: fixedStrings,
            color: color,
            wordRegexp: wordRegexp,
            lineRegexp: lineRegexp,
            invertMatch: invertMatch,
            maxCount: UInt64(clamping: maxCount)
        )
    }
    return GrepReport(
        lines: report.lines.map { GrepLine(number: Int($0.number), text: $0.text) },
        matched: report.matched
    )
}
