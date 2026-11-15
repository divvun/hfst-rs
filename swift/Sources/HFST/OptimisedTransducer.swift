import Foundation
internal import HFSTGlue

// [spec:hfst:req:swift.lookup]
// [spec:hfst:req:swift.threads]
/// An optimized-lookup transducer read for the optimised lookup, which is
/// divvunspell's: its results are what divvunspell gives for the same
/// transducer and word. `Transducer.lookup` gives hfst's. The two agree for
/// most words; they differ on Unicode normalisation, text outside the
/// alphabet, cycles of epsilon and flag arcs, duplicate paths and what
/// `limit` means, as `@divvun/hfst`'s MIGRATING.md describes. It never
/// changes, so tasks may look up in it at the same time.
public final class OptimisedTransducer: Sendable, CustomStringConvertible {
    let glue: HFSTGlue.OptimisedTransducer

    // [spec:hfst:req:swift.load]
    /// The one optimized-lookup transducer in `data`, weighted or unweighted,
    /// with or without an HFST3 header. Bytes in another format throw
    /// `unsupportedFormat`, naming it, and bytes that are not a transducer
    /// stream, or end partway through one, throw `invalidFormat`. A stream of
    /// several transducers, such as a pmatch container, throws
    /// `transducerCount`.
    public init(data: Data) throws(HFSTError) {
        glue = try calling { try readOptimisedTransducer(bytes: data) }
    }

    // [spec:hfst:req:swift.load]
    /// The optimized-lookup transducer in the file at `url`, as
    /// `init(data:)` reads its bytes. A file that cannot be read throws what
    /// `Data(contentsOf:)` throws.
    public convenience init(contentsOf url: URL) throws {
        try self.init(data: Data(contentsOf: url, options: .mappedIfSafe))
    }

    /// Which optimized-lookup format the transducer is in.
    public var type: TransducerType { TransducerType(glue: glue.format()) }

    /// The `name` property of its HFST3 header, or `""` when there is none.
    public var name: String { glue.name() }

    /// The properties of its HFST3 header, as `Transducer.properties` has
    /// them.
    public var properties: [String: String] { glue.properties() }

    // [spec:hfst:req:swift.lookup]
    /// What divvunspell's lookup returns for `input`: every output of a path
    /// that reads it, with the path's weight, final weight included, lightest
    /// first, then by output in code point order. Each output comes once at
    /// each weight it has. `limit` keeps the lightest results; every result
    /// is found first, so there is no time limit. A negative limit throws
    /// `invalidArgument`.
    public func lookup(_ input: String, limit: Int? = nil) throws(HFSTError) -> [LookupResult] {
        try calling { try glue.lookup(input: input, limit: limit.map(Int64.init)) }
            .map(LookupResult.init)
    }

    public var description: String {
        name.isEmpty
            ? "OptimisedTransducer(\(type.rawValue))"
            : "OptimisedTransducer(\(type.rawValue), \(name.debugDescription))"
    }
}
