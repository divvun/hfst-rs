import Foundation
internal import HFSTGlue

// [spec:hfst:req:swift.dhfst]
/// A DHFST file, divvunspell's speller format, of either type, read and
/// validated as divvunspell reads it.
public enum Dhfst: Sendable {
    /// Type 2, a speller's lexicon.
    case acceptor(DhfstAcceptor)
    /// Type 1, the model of a speller's typing errors.
    case errorModel(DhfstErrorModel)

    // [spec:hfst:req:swift.load]
    /// The DHFST file in `data`. A file that fails the validation divvunspell
    /// runs on load, or is truncated, throws `invalidFormat` with the
    /// reason. Bytes that are not DHFST throw `invalidFormat`, or
    /// `unsupportedFormat` naming a format the package does not read.
    public init(data: Data) throws(HFSTError) {
        switch try calling({ try readDhfst(bytes: data) }) {
        case let .acceptor(acceptor):
            self = .acceptor(DhfstAcceptor(glue: acceptor))
        case let .errorModel(model):
            self = .errorModel(DhfstErrorModel(model))
        }
    }

    // [spec:hfst:req:swift.load]
    /// The DHFST file at `url`, as `init(data:)` reads its bytes. A file that
    /// cannot be read throws what `Data(contentsOf:)` throws.
    public init(contentsOf url: URL) throws {
        // Two statements, not one `try`: an initialiser that mixes untyped and
        // typed throws in one expression crashes Swift 6.4's optimiser when it
        // inlines it into an app's release build.
        let data = try Data(contentsOf: url, options: .mappedIfSafe)
        try self.init(data: data)
    }
}

// [spec:hfst:req:swift.dhfst]
/// How a DHFST acceptor stores its arcs, as `dhfst-tools` reports it.
public struct AcceptorInfo: Sendable, Hashable {
    /// State numbers: every state is numbered below this.
    public let ids: Int
    /// Slots in the table, at least `ids` plus `labelMax`.
    public let slots: Int
    /// Slots that hold arcs, free or regular.
    public let usedSlots: Int
    /// Bytes per slot check: 1 when every regular label is below 256, else 2.
    public let checkBytes: Int
    /// The greatest regular label.
    public let labelMax: Int
    /// Bytes per record, each a target and the index of a weight.
    public let recordBytes: Int
    /// Records in the list of arcs that do not fit their slot.
    public let listRecords: Int
    /// Arcs, free and regular.
    public let arcs: Int
    /// Free arcs: epsilon and flag diacritic arcs.
    public let freeArcs: Int
    /// Distinct symbol and weight pairs of the free arcs.
    public let freePairs: Int
    /// Distinct regular arc weights.
    public let weights: Int
    public let finals: Int
    /// Whether the file stores each state's distance to a final state.
    public let distances: Bool
}

// [spec:hfst:req:swift.dhfst]
// [spec:hfst:req:swift.threads]
/// A DHFST acceptor: a speller's lexicon, read and validated as divvunspell
/// reads it. It never changes, so tasks may look words up in it at the same
/// time.
public final class DhfstAcceptor: Sendable {
    let glue: HFSTGlue.DhfstAcceptor

    init(glue: HFSTGlue.DhfstAcceptor) {
        self.glue = glue
    }

    /// The format version the file declares.
    public var version: Int { Int(glue.version()) }

    /// The symbol names, `@_EPSILON_SYMBOL_@` first.
    public var symbols: [String] { glue.symbols() }

    /// The JSON the writer left in the file's `meta` section, or `nil` when
    /// it has none.
    public var meta: String? { glue.meta() }

    /// How the file stores its arcs.
    public var info: AcceptorInfo {
        let info = glue.info()
        return AcceptorInfo(
            ids: Int(info.ids),
            slots: Int(info.slots),
            usedSlots: Int(info.usedSlots),
            checkBytes: Int(info.checkBytes),
            labelMax: Int(info.labelMax),
            recordBytes: Int(info.recordBytes),
            listRecords: Int(info.listRecords),
            arcs: Int(info.arcs),
            freeArcs: Int(info.freeArcs),
            freePairs: Int(info.freePairs),
            weights: Int(info.weights),
            finals: Int(info.finals),
            distances: info.distances
        )
    }

    // [spec:hfst:req:swift.dhfst]
    /// The least weight of a path that accepts `word`, final weight
    /// included, or `nil` when no path does: what divvunspell's speller finds
    /// checking the word as typed against this lexicon alone. The word is
    /// read one extended grapheme cluster at a time, each the symbol of the
    /// same text or else the lexicon's unknown symbol. Epsilon and flag
    /// diacritic arcs are walked, and the flags obeyed, as divvunspell does.
    public func lookup(_ word: String) -> Float? {
        calling { glue.lookup(word: word) }
    }
}

// [spec:hfst:req:swift.dhfst]
/// A DHFST error model, read and validated as divvunspell reads one. The
/// package offers no suggestion search, so an error model only describes
/// itself, and keeps none of its file's bytes.
public struct DhfstErrorModel: Sendable, Hashable {
    /// The format version the file declares.
    public let version: Int
    /// The number of states, one for each state of the source transducer
    /// that its start state reaches.
    public let stateCount: Int
    /// The symbol names, `@_EPSILON_SYMBOL_@` first.
    public let symbols: [String]
    /// The JSON the writer left in the file's `meta` section, or `nil` when
    /// it has none.
    public let meta: String?

    init(_ model: HFSTGlue.DhfstErrorModel) {
        version = Int(model.version)
        stateCount = Int(model.stateCount)
        symbols = model.symbols
        meta = model.meta
    }
}
