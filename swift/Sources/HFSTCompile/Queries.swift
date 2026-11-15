import Foundation
internal import HFSTCompileGlue
internal import HFSTGlue

// [spec:hfst:req:swift.operations]
/// A pair of symbols an arc carries, input then output; an epsilon is `""`.
public struct SymbolPair: Sendable, Hashable {
    public let input: String
    public let output: String

    init(input: String, output: String) {
        self.input = input
        self.output = output
    }

    init(_ pair: HFSTCompileGlue.SymbolPair) {
        self.init(input: pair.input, output: pair.output)
    }
}

// [spec:hfst:req:swift.operations]
/// A symbol and how many arcs out of one state carry it.
public struct SymbolCount: Sendable, Hashable {
    public let symbol: String
    public let count: Int
}

// [spec:hfst:req:swift.operations]
/// A pair of symbols and how many arcs carry it.
public struct SymbolPairCount: Sendable, Hashable {
    public let pair: SymbolPair
    public let count: Int
}

// [spec:hfst:req:swift.operations]
/// What `hfst summarize --verbose` measures besides the summary.
public struct SummaryDetails: Sendable, Hashable {
    public let infinitelyAmbiguous: Bool
    /// The symbols a path can start with; `nil` for optimized lookup, which
    /// has no graph to walk.
    public let firstInputSymbols: [String]?
}

// [spec:hfst:req:swift.operations]
/// What `hfst summarize` reports on a transducer.
public struct Summary: Sendable, Hashable {
    public let name: String
    public let type: TransducerType
    public let states: Int
    public let finalStates: Int
    public let arcs: Int
    /// Arcs with an epsilon on both sides.
    public let ioEpsilons: Int
    public let inputEpsilons: Int
    public let outputEpsilons: Int
    /// The most arcs a state has.
    public let densestArcs: Int
    /// The fewest arcs a state has.
    public let sparsestArcs: Int
    public let uniqueInputArcs: Int
    public let uniqueOutputArcs: Int
    /// The input symbol with the most arcs out of one state, and how many.
    public let mostAmbiguousInput: SymbolCount
    public let mostAmbiguousOutput: SymbolCount
    /// The symbols the arcs use.
    public let foundAlphabet: [String]
    /// Each pair of symbols an arc carries and how many arcs carry it.
    public let symbolPairs: [SymbolPairCount]
    public let acceptor: Bool
    public let inputDeterministic: Bool
    public let outputDeterministic: Bool
    public let cyclic: Bool
    public let cyclicAtInitialState: Bool
    /// The alphabet the transducer declares.
    public let alphabet: [String]
    /// What `verbose` measures besides; `nil` without it.
    public let details: SummaryDetails?
}

// [spec:hfst:req:swift.operations]
/// `hfst check-alpha`'s report on two transducers.
public struct AlphabetComparison: Sendable, Hashable {
    /// The symbols the first transducer's arcs use.
    public let firstUsed: [String]
    public let secondUsed: [String]
    /// The alphabet the first transducer declares.
    public let firstDeclared: [String]
    public let secondDeclared: [String]
    /// Whether either has a symbol, used or declared, the other lacks.
    public let mismatch: Bool
}

// [spec:hfst:req:swift.operations]
/// `hfst dump-alphabets`' report on a transducer.
public struct Alphabets: Sendable, Hashable {
    public let declared: [String]
    /// The symbols its arcs use.
    public let seen: [String]
}

// [spec:hfst:req:swift.operations]
/// One path `hfst fst2strings` prints.
public struct ExtractedPath: Sendable, Hashable {
    /// The input side as the tool prints it: no epsilons, and flag
    /// diacritics only with `printFlags`.
    public let input: String
    public let output: String
    /// The path's symbol pairs; a pair of flag diacritics only with
    /// `printFlags`.
    public let pairs: [SymbolPair]
    public let weight: Float
}

// [spec:hfst:req:swift.operations]
/// What `hfst guess` prints for a word.
public struct GuessReport: Sendable, Hashable {
    public let input: String
    /// Whether `guesses` are paradigms, as with model forms, rather than
    /// analyses.
    public let paradigms: Bool
    /// Each guessed analysis or, with model forms, for each guess the word,
    /// the analysis and the model forms generated for it, separated by tabs.
    public let guesses: [String]
}

// [spec:hfst:req:swift.operations]
// [spec:hfst:req:swift.threads]
// Each query reports what the hfst tool it is named after reports, and
// changes nothing.
extension Transducer {
    /// What `hfst summarize` reports on the transducer, and with `verbose`
    /// what `--verbose` measures besides.
    public func summarize(verbose: Bool = false) throws(HFSTError) -> Summary {
        let s = try calling { try glue.summarize(verbose: verbose) }
        return Summary(
            name: s.name,
            type: TransducerType(glue: s.format),
            states: Int(s.states),
            finalStates: Int(s.finalStates),
            arcs: Int(s.arcs),
            ioEpsilons: Int(s.ioEpsilons),
            inputEpsilons: Int(s.inputEpsilons),
            outputEpsilons: Int(s.outputEpsilons),
            densestArcs: Int(s.densestArcs),
            sparsestArcs: Int(s.sparsestArcs),
            uniqueInputArcs: Int(s.uniqueInputArcs),
            uniqueOutputArcs: Int(s.uniqueOutputArcs),
            mostAmbiguousInput: SymbolCount(symbol: s.mostAmbiguousInput.symbol, count: Int(s.mostAmbiguousInput.count)),
            mostAmbiguousOutput: SymbolCount(symbol: s.mostAmbiguousOutput.symbol, count: Int(s.mostAmbiguousOutput.count)),
            foundAlphabet: s.foundAlphabet,
            symbolPairs: s.symbolPairs.map {
                SymbolPairCount(pair: SymbolPair(input: $0.input, output: $0.output), count: Int($0.count))
            },
            acceptor: s.acceptor,
            inputDeterministic: s.inputDeterministic,
            outputDeterministic: s.outputDeterministic,
            cyclic: s.cyclic,
            cyclicAtInitialState: s.cyclicAtInitialState,
            alphabet: s.alphabet,
            details: s.details.map {
                SummaryDetails(infinitelyAmbiguous: $0.infinitelyAmbiguous, firstInputSymbols: $0.firstInputSymbols)
            }
        )
    }

