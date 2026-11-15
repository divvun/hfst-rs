import Foundation
import HFST
import Testing

// The parity check: the package against the native library.
// js/test/parity-fixtures.mjs writes parity.json beside the fixtures: what
// the native hfst library gives for each of them, what native divvun-fst's
// lookup gives, and what hfst tokenise prints. The npm and Python packages'
// parity suites run the same file. Each entry runs through the built
// package and its result is compared with the native one value for value:
// both hold each weight as the double that holds its f32 exactly, so no
// tolerance is needed.

nonisolated(unsafe) private let native: [String: [[String: Any]]] = {
    (try? Fixtures.json("parity.json") as? [String: [[String: Any]]]) ?? [:]
}()

/// `value` with each decimal number in it as the double nearest it.
/// JSONSerialization reads a number with many digits, such as a weight
/// written as the double that holds its f32, as an NSDecimalNumber, which
/// equals no double: its digits are not the double's exact binary value.
/// Its `doubleValue` can miss the nearest double by one place, so its
/// digits are read again.
private func doubles(_ value: Any) -> Any {
    switch value {
    case let decimal as NSDecimalNumber: Double(decimal.stringValue) ?? decimal.doubleValue
    case let object as [String: Any]: object.mapValues(doubles)
    case let array as [Any]: array.map(doubles)
    default: value
    }
}

/// Each entry of `section` whose result differs from the native one. A
/// section with no entries is an issue: the expectations are missing, not
/// met.
private func differences(
    _ section: String,
    _ run: ([String: Any]) throws -> (ours: Any, native: Any)
) rethrows -> [String] {
    let entries = native[section] ?? []
    #expect(!entries.isEmpty, "parity.json holds no \(section) entries")
    var found: [String] = []
    for entry in entries {
        let (ours, theirs) = try run(entry)
        if !(ours as AnyObject).isEqual(doubles(theirs)) {
            let what = entry["name"] ?? entry["input"] ?? entry["word"] ?? ""
            found.append("\(entry["file"] ?? "") \(what): \(ours), native \(theirs)")
        }
    }
    return found
}

/// What `make` makes of each fixture, made once.
private final class Loaded<T>: @unchecked Sendable {
    private var read: [String: T] = [:]
    private let make: (Data) throws -> T

    init(_ make: @escaping (Data) throws -> T) {
        self.make = make
    }

    func callAsFunction(_ name: String) throws -> T {
        if let known = read[name] { return known }
        let made = try make(Fixtures.data(name))
        read[name] = made
        return made
    }
}

private func results(_ found: [LookupResult]) -> [[String: Any]] {
    found.map { ["output": $0.output, "symbols": $0.symbols, "weight": Double($0.weight)] }
}

private func limit(_ entry: [String: Any]) -> Int? {
    entry["limit"] as? Int
}

/// The `TokeniseOptions` an npm package options object stands for, its time
/// limit in milliseconds.
private func tokeniseOptions(_ given: [String: Any]) -> TokeniseOptions {
    var options = TokeniseOptions()
    if let dedupe = given["dedupe"] as? Bool { options.dedupe = dedupe }
    options.maxWeightClasses = given["maxWeightClasses"] as? Int
    options.beam = (given["beam"] as? Double).map(Float.init)
    options.weightCutoff = (given["weightCutoff"] as? Double).map(Float.init)
    options.timeLimit = (given["timeLimit"] as? Double).map { $0 / 1000 }
    if let multichar = given["tokenizeMultichar"] as? Bool { options.tokenizeMultichar = multichar }
    if let printAll = given["printAll"] as? Bool { options.printAll = printAll }
    return options
}

@Suite struct ParityTests {
    // [spec:hfst:req:swift.parity/test]
    @Test func lookupsAreTheNativeLibrarys() throws {
        let streams = Loaded(Transducer.all(in:))
        var members: [String: Int] = [:]
        for entry in native["lookup"] ?? [] {
            let file = entry["file"] as? String ?? ""
            members[file] = max(members[file] ?? 0, (entry["member"] as? Int ?? 0) + 1)
        }
        for (file, count) in members {
            #expect(try streams(file).count == count, "\(file)")
        }
        let found = try differences("lookup") { entry in
            let transducer = try streams(entry["file"] as? String ?? "")[entry["member"] as? Int ?? 0]
            let input = entry["input"] as? String ?? ""
            return (results(try transducer.lookup(input, limit: limit(entry))), entry["results"] ?? [])
        }
        #expect(found.isEmpty, "\(found.count) differ:\n\(found.prefix(20).joined(separator: "\n"))")
    }

