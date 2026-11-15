import Foundation
package import HFSTGlue

// [spec:hfst:req:swift.tokenise]
/// One reading of a token.
public struct Reading: Sendable, Hashable {
    /// The tokeniser's output for the token, such as `cat+N`.
    public let analysis: String
    /// The reading's weight. Lower is better.
    public let weight: Float
}

// [spec:hfst:req:swift.tokenise]
/// A token the tokeniser found or, under `printAll`, a stretch of text
/// between tokens where no pattern matched.
public struct Token: Sendable, Hashable {
    /// The token's text, as the tokeniser read it.
    public let surface: String
    /// Where the token is in the text it was found in: `text[range]` is the
    /// surface, unless the text holds something the tokeniser reads without
    /// printing, such as the spelling of a flag diacritic.
    public let range: Range<String.Index>
    /// Where the token is in the text's UTF-8 bytes.
    public let utf8Range: Range<Int>
    /// Whether any reading is an analysis. A reading the tokeniser marks as
    /// unknown, by an empty output or one containing ` ??`, is not one.
    public let analysed: Bool
    /// The readings `hfst tokenise --cg --print-weights` prints for the
    /// token, in that order. A stretch where no pattern matched has none.
    public let readings: [Reading]
}

// [spec:hfst:req:swift.tokenise]
/// The formats `Tokeniser.format(_:as:options:)` prints, under the names of
/// the `hfst tokenise` options that choose them, which are the raw values.
public enum TokeniseFormat: String, Sendable, Hashable, CaseIterable {
    /// One token a line, `--segment`, the tool's default.
    case segment
    /// Tokens separated by spaces.
    case spaceSeparated = "space-separated"
    /// Each reading with its surface and weight.
    case xerox
    /// Constraint Grammar cohorts.
    case cg
    /// FinnPos input.
    case finnpos
    /// The Constraint Grammar form Giella grammars read.
    case giellaCG = "giella-cg"
    /// CoNLL-U.
    case conllu
    /// The cohorts VISL CG-3 reads.
    case visl
}

// [spec:hfst:req:swift.tokenise]
/// What changes the tokens `Tokeniser.tokenise(_:options:)` returns. Each
/// property has the value `hfst tokenise` gives the option unless set.
public struct TokeniseOptions: Sendable, Hashable {
    /// Drops each reading that repeats another, in analysis, pattern and
    /// weight, as `hfst tokenise` does unless given `--duplicates`.
    public var dedupe: Bool
    /// Keeps the readings of only this many of a token's best weights, as
    /// `--weight-classes`. Readings of equal weight make one class. An
    /// unknown reading counts toward none, and is kept unless it comes after
    /// the first reading the limit drops. At least 1; `nil` keeps every
    /// class.
    public var maxWeightClasses: Int?
    /// Keeps only the readings whose weight is within this much of the
    /// token's best, as `--beam`. At least 0; `nil` keeps every reading.
    public var beam: Float?
    /// Abandons any match heavier than this, so it gives neither a token nor
    /// a reading. `hfst tokenise` has no such setting.
    public var weightCutoff: Float?
    /// Stops looking for further matches after this many seconds and keeps
    /// what was found, as `--time-cutoff`. `nil` or 0 sets no limit.
    public var timeLimit: TimeInterval?
    /// Reads multicharacter symbols in the text, as `--tokenize-multichar`.
    /// Otherwise the text is read one grapheme at a time, unless the
    /// tokeniser's patterns read multicharacter symbols.
    public var tokenizeMultichar: Bool
    /// Includes each stretch of text where no pattern matched, as
    /// `--print-all`.
    public var printAll: Bool

    public init(
        dedupe: Bool = true,
        maxWeightClasses: Int? = nil,
        beam: Float? = nil,
        weightCutoff: Float? = nil,
        timeLimit: TimeInterval? = nil,
        tokenizeMultichar: Bool = false,
        printAll: Bool = false
    ) {
        self.dedupe = dedupe
        self.maxWeightClasses = maxWeightClasses
        self.beam = beam
        self.weightCutoff = weightCutoff
        self.timeLimit = timeLimit
        self.tokenizeMultichar = tokenizeMultichar
        self.printAll = printAll
    }
}

// [spec:hfst:req:swift.tokenise]
/// What changes the text `Tokeniser.format(_:as:options:)` returns: every
/// option `TokeniseOptions` has but `weightCutoff`, which `hfst tokenise` has
/// no setting for, and the options below. An option left `nil` has the value
/// `hfst tokenise` gives it with the format's flag; an option set overrides
/// what that flag implies, as a flag after it would. `.giellaCG` implies
/// `printWeights`, `printAll`, `hackUncompose` and a `maxWeightClasses` of 2;
/// `.visl` implies `printAll` and no weights.
///
/// `hfst tokenise` takes each paragraph of its input (each line, under
/// `newline`) as one input, while `.giellaCG`, `.visl` and `superblanks` read
/// the text line by line, and `format` does the same.
public struct FormatOptions: Sendable, Hashable {
    public var dedupe: Bool?
    public var maxWeightClasses: Int?
    public var beam: Float?
    public var timeLimit: TimeInterval?
    public var tokenizeMultichar: Bool?
    public var printAll: Bool?
    /// Whether the output carries weights: `true` stands for
    /// `--print-weights` and `false` for `--no-weights`.
    public var printWeights: Bool?
    /// Prints a reading's intermediate form in `.giellaCG` and `.visl`
    /// output, when the tokeniser was compiled to carry one.
    public var hackUncompose: Bool?
    /// Takes each line, not each paragraph, as one input, as `--newline`.
    public var newline: Bool?
    /// Keeps the newline that ends each input, as `--keep-newline`, which
    /// also takes each line as one input.
    public var keepNewline: Bool?
    /// Passes text inside unescaped square brackets through unmatched, as
    /// `--superblanks`.
    public var superblanks: Bool?

