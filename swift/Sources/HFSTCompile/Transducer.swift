import Foundation
internal import HFSTCompileGlue
internal import HFSTGlue

// [spec:hfst:req:swift.transducer]
// [spec:hfst:req:swift.threads]
/// A transducer of any type the hfst library reads: tropical OpenFst, foma,
/// and optimized lookup, weighted and unweighted. Its operations change it
/// in place and return nothing, as `sort()` does; `copy()` gives one of its
/// own to change. Tasks may share one: an operation holds it for its length,
/// and a call on a transducer another task is changing waits until that
/// change is done. Make one with `all(in:)`, `init(data:)`, their
/// `contentsOf:` forms or a compiler.
public final class Transducer: Sendable, CustomStringConvertible {
    let glue: HFSTCompileGlue.Transducer

    init(glue: HFSTCompileGlue.Transducer) {
        self.glue = glue
    }

    /// Every transducer in `data`, the bytes of an HFST transducer stream, in
    /// stream order, of any type the library reads, with an HFST3 header or
    /// without one. A pmatch container reads as TOP followed by its
    /// definitions, each under its name. Bytes that are not a transducer
    /// stream throw an `HFSTError` whose code says why.
    public static func all(in data: Data) throws(HFSTError) -> [Transducer] {
        try calling { try HFSTCompileGlue.readTransducers(bytes: data) }.map(Transducer.init(glue:))
    }

    /// Every transducer at the file URL `url`, as `all(in:)` reads a file's
    /// bytes; a THFST directory, as `hfst fst2fst -f thfst` writes one, reads
    /// as its one transducer, in weighted optimized lookup. A file that
    /// cannot be read throws what `Data(contentsOf:)` throws.
    public static func all(contentsOf url: URL) throws -> [Transducer] {
        var directory: ObjCBool = false
        if FileManager.default.fileExists(atPath: url.path, isDirectory: &directory), directory.boolValue {
            return [Transducer(glue: try calling { try readThfst(path: url.path) })]
        }
        return try all(in: Data(contentsOf: url, options: .mappedIfSafe))
    }

    /// The one transducer in `data`, as `all(in:)` reads it. A stream
    /// holding more throws `transducerCount`.
    public convenience init(data: Data) throws(HFSTError) {
        self.init(glue: try calling { try HFSTCompileGlue.readTransducer(bytes: data) })
    }

    /// The one transducer at `url`, as `all(contentsOf:)` reads it.
    public convenience init(contentsOf url: URL) throws {
        let all = try Transducer.all(contentsOf: url)
        guard all.count == 1 else {
            throw HFSTError(
                code: .transducerCount,
                message: "Transducer(contentsOf:) takes exactly one transducer, and \(url.path) holds \(all.count)"
            )
        }
        self.init(glue: all[0].glue)
    }

    public var type: TransducerType { TransducerType(glue: glue.format()) }

    /// The `name` property of its HFST3 header, or `""` when there is none.
    public var name: String { glue.name() }

    /// The properties of its HFST3 header, such as a pmatch container's
    /// settings. `name` and `type` have their own properties, and the
    /// header's `version` belongs to the stream rather than the transducer,
    /// so none of the three is here.
    public var properties: [String: String] { glue.properties() }

    // [spec:hfst:req:swift.transducer]
    /// Every result the hfst library's lookup finds for `input`, lightest
    /// first, then by output in code point order, then by symbols. In
    /// optimized lookup, flag diacritics are obeyed; `limit` stops the
    /// search once it has found that many paths, and `timeLimit` after that
    /// many seconds, 0 setting none. In the other types the results are what
    /// `hfst lookup` finds, in its order, at most `limit` of them.
    public func lookup(
        _ input: String,
        limit: Int? = nil,
        timeLimit: TimeInterval? = nil
    ) throws(HFSTError) -> [HFST.LookupResult] {
        try calling {
            try glue.lookup(input: input, limit: limit.map(Int64.init), timeLimit: timeLimit)
        }.map(HFST.LookupResult.init)
    }

    // [spec:hfst:req:swift.transducer]
    /// A transducer of its own with the same contents, which changes
    /// independently of this one.
    public func copy() throws(HFSTError) -> Transducer {
        Transducer(glue: try calling { try glue.copy() })
    }

    public var description: String {
        name.isEmpty ? "Transducer(\(type.rawValue))" : "Transducer(\(type.rawValue), \(name.debugDescription))"
    }
}
