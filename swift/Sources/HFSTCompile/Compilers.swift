import Foundation
internal import HFSTCompileGlue

// [spec:hfst:req:swift.compile-files]
/// Where a compile reads the files its source names, and where an xfst
/// script writes: the filesystem, files held in memory, or a closure that
/// supplies each by name. Without the filesystem, what a script writes is
/// kept in memory, where its later commands read it, and comes back in its
/// `XfstResult`; a name the files do not supply fails the compile and names
/// the file.
public struct CompileFiles: Sendable {
    enum Kind: Sendable {
        case fileSystem
        case memory([String: Data])
        case resolver(@Sendable (String) -> Data?)
    }

    let kind: Kind

    /// The filesystem: a relative name is resolved against the working
    /// directory.
    public static let fileSystem = CompileFiles(kind: .fileSystem)

    /// The files of `files`, each its bytes under its name.
    public static func memory(_ files: [String: Data]) -> CompileFiles {
        CompileFiles(kind: .memory(files))
    }

    /// The files of `files`, by name, each the UTF-8 of its text.
    public static func memory(_ files: [String: String]) -> CompileFiles {
        CompileFiles(kind: .memory(files.mapValues { Data($0.utf8) }))
    }

    /// The files `resolve` supplies, each asked for once, by name, or `nil`
    /// for a file it does not have. The library calls it from the thread
    /// the compile runs on, so it must be `@Sendable`.
    public static func resolver(_ resolve: @escaping @Sendable (String) -> Data?) -> CompileFiles {
        CompileFiles(kind: .resolver(resolve))
    }

    /// The glue's files for one compile, made afresh for each, so what one
    /// compile writes in memory is not another's.
    var glue: Files {
        switch kind {
        case .fileSystem: Files.fileSystem()
        case let .memory(files): Files.memory(files: files)
        case let .resolver(resolve): Files.resolver(source: Source(resolve: resolve))
        }
    }
}

/// A caller's closure as the glue asks it for files.
private final class Source: FileSource {
    let resolve: @Sendable (String) -> Data?

    init(resolve: @escaping @Sendable (String) -> Data?) {
        self.resolve = resolve
    }

    func read(name: String) -> Data? {
        resolve(name)
    }
}

/// What the standard input is called when a tool reads its source there,
/// and so what a source is called unless named.
let standardInput = "<stdin>"

// [spec:hfst:req:swift.compile]
/// What `regex` makes of its source, as the `hfst regexp2fst` option of the
/// same name makes it.
public struct RegexOptions: Sendable, Hashable {
    public var format: TransducerType
    /// Each expression ends with `;`, rather than a line.
    public var semicolon: Bool
    /// One transducer, the union of the expressions.
    public var disjunct: Bool
    /// Do not expand `?` by the other operand's symbols.
    public var doNotHarmonize: Bool
    public var harmonizeFlags: Bool
    /// Determinize each result rather than minimizing it.
    public var doNotMinimize: Bool
    /// Minimize with the weights as part of the labels.
    public var encodeWeights: Bool
    /// Flag diacritics are ordinary symbols in composition.
    public var xeroxComposition: Bool
    /// A one-sided flag diacritic is an epsilon in composition, as `--xfst
    /// flag-is-epsilon`.
    public var flagIsEpsilon: Bool
    /// Compile each expression, one parallel optional replace rule, as one
    /// rewrite pass.
    public var replacePass: Bool
    /// Say what the compile does, at the info level of the log.
    public var verbose: Bool

