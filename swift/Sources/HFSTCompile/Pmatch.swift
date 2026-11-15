import Foundation
internal import HFSTCompileGlue
internal import HFSTGlue

// [spec:hfst:req:swift.operations]
/// A match `hfst pmatch --locate` reports.
public struct PmatchLocation: Sendable, Hashable {
    /// Where it starts in the piece, as the library counts.
    public let start: Int
    public let length: Int
    public let output: String
    public let tag: String
    public let weight: Float
}

// [spec:hfst:req:swift.operations]
/// What `hfst pmatch` reports for a piece of its input.
public struct PmatchResult: Sendable, Hashable {
    /// The piece: a line, or a run of lines up to an empty one.
    public let input: String
    /// The piece with its matches marked up, unless `locate`.
    public let marked: String?
    /// Its matches, with `locate`.
    public let located: [PmatchLocation]?
}

// [spec:hfst:req:swift.compile-product+1]
// HFST's Tokeniser, read from a pmatch container, gains what `hfst pmatch`
// does with one.
extension HFST.Tokeniser {
    // [spec:hfst:req:swift.operations]
    /// What `hfst pmatch` reports for `text` with the tokeniser's container:
    /// for each piece of it, each run of lines up to an empty one or with
    /// `newline` each line, the piece with its matches marked or with
    /// `locate` its matches. The options left `nil` keep the container's own
    /// settings.
    public func pmatch(
        _ text: String,
        newline: Bool = false,
        locate: Bool = false,
        extractPatterns: Bool? = nil,
        deletePatterns: Bool? = nil,
        markPatterns: Bool? = nil,
        maxContext: Int? = nil,
        maxRecursion: Int? = nil,
        timeCutoff: TimeInterval = 0,
        weightCutoff: Float? = nil
    ) throws(HFSTError) -> [PmatchResult] {
        let settings = PmatchRunSettings(
            newline: newline,
            locate: locate,
            extractPatterns: extractPatterns,
            deletePatterns: deletePatterns,
            markPatterns: markPatterns,
            maxContext: maxContext.map(Int64.init),
            maxRecursion: maxRecursion.map(Int64.init),
            timeCutoff: timeCutoff,
            weightCutoff: weightCutoff
        )
        return try calling {
            try HFSTCompileGlue.pmatch(tokeniser: glue, text: text, settings: settings)
        }.map { piece in
            PmatchResult(
                input: piece.input,
                marked: piece.marked,
                located: piece.located?.map {
                    PmatchLocation(
                        start: Int($0.start),
                        length: Int($0.length),
                        output: $0.output,
                        tag: $0.tag,
                        weight: $0.weight
                    )
                }
            )
        }
    }
}