    // [spec:hfst:req:swift.parity/test]
    @Test func optimisedLookupsAreNativeDivvunFsts() throws {
        let transducers = Loaded(OptimisedTransducer.init(data:))
        let found = try differences("optimised") { entry in
            let transducer = try transducers(entry["file"] as? String ?? "")
            let input = entry["input"] as? String ?? ""
            return (results(try transducer.lookup(input, limit: limit(entry))), entry["results"] ?? [])
        }
        #expect(found.isEmpty, "\(found.count) differ:\n\(found.prefix(20).joined(separator: "\n"))")
    }

    // [spec:hfst:req:swift.parity/test]
    @Test func tokensAreTheNativeLibrarys() throws {
        let tokenisers = Loaded(Tokeniser.init(data:))
        let found = try differences("tokens") { entry in
            let text = entry["text"] as? String ?? ""
            let options = tokeniseOptions(entry["options"] as? [String: Any] ?? [:])
            let tokens = try tokenisers(entry["file"] as? String ?? "").tokenise(text, options: options)
            for token in tokens {
                #expect(String(text[token.range]) == token.surface, "\(entry["name"] ?? "") \(token.surface)")
            }
            let ours: [[String: Any]] = tokens.map { token in
                [
                    "surface": token.surface,
                    "analysed": token.analysed,
                    "readings": token.readings.map { ["analysis": $0.analysis, "weight": Double($0.weight)] },
                ]
            }
            return (ours, entry["tokens"] ?? [])
        }
        #expect(found.isEmpty, "\(found.count) differ:\n\(found.prefix(20).joined(separator: "\n"))")
    }

    // [spec:hfst:req:swift.parity/test]
    @Test func formatsAreWhatHfstTokenisePrints() throws {
        let tokenisers = Loaded(Tokeniser.init(data:))
        let found = try differences("formats") { entry in
            let format = try #require(TokeniseFormat(rawValue: entry["format"] as? String ?? ""))
            let options = formatOptions(entry["options"] as? [String: Any] ?? [:])
            let text = entry["text"] as? String ?? ""
            let printed = try tokenisers(entry["file"] as? String ?? "").format(text, as: format, options: options)
            return (printed, entry["printed"] ?? "")
        }
        #expect(found.isEmpty, "\(found.count) differ:\n\(found.prefix(20).joined(separator: "\n"))")
    }

    // [spec:hfst:req:swift.parity/test]
    @Test func acceptorLookupsAreTheNativeLibrarys() throws {
        let models = Loaded(Dhfst.init(data:))
        let found = try differences("acceptors") { entry in
            guard case let .acceptor(acceptor) = try models(entry["file"] as? String ?? "") else {
                return ("not an acceptor", entry)
            }
            let weight: Any = acceptor.lookup(entry["word"] as? String ?? "").map { Double($0) } ?? NSNull()
            return (weight, entry["weight"] ?? NSNull())
        }
        #expect(found.isEmpty, "\(found.count) differ:\n\(found.prefix(20).joined(separator: "\n"))")
    }

    // [spec:hfst:req:swift.parity/test]
    @Test func dhfstDescriptionsAreTheNativeLibrarys() throws {
        let found = try differences("models") { entry in
            let file = entry["file"] as? String ?? ""
            var described: [String: Any] = ["file": file]
            switch try Dhfst(data: Fixtures.data(file)) {
            case let .acceptor(acceptor):
                let info = acceptor.info
                described["type"] = "acceptor"
                described["version"] = acceptor.version
                described["symbols"] = acceptor.symbols
                described["meta"] = acceptor.meta ?? NSNull()
                described["info"] = [
                    "ids": info.ids, "slots": info.slots, "usedSlots": info.usedSlots,
                    "checkBytes": info.checkBytes, "labelMax": info.labelMax,
                    "recordBytes": info.recordBytes, "listRecords": info.listRecords,
                    "arcs": info.arcs, "freeArcs": info.freeArcs, "freePairs": info.freePairs,
                    "weights": info.weights, "finals": info.finals, "distances": info.distances,
                ] as [String: Any]
            case let .errorModel(model):
                described["type"] = "error-model"
                described["version"] = model.version
                described["symbols"] = model.symbols
                described["meta"] = model.meta ?? NSNull()
                described["stateCount"] = model.stateCount
            }
            return (described, entry)
        }
        #expect(found.isEmpty, "\(found.count) differ:\n\(found.prefix(20).joined(separator: "\n"))")
    }
}