    public init(
        format: TransducerType = .openFstTropical,
        semicolon: Bool = false,
        disjunct: Bool = false,
        doNotHarmonize: Bool = false,
        harmonizeFlags: Bool = false,
        doNotMinimize: Bool = false,
        encodeWeights: Bool = false,
        xeroxComposition: Bool = false,
        flagIsEpsilon: Bool = false,
        replacePass: Bool = false,
        verbose: Bool = false
    ) {
        self.format = format
        self.semicolon = semicolon
        self.disjunct = disjunct
        self.doNotHarmonize = doNotHarmonize
        self.harmonizeFlags = harmonizeFlags
        self.doNotMinimize = doNotMinimize
        self.encodeWeights = encodeWeights
        self.xeroxComposition = xeroxComposition
        self.flagIsEpsilon = flagIsEpsilon
        self.replacePass = replacePass
        self.verbose = verbose
    }
}

// [spec:hfst:req:swift.compile]
/// What `compileLexc` makes of its sources, as the `hfst lexc` option of the
/// same name makes it.
public struct LexcOptions: Sendable, Hashable {
    public var format: TransducerType
    /// Align the characters of a string pair one to one.
    public var alignStrings: Bool
    /// Join the lexicons with flag diacritics rather than composing them;
    /// `minimizeFlags` then uses as few flags as it can, and `renameFlags`
    /// renames them.
    public var withFlags: Bool
    public var minimizeFlags: Bool
    public var renameFlags: Bool
    /// Compose as Xerox tools do, as the tool does unless told
    /// `--xerox-composition=no`.
    public var xeroxComposition: Bool
    public var flagIsEpsilon: Bool
    public var encodeWeights: Bool
    /// Do not read the source's text as Unicode characters.
    public var splitCharacters: Bool
    /// A warning fails the compile, as `-Werror`.
    public var werror: Bool
    public var warnOneSidedFlags: Bool
    public var warnUnusedLexicons: Bool
    public var warnRepeatedLexicons: Bool
    public var warnMissingLexicons: Bool
    public var warnMissingAlphabets: Bool
    public var warnUnnecessaryEscapes: Bool
    /// No warnings or notes.
    public var quiet: Bool
    /// Progress as well.
    public var verbose: Bool

    public init(
        format: TransducerType = .openFstTropical,
        alignStrings: Bool = false,
        withFlags: Bool = false,
        minimizeFlags: Bool = false,
        renameFlags: Bool = false,
        xeroxComposition: Bool = true,
        flagIsEpsilon: Bool = false,
        encodeWeights: Bool = false,
        splitCharacters: Bool = false,
        werror: Bool = false,
        warnOneSidedFlags: Bool = false,
        warnUnusedLexicons: Bool = false,
        warnRepeatedLexicons: Bool = false,
        warnMissingLexicons: Bool = false,
        warnMissingAlphabets: Bool = false,
        warnUnnecessaryEscapes: Bool = false,
        quiet: Bool = false,
        verbose: Bool = false
    ) {
        self.format = format
        self.alignStrings = alignStrings
        self.withFlags = withFlags
        self.minimizeFlags = minimizeFlags
        self.renameFlags = renameFlags
        self.xeroxComposition = xeroxComposition
        self.flagIsEpsilon = flagIsEpsilon
        self.encodeWeights = encodeWeights
        self.splitCharacters = splitCharacters
        self.werror = werror
        self.warnOneSidedFlags = warnOneSidedFlags
        self.warnUnusedLexicons = warnUnusedLexicons
        self.warnRepeatedLexicons = warnRepeatedLexicons
        self.warnMissingLexicons = warnMissingLexicons
        self.warnMissingAlphabets = warnMissingAlphabets
        self.warnUnnecessaryEscapes = warnUnnecessaryEscapes
        self.quiet = quiet
        self.verbose = verbose
    }
}

// [spec:hfst:req:swift.compile]
/// What `compileTwolc` makes of its grammar, as the `hfst twolc` option of
/// the same name makes it.
public struct TwolcOptions: Sendable, Hashable {
    /// Tropical OpenFst or foma.
    public var format: TransducerType
    /// Resolve left-arrow conflicts; `dontResolveRight` leaves right-arrow
    /// conflicts alone while doing so.
    public var resolve: Bool
    public var dontResolveRight: Bool
    public var quiet: Bool
    public var verbose: Bool