    public init(
        dedupe: Bool? = nil,
        maxWeightClasses: Int? = nil,
        beam: Float? = nil,
        timeLimit: TimeInterval? = nil,
        tokenizeMultichar: Bool? = nil,
        printAll: Bool? = nil,
        printWeights: Bool? = nil,
        hackUncompose: Bool? = nil,
        newline: Bool? = nil,
        keepNewline: Bool? = nil,
        superblanks: Bool? = nil
    ) {
        self.dedupe = dedupe
        self.maxWeightClasses = maxWeightClasses
        self.beam = beam
        self.timeLimit = timeLimit
        self.tokenizeMultichar = tokenizeMultichar
        self.printAll = printAll
        self.printWeights = printWeights
        self.hackUncompose = hackUncompose
        self.newline = newline
        self.keepNewline = keepNewline
        self.superblanks = superblanks
    }
}

// [spec:hfst:req:swift.tokenise]
// [spec:hfst:req:swift.threads]
/// A tokeniser read from a pmatch container, as `hfst tokenise` reads one.
/// It holds the container's tables alone and never changes, so tasks may
/// tokenise with it at the same time: each call is a run of its own.
public final class Tokeniser: Sendable {
    package let glue: HFSTGlue.Tokeniser

    // [spec:hfst:req:swift.load]
    /// The tokeniser in a pmatch container's bytes, as `hfst pmatch2fst`
    /// writes them. Bytes that are not a pmatch container throw
    /// `invalidFormat`, among them a transducer that is not one: `hfst
    /// tokenise` wraps such a transducer in a tokeniser of its own, which
    /// this reader does not make.
    public init(data: Data) throws(HFSTError) {
        glue = try calling { try readTokeniser(bytes: data) }
    }

    // [spec:hfst:req:swift.load]
    /// The tokeniser in the pmatch container at `url`, as `init(data:)`
    /// reads its bytes. A file that cannot be read throws what
    /// `Data(contentsOf:)` throws.
    public convenience init(contentsOf url: URL) throws {
        try self.init(data: Data(contentsOf: url, options: .mappedIfSafe))
    }

    package init(glue: HFSTGlue.Tokeniser) {
        self.glue = glue
    }

    // [spec:hfst:req:swift.tokenise]
    /// The tokens the tokeniser finds in `text`, in text order, matching the
    /// whole text as one input. `hfst tokenise` takes each paragraph as one
    /// input, so for text with no blank line these are the cohorts
    /// `hfst tokenise --cg --print-weights` prints, given the same options.
    /// An option value the command line would turn down throws
    /// `invalidArgument`.
    public func tokenise(
        _ text: String,
        options: TokeniseOptions = TokeniseOptions()
    ) throws(HFSTError) -> [Token] {
        let settings = TokeniseSettings(
            selection: SelectionSettings(
                dedupe: options.dedupe,
                maxWeightClasses: options.maxWeightClasses.map(Int64.init),
                beam: options.beam,
                timeLimit: options.timeLimit,
                tokenizeMultichar: options.tokenizeMultichar,
                printAll: options.printAll
            ),
            weightCutoff: options.weightCutoff
        )
        let found = try calling { try glue.tokenise(text: text, settings: settings) }
        return tokens(found, in: text)
    }

    // [spec:hfst:req:swift.tokenise]
    /// Exactly what `hfst tokenise` prints in `format` when `text` is its
    /// standard input, given the flags `options` stand for.
    public func format(
        _ text: String,
        as format: TokeniseFormat = .segment,
        options: FormatOptions = FormatOptions()
    ) throws(HFSTError) -> String {
        let settings = FormatSettings(
            selection: SelectionSettings(
                dedupe: options.dedupe,
                maxWeightClasses: options.maxWeightClasses.map(Int64.init),
                beam: options.beam,
                timeLimit: options.timeLimit,
                tokenizeMultichar: options.tokenizeMultichar,
                printAll: options.printAll
            ),
            printWeights: options.printWeights,
            hackUncompose: options.hackUncompose,
            newline: options.newline,
            keepNewline: options.keepNewline,
            superblanks: options.superblanks
        )
        return try calling {
            try glue.format(text: text, format: format.rawValue, settings: settings)
        }
    }
}

// [spec:hfst:req:swift.tokenise]
/// `found`, whose offsets are UTF-8 bytes of `text` and only go forward, as
/// tokens whose ranges index `text`. The text is walked once.
func tokens(_ found: [HFSTGlue.Token], in text: String) -> [Token] {
    let utf8 = text.utf8
    var index = utf8.startIndex
    var offset = 0
    let at = { (byte: Int) -> String.Index in
        let byte = min(max(byte, offset), utf8.count)
        index = utf8.index(index, offsetBy: byte - offset)
        offset = byte
        return index
    }
    return found.map { token in
        let start = at(Int(token.start))
        let end = at(Int(token.end))
        return Token(
            surface: token.surface,
            range: start..<end,
            utf8Range: Int(token.start)..<Int(token.end),
            analysed: token.analysed,
            readings: token.readings.map { Reading(analysis: $0.analysis, weight: $0.weight) }
        )
    }
}
