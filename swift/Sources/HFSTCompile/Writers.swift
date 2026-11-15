import Foundation
internal import HFSTCompileGlue

// [spec:hfst:req:swift.writers]
// [spec:hfst:req:swift.threads]
// Each writer writes the transducer as the hfst tool it names writes it,
// and changes nothing.
extension Transducer {
    /// The bytes of a stream holding the transducer, as `hfst fst2fst`
    /// writes it: in `type`, converted and named as `--format` converts and
    /// names it, or as it is, in its own type, when `type` is `nil`.
    /// `useBackendFormat` leaves out the HFST3 header, and `quick` packs
    /// optimized-lookup tables less tightly.
    public func data(
        type: TransducerType? = nil,
        useBackendFormat: Bool = false,
        quick: Bool = false
    ) throws(HFSTError) -> Data {
        try calling { try glue.toBytes(format: type?.rawValue, useBackendFormat: useBackendFormat, quick: quick) }
    }

    /// The AT&T text `hfst fst2txt` writes of the transducer. Weights are
    /// written for every type but foma, which has none, unless
    /// `doNotPrintWeights`; `printWeights` writes them for foma too.
    /// `useNumbers` writes symbol numbers rather than names.
    public func attText(
        printWeights: Bool = false,
        doNotPrintWeights: Bool = false,
        useNumbers: Bool = false
    ) throws(HFSTError) -> String {
        try text("att", printWeights, doNotPrintWeights, useNumbers)
    }

    /// The Prolog text `hfst fst2txt -f prolog` writes of the transducer
    /// alone in its stream, so a transducer with no name is `NO_NAME_1`.
    public func prologText(printWeights: Bool = false, doNotPrintWeights: Bool = false) throws(HFSTError) -> String {
        try text("prolog", printWeights, doNotPrintWeights, false)
    }

    /// The Graphviz text `hfst fst2txt -f dot` writes of the transducer.
    public func dotText(printWeights: Bool = false, doNotPrintWeights: Bool = false) throws(HFSTError) -> String {
        try text("dot", printWeights, doNotPrintWeights, false)
    }

    /// The PC-KIMMO text `hfst fst2txt -f pckimmo` writes of the transducer.
    public func pckimmoText(printWeights: Bool = false, doNotPrintWeights: Bool = false) throws(HFSTError) -> String {
        try text("pckimmo", printWeights, doNotPrintWeights, false)
    }

    private func text(
        _ format: String,
        _ printWeights: Bool,
        _ doNotPrintWeights: Bool,
        _ useNumbers: Bool
    ) throws(HFSTError) -> String {
        try calling {
            try glue.toText(
                format: format,
                printWeights: printWeights,
                doNotPrintWeights: doNotPrintWeights,
                useNumbers: useNumbers
            )
        }
    }

    /// The DHFST acceptor (type 2) `hfst fst2fst -f dhfst --dhfst-type
    /// acceptor` writes of the transducer. `sourceName` is the file name,
    /// without its directory, the tool would have read it from, which the
    /// file's meta section names; `quick` packs the conversion's tables less
    /// tightly.
    public func dhfstAcceptorData(quick: Bool = false, sourceName: String = "") throws(HFSTError) -> Data {
        try calling { try glue.toDhfstAcceptor(quick: quick, sourceName: sourceName) }
    }

    /// The DHFST error model (type 1) `hfst fst2fst -f dhfst --dhfst-type
    /// errmodel` writes of the transducer, its fallback chains at most
    /// `maxFallbackDepth` long, as `--max-fallback-depth`, or unbounded for
    /// `nil`, as `--unbounded-fallback`.
    public func dhfstErrorModelData(
        maxFallbackDepth: Int? = 4,
        quick: Bool = false,
        sourceName: String = ""
    ) throws(HFSTError) -> Data {
        try calling {
            try glue.toDhfstErrorModel(
                maxFallbackDepth: maxFallbackDepth.map { UInt32(clamping: $0) },
                quick: quick,
                sourceName: sourceName
            )
        }
    }

    /// Writes the transducer as the THFST directory at the file URL `url`,
    /// divvunspell's format, as `hfst fst2fst -f thfst -o PATH` writes it:
    /// the directory and its three files `alphabet`, `index` and
    /// `transition`. `quick` packs the tables less tightly.
    public func writeThfst(to url: URL, quick: Bool = false) throws(HFSTError) {
        try calling { try glue.writeThfst(path: url.path, quick: quick) }
    }
}

// [spec:hfst:req:swift.writers]
/// The BHFST archive `hfst bhfst` packs of a speller: `acceptor` and
/// `errorModel` each the bytes of a DHFST file, packed as they are, or of a
/// transducer stream, converted to THFST; and `metadata`, the text of the
/// archive's meta.json, or with `indexXML` of a zhfst index.xml to convert,
/// as `--meta` and `--index-xml` take them. `spellerConfig` is the text of
/// the speller runtime configuration `--speller-config` adds to meta.json.
public func packBhfst(
    acceptor: Data,
    errorModel: Data,
    metadata: String? = nil,
    indexXML: Bool = false,
    spellerConfig: String? = nil
) throws(HFSTError) -> Data {
    try calling {
        try HFSTCompileGlue.packBhfst(
            acceptor: acceptor,
            errorModel: errorModel,
            metadata: metadata.map { Data($0.utf8) },
            indexXml: indexXML,
            spellerConfig: spellerConfig.map { Data($0.utf8) }
        )
    }
}