    public init(
        format: TransducerType = .openFstTropical,
        resolve: Bool = false,
        dontResolveRight: Bool = false,
        quiet: Bool = false,
        verbose: Bool = false
    ) {
        self.format = format
        self.resolve = resolve
        self.dontResolveRight = dontResolveRight
        self.quiet = quiet
        self.verbose = verbose
    }
}

// [spec:hfst:req:swift.compile]
/// How `compileXfst` runs its script, as the `hfst xfst` option of the same
/// name runs it.
public struct XfstOptions: Sendable, Hashable {
    /// The type the script works in, tropical OpenFst or foma.
    public var format: TransducerType
    /// Print weights where words are printed.
    public var printWeight: Bool
    /// Read and write only in the working directory, and run no shell
    /// command.
    public var restrictedMode: Bool
    /// Print no command output.
    public var silent: Bool

    public init(
        format: TransducerType = .openFstTropical,
        printWeight: Bool = false,
        restrictedMode: Bool = false,
        silent: Bool = false
    ) {
        self.format = format
        self.printWeight = printWeight
        self.restrictedMode = restrictedMode
        self.silent = silent
    }
}

// [spec:hfst:req:swift.compile]
/// What `compilePmatch` makes of its script, as the `hfst pmatch2fst`
/// option of the same name makes it.
public struct PmatchOptions: Sendable, Hashable {
    /// Compile every RTN into the network that uses it.
    public var flatten: Bool
    /// Weight each word `Like()` finds by its cosine distance from the word
    /// it is like.
    public var cosineDistances: Bool
    public var verbose: Bool

    public init(flatten: Bool = false, cosineDistances: Bool = false, verbose: Bool = false) {
        self.flatten = flatten
        self.cosineDistances = cosineDistances
        self.verbose = verbose
    }
}

// [spec:hfst:req:swift.compile]
/// What `compileStrings` makes of its lines, as the `hfst strings2fst`
/// option of the same name makes it.
public struct StringsOptions: Sendable, Hashable {
    public var format: TransducerType
    /// One transducer, the union of the lines.
    public var disjunctStrings: Bool
    /// Divide each weight by the sum of all of them, with `disjunctStrings`.
    public var norm: Bool
    /// Take the negative natural logarithm of each weight.
    public var log: Bool
    /// Take the negative base 10 logarithm of each weight.
    public var log10: Bool
    /// The lines are pair strings, `a:b c`, rather than string pairs,
    /// `ac:bc`.
    public var pairstrings: Bool
    /// Symbols are separated by spaces.
    public var hasSpaces: Bool
    /// What the lines spell the epsilon as.
    public var epsilon: String
    /// The strings read as one symbol each, the lines of the tool's
    /// `--multichar-symbols` file.
    public var multicharSymbols: [String]
    public var warnNegativeWeights: Bool
    public var werror: Bool

    public init(
        format: TransducerType = .openFstTropical,
        disjunctStrings: Bool = false,
        norm: Bool = false,
        log: Bool = false,
        log10: Bool = false,
        pairstrings: Bool = false,
        hasSpaces: Bool = false,
        epsilon: String = "@0@",
        multicharSymbols: [String] = [],
        warnNegativeWeights: Bool = true,
        werror: Bool = false
    ) {
        self.format = format
        self.disjunctStrings = disjunctStrings
        self.norm = norm
        self.log = log
        self.log10 = log10
        self.pairstrings = pairstrings
        self.hasSpaces = hasSpaces
        self.epsilon = epsilon
        self.multicharSymbols = multicharSymbols
        self.warnNegativeWeights = warnNegativeWeights
        self.werror = werror
    }
}