    /// How the symbols the transducer uses and declares differ from those
    /// of `other`, as `hfst check-alpha` reports.
    public func checkAlpha(_ other: Transducer) throws(HFSTError) -> AlphabetComparison {
        let c = try calling { try glue.checkAlpha(other: other.glue) }
        return AlphabetComparison(
            firstUsed: c.firstUsed,
            secondUsed: c.secondUsed,
            firstDeclared: c.firstDeclared,
            secondDeclared: c.secondDeclared,
            mismatch: c.mismatch
        )
    }

    /// Whether the transducer and `other` are equivalent, as `hfst compare`
    /// decides. Like the tool, it compares two transducers of one algebra
    /// type; flag diacritics are harmonized unless `doNotHarmonize`, and
    /// eliminated first with `eliminateFlags`.
    public func compare(
        _ other: Transducer,
        doNotHarmonize: Bool = false,
        eliminateFlags: Bool = false
    ) throws(HFSTError) -> Bool {
        try calling {
            try glue.compare(other: other.glue, doNotHarmonize: doNotHarmonize, eliminateFlags: eliminateFlags)
        }
    }

    /// What `hfst dump-alphabets` reports on the transducer.
    public func dumpAlphabets() throws(HFSTError) -> Alphabets {
        let a = try calling { try glue.dumpAlphabets() }
        return Alphabets(declared: a.declared, seen: a.seen)
    }

    /// The paths `hfst fst2strings` prints, in its order. Each limit left
    /// `nil` sets none: at most `maxStrings` paths, the `nbest` lightest,
    /// `random` paths chosen at random, cycles followed at most `cycles`
    /// times, and only paths within `beam` of the lightest, whose input is
    /// at most `maxInLength` bytes and output `maxOutLength`. Each path must
    /// start with `inPrefix` and `outPrefix` and must not contain `inExclude`
    /// or `outExclude`, unless they are empty. `obeyFlags` drops the paths
    /// flag diacritics forbid, and `printFlags` keeps the flags in the paths.
    public func extractPaths(
        maxStrings: Int? = nil,
        nbest: Int? = nil,
        random: Int? = nil,
        cycles: Int? = nil,
        beam: Float? = nil,
        obeyFlags: Bool = false,
        printFlags: Bool = false,
        maxInLength: Int? = nil,
        maxOutLength: Int? = nil,
        inPrefix: String = "",
        outPrefix: String = "",
        inExclude: String = "",
        outExclude: String = ""
    ) throws(HFSTError) -> [ExtractedPath] {
        let narrow = { (value: Int?) -> Int32? in value.map { Int32(clamping: $0) } }
        let length = { (value: Int?) -> UInt32? in value.map { UInt32(clamping: $0) } }
        let settings = ExtractSettings(
            maxStrings: narrow(maxStrings),
            nbest: narrow(nbest),
            random: narrow(random),
            cycles: narrow(cycles),
            beam: beam,
            obeyFlags: obeyFlags,
            printFlags: printFlags,
            maxInLength: length(maxInLength),
            maxOutLength: length(maxOutLength),
            inPrefix: inPrefix,
            outPrefix: outPrefix,
            inExclude: inExclude,
            outExclude: outExclude
        )
        return try calling { try glue.extractPaths(settings: settings) }.map {
            ExtractedPath(input: $0.input, output: $0.output, pairs: $0.pairs.map(SymbolPair.init), weight: $0.weight)
        }
    }

    /// What `hfst guess` prints for each of `words` with the transducer, a
    /// guesser `guessify` made. With `modelForms`, the text of a model form
    /// file, it generates model forms for each guess with `generator`, or
    /// with one compiled from the guesser.
    public func guess(
        _ words: [String],
        generator: Transducer? = nil,
        modelForms: String? = nil,
        maxNumberOfGuesses: Int = 5,
        maxNumberOfForms: Int = 2,
        generateThreshold: Float = 50
    ) throws(HFSTError) -> [GuessReport] {
        try calling {
            try glue.guess(
                words: words,
                generator: generator?.glue,
                modelForms: modelForms,
                maxNumberOfGuesses: Int64(maxNumberOfGuesses),
                maxNumberOfForms: Int64(maxNumberOfForms),
                generateThreshold: generateThreshold
            )
        }.map { GuessReport(input: $0.input, paradigms: $0.paradigms, guesses: $0.guesses) }
    }

    /// The tokeniser `hfst tokenize` makes of a transducer that is not a
    /// pmatch container: each word of the transducer's input side a token,
    /// analysed as its outputs. Multicharacter symbols are matched longest
    /// first when the transducer has any, or when a call's options set
    /// `tokenizeMultichar`.
    public func naiveTokeniser() throws(HFSTError) -> HFST.Tokeniser {
        HFST.Tokeniser(glue: try calling { try glue.naiveTokeniser() })
    }
}
