import Foundation
package import HFSTGlue

// [spec:hfst:req:swift.names]
/// The transducer types, under the names `hfst fst2fst --format` gives them,
/// which are the raw values. `HFST` reads the two optimized-lookup types;
/// `HFSTCompile` reads and writes all four.
public enum TransducerType: String, Sendable, Hashable, CaseIterable {
    /// HFST's weighted optimized-lookup format, which analysers, spellers and
    /// pmatch containers use.
    case optimizedLookupWeighted = "optimized-lookup-weighted"
    case optimizedLookupUnweighted = "optimized-lookup-unweighted"
    /// OpenFst with tropical weights, what the compilers build by default.
    case openFstTropical = "openfst-tropical"
    /// foma's unweighted transducers.
    case foma

    package init(glue name: String) {
        self = TransducerType(rawValue: name) ?? .optimizedLookupWeighted
    }
}

// [spec:hfst:req:swift.lookup]
/// One path a lookup found: its output symbols, joined and one by one, and
/// its weight. Flag diacritics and epsilons are left out of both, as `hfst
/// lookup` leaves them out of what it prints.
public struct LookupResult: Sendable, Hashable {
    public let output: String
    public let symbols: [String]
    /// The sum of the weights along the path. Lower is better.
    public let weight: Float

    package init(_ result: HFSTGlue.LookupResult) {
        output = result.output
        symbols = result.symbols
        weight = result.weight
    }
}

// [spec:hfst:req:swift.lookup]
// [spec:hfst:req:swift.threads]
/// An optimized-lookup transducer, looked up in with the hfst library's
/// engine. Its results are what `hfst lookup` prints; `OptimisedTransducer`
/// gives divvunspell's instead. It never changes, so tasks may look up in it
/// at the same time.
public final class Transducer: Sendable, CustomStringConvertible {
    let glue: HFSTGlue.Transducer

    // [spec:hfst:req:swift.load]
    /// Every transducer in `data`, the bytes of an HFST transducer stream, in
    /// stream order. A pmatch container reads as TOP followed by its
    /// definitions, each under its name. Optimized-lookup transducers read
    /// with an HFST3 header or without one. Bytes in another format throw
    /// `unsupportedFormat`, naming it; bytes that are not a transducer
    /// stream, or end partway through one, throw `invalidFormat`.
    public static func all(in data: Data) throws(HFSTError) -> [Transducer] {
        try calling { try readTransducers(bytes: data) }.map(Transducer.init(glue:))
    }

    // [spec:hfst:req:swift.load]
    /// Every transducer in the file at `url`, as `all(in:)` reads its bytes.
    /// A file that cannot be read throws what `Data(contentsOf:)` throws.
    public static func all(contentsOf url: URL) throws -> [Transducer] {
        try all(in: Data(contentsOf: url, options: .mappedIfSafe))
    }

    // [spec:hfst:req:swift.load]
    /// The one transducer in `data`, as `all(in:)` reads it. A stream holding
    /// more throws `transducerCount`.
    public init(data: Data) throws(HFSTError) {
        glue = try calling { try readTransducer(bytes: data) }
    }

    // [spec:hfst:req:swift.load]
    /// The one transducer in the file at `url`, as `init(data:)` reads its
    /// bytes.
    public convenience init(contentsOf url: URL) throws {
        try self.init(data: Data(contentsOf: url, options: .mappedIfSafe))
    }

    init(glue: HFSTGlue.Transducer) {
        self.glue = glue
    }

    /// Which optimized-lookup format the transducer is in.
    public var type: TransducerType { TransducerType(glue: glue.format()) }

    /// The `name` property of its HFST3 header, or `""` when there is none.
    public var name: String { glue.name() }

    /// The properties of its HFST3 header, such as a pmatch container's
    /// settings. `name` and `type` have their own properties, and the
    /// header's `version` belongs to the stream rather than the transducer,
    /// so none of the three is here.
    public var properties: [String: String] { glue.properties() }

    // [spec:hfst:req:swift.lookup]
    /// Every result the hfst library's lookup finds for `input`, lightest
    /// first, then by output in code point order, then by symbols. Flag
    /// diacritics are obeyed, so a path they block is not a result. `limit`
    /// stops the search once it has found that many paths, so they are the
    /// first it found, not the lightest; `timeLimit` stops it after that many
    /// seconds, and the lookup returns what it found. 0 sets no time limit.
    /// A negative limit throws `invalidArgument`.
    public func lookup(
        _ input: String,
        limit: Int? = nil,
        timeLimit: TimeInterval? = nil
    ) throws(HFSTError) -> [LookupResult] {
        try calling {
            try glue.lookup(input: input, limit: limit.map(Int64.init), timeLimit: timeLimit)
        }.map(LookupResult.init)
    }

    public var description: String {
        name.isEmpty ? "Transducer(\(type.rawValue))" : "Transducer(\(type.rawValue), \(name.debugDescription))"
    }
}