// [spec:hfst:req:swift.compile]
/// What `parseAtt` makes of its text, as the `hfst txt2fst` option of the
/// same name makes it.
public struct AttOptions: Sendable, Hashable {
    public var format: TransducerType
    /// What the text spells the epsilon as.
    public var epsilon: String
    /// One transducer, the union of the text's.
    public var disjunct: Bool
    public var checkNegativeEpsilonCycles: Bool
    public var warnNegativeWeights: Bool

    public init(
        format: TransducerType = .openFstTropical,
        epsilon: String = "@0@",
        disjunct: Bool = false,
        checkNegativeEpsilonCycles: Bool = false,
        warnNegativeWeights: Bool = true
    ) {
        self.format = format
        self.epsilon = epsilon
        self.disjunct = disjunct
        self.checkNegativeEpsilonCycles = checkNegativeEpsilonCycles
        self.warnNegativeWeights = warnNegativeWeights
    }
}

// [spec:hfst:req:swift.compile]
/// What `parseProlog` makes of its text, as `hfst txt2fst --prolog` makes
/// it with the option of the same name.
public struct PrologOptions: Sendable, Hashable {
    public var format: TransducerType
    public var checkNegativeEpsilonCycles: Bool

    public init(format: TransducerType = .openFstTropical, checkNegativeEpsilonCycles: Bool = false) {
        self.format = format
        self.checkNegativeEpsilonCycles = checkNegativeEpsilonCycles
    }
}

// [spec:hfst:req:swift.compile]
/// What an xfst script left behind.
public struct XfstResult: Sendable {
    /// What it printed: the tool's standard output.
    public let output: String
    /// Its stack, the bottom first.
    public let stack: [Transducer]
    /// The networks it defined, by name.
    public let definitions: [String: Transducer]
    /// Each file it wrote, by name, holding what the script left in it.
    public let written: [String: Data]
    /// Each warning, and each error of a command that failed without
    /// stopping the script.
    public let warnings: [HFST.Diagnostic]
}

// [spec:hfst:req:swift.compile]
// [spec:hfst:req:swift.compile-errors]
/// The transducer `hfst regexp2fst` writes of the regular expression
/// `source`. With `RegexOptions(disjunct: true)` a source of several
/// expressions, one a line or with `semicolon` each ending in `;`, compiles
/// to their union; without it, a source of more than one throws
/// `transducerCount`. `name` is the file name the tool would read the source
/// from, which diagnostics give it. A source that does not compile throws
/// `compileFailed`, with every diagnostic it made.
public func regex(
    _ source: String,
    options: RegexOptions = RegexOptions(),
    name: String = "<stdin>",
    files: CompileFiles = .fileSystem
) throws(HFSTError) -> Compiled<Transducer> {
    let settings = RegexSettings(
        format: options.format.rawValue,
        semicolon: options.semicolon,
        disjunct: options.disjunct,
        doNotHarmonize: options.doNotHarmonize,
        harmonizeFlags: options.harmonizeFlags,
        doNotMinimize: options.doNotMinimize,
        encodeWeights: options.encodeWeights,
        xeroxComposition: options.xeroxComposition,
        flagIsEpsilon: options.flagIsEpsilon,
        replacePass: options.replacePass,
        verbose: options.verbose
    )
    let compiled = try calling(sources: [source]) {
        try HFSTCompileGlue.regex(source: source, name: name, settings: settings, files: files.glue)
    }
    return Compiled(value: Transducer(glue: compiled.transducer), warnings: diagnostics(compiled.warnings, in: [source]))
}

// [spec:hfst:req:swift.compile]
/// The transducer `hfst lexc` writes of the lexc `sources`, compiled
/// together and named after the first. `names` are the file names the tool
/// would read them from, which diagnostics and the result's name give them;
/// each is `<stdin>` unless given.
public func compileLexc(
    _ sources: [String],
    options: LexcOptions = LexcOptions(),
    names: [String]? = nil,
    files: CompileFiles = .fileSystem
) throws(HFSTError) -> Compiled<Transducer> {
    let settings = LexcSettings(
        format: options.format.rawValue,
        alignStrings: options.alignStrings,
        withFlags: options.withFlags,
        minimizeFlags: options.minimizeFlags,
        renameFlags: options.renameFlags,
        xeroxComposition: options.xeroxComposition,
        flagIsEpsilon: options.flagIsEpsilon,
        encodeWeights: options.encodeWeights,
        splitCharacters: options.splitCharacters,
        werror: options.werror,
        warnOneSidedFlags: options.warnOneSidedFlags,
        warnUnusedLexicons: options.warnUnusedLexicons,
        warnRepeatedLexicons: options.warnRepeatedLexicons,
        warnMissingLexicons: options.warnMissingLexicons,
        warnMissingAlphabets: options.warnMissingAlphabets,
        warnUnnecessaryEscapes: options.warnUnnecessaryEscapes,
        quiet: options.quiet,
        verbose: options.verbose
    )
    let names = names ?? Array(repeating: standardInput, count: sources.count)
    let compiled = try calling(sources: sources) {
        try HFSTCompileGlue.compileLexc(sources: sources, names: names, settings: settings, files: files.glue)
    }
    return Compiled(value: Transducer(glue: compiled.transducer), warnings: diagnostics(compiled.warnings, in: sources))
}

// [spec:hfst:req:swift.compile]
/// The rule transducers `hfst twolc` writes of the twolc grammar `source`,
/// in tropical OpenFst or foma. `name` is the file name the tool would read
/// it from, `<twolc>` for its standard input. A twolc grammar names no file.
public func compileTwolc(
    _ source: String,
    options: TwolcOptions = TwolcOptions(),
    name: String = "<twolc>"
) throws(HFSTError) -> Compiled<[Transducer]> {
    let settings = TwolcSettings(
        format: options.format.rawValue,
        resolve: options.resolve,
        dontResolveRight: options.dontResolveRight,
        quiet: options.quiet,
        verbose: options.verbose
    )
    let compiled = try calling(sources: [source]) {
        try HFSTCompileGlue.compileTwolc(source: source, name: name, settings: settings)
    }
    return Compiled(
        value: compiled.transducers.map(Transducer.init(glue:)),
        warnings: diagnostics(compiled.warnings, in: [source])
    )
}

// [spec:hfst:req:swift.compile]
// [spec:hfst:req:swift.compile-files]
/// Runs the xfst script `source` as `hfst xfst -F` runs it, in tropical
/// OpenFst or foma, with `input` as the standard input its commands read:
/// what it printed, its stack, its defined networks and the files it wrote.
/// A command that fails is a warning, and the script throws
/// `compileFailed` only when it stops on one.
public func compileXfst(
    _ source: String,
    options: XfstOptions = XfstOptions(),
    input: String = "",
    name: String = "<stdin>",
    files: CompileFiles = .fileSystem
) throws(HFSTError) -> XfstResult {
    let settings = XfstSettings(
        format: options.format.rawValue,
        printWeight: options.printWeight,
        restrictedMode: options.restrictedMode,
        silent: options.silent
    )
    let ran = try calling(sources: [source]) {
        try HFSTCompileGlue.compileXfst(
            source: source,
            input: input,
            name: name,
            settings: settings,
            files: files.glue
        )
    }
    return XfstResult(
        output: ran.output,
        stack: ran.stack.map(Transducer.init(glue:)),
        definitions: ran.definitions.mapValues(Transducer.init(glue:)),
        written: Dictionary(ran.written.map { ($0.name, $0.contents) }, uniquingKeysWith: { _, last in last }),
        warnings: diagnostics(ran.warnings, in: [source])
    )
}

// [spec:hfst:req:swift.compile]
/// The archive `hfst pmatch2fst` writes of the pmatch script `source`, which
/// `Tokeniser(data:)` reads. `name` is the file name the tool would read it
/// from: a relative `@bin"FILE"` or `@txt"FILE"` is found beside it, as the
/// tool finds one beside its input.
public func compilePmatch(
    _ source: String,
    options: PmatchOptions = PmatchOptions(),
    name: String = "<stdin>",
    files: CompileFiles = .fileSystem
) throws(HFSTError) -> Compiled<Data> {
    let settings = PmatchSettings(
        flatten: options.flatten,
        cosineDistances: options.cosineDistances,
        verbose: options.verbose
    )
    let compiled = try calling(sources: [source]) {
        try HFSTCompileGlue.compilePmatch(source: source, name: name, settings: settings, files: files.glue)
    }
    return Compiled(value: compiled.archive, warnings: diagnostics(compiled.warnings, in: [source]))
}

// [spec:hfst:req:swift.compile]
/// The transducer `hfst strings2fst` writes of the strings `source`, one a
/// line, each with an optional tab and weight. With
/// `StringsOptions(disjunctStrings: true)` the lines compile to their union;
/// without it, a source of more than one line throws `transducerCount`.
public func compileStrings(
    _ source: String,
    options: StringsOptions = StringsOptions(),
    name: String = "<stdin>"
) throws(HFSTError) -> Compiled<Transducer> {
    let settings = StringsSettings(
        format: options.format.rawValue,
        disjunctStrings: options.disjunctStrings,
        norm: options.norm,
        log: options.log,
        log10: options.log10,
        pairstrings: options.pairstrings,
        hasSpaces: options.hasSpaces,
        epsilon: options.epsilon,
        multicharSymbols: options.multicharSymbols,
        warnNegativeWeights: options.warnNegativeWeights,
        werror: options.werror
    )
    let compiled = try calling(sources: [source]) {
        try HFSTCompileGlue.compileStrings(source: source, name: name, settings: settings)
    }
    return Compiled(value: Transducer(glue: compiled.transducer), warnings: diagnostics(compiled.warnings, in: [source]))
}

// [spec:hfst:req:swift.compile]
/// The transducer `hfst txt2fst` writes of the AT&T text `source`, named
/// after `name`. With `AttOptions(disjunct: true)` a text of several
/// transducers, separated by `--` lines, reads as their union; without it, a
/// text of more than one throws `transducerCount`.
public func parseAtt(
    _ source: String,
    options: AttOptions = AttOptions(),
    name: String = "<stdin>"
) throws(HFSTError) -> Compiled<Transducer> {
    let settings = TextSettings(
        format: options.format.rawValue,
        prolog: false,
        epsilon: options.epsilon,
        disjunct: options.disjunct,
        checkNegativeEpsilonCycles: options.checkNegativeEpsilonCycles,
        warnNegativeWeights: options.warnNegativeWeights
    )
    return try parse(source, name: name, settings: settings)
}

// [spec:hfst:req:swift.compile]
/// The transducer `hfst txt2fst --prolog` writes of the Prolog text
/// `source`, named after `name`. A text of more than one network throws
/// `transducerCount`.
public func parseProlog(
    _ source: String,
    options: PrologOptions = PrologOptions(),
    name: String = "<stdin>"
) throws(HFSTError) -> Compiled<Transducer> {
    let settings = TextSettings(
        format: options.format.rawValue,
        prolog: true,
        epsilon: "@0@",
        disjunct: false,
        checkNegativeEpsilonCycles: options.checkNegativeEpsilonCycles,
        warnNegativeWeights: true
    )
    return try parse(source, name: name, settings: settings)
}

private func parse(_ source: String, name: String, settings: TextSettings) throws(HFSTError) -> Compiled<Transducer> {
    let compiled = try calling(sources: [source]) {
        try parseText(source: source, name: name, settings: settings)
    }
    return Compiled(value: Transducer(glue: compiled.transducer), warnings: diagnostics(compiled.warnings, in: [source]))
}
