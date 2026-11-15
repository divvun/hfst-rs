import Foundation
import HFST
import Testing

private let text = "The cat saw a dog in New York, blorf! Øl 🐱 blåbær 🐱🐱."

private func tokeniser() throws -> Tokeniser {
    try Tokeniser(contentsOf: Fixtures.url("tokenise.pmhfst"))
}

/// The `FormatOptions` an npm package options object stands for, its time
/// limit in milliseconds.
func formatOptions(_ given: [String: Any]) -> FormatOptions {
    var options = FormatOptions()
    options.dedupe = given["dedupe"] as? Bool
    options.maxWeightClasses = given["maxWeightClasses"] as? Int
    options.beam = (given["beam"] as? Double).map(Float.init)
    options.timeLimit = (given["timeLimit"] as? Double).map { $0 / 1000 }
    options.tokenizeMultichar = given["tokenizeMultichar"] as? Bool
    options.printAll = given["printAll"] as? Bool
    options.printWeights = given["printWeights"] as? Bool
    options.hackUncompose = given["hackUncompose"] as? Bool
    options.newline = given["newline"] as? Bool
    options.keepNewline = given["keepNewline"] as? Bool
    options.superblanks = given["superblanks"] as? Bool
    return options
}

@Suite struct TokeniseTests {
    // [spec:hfst:req:swift.tokenise/test]
    @Test func rangesIndexTheText() throws {
        let tokeniser = try tokeniser()
        for options in [TokeniseOptions(), TokeniseOptions(printAll: true)] {
            let tokens = try tokeniser.tokenise(text, options: options)
            for token in tokens {
                #expect(String(text[token.range]) == token.surface)
                #expect(Array(Array(text.utf8)[token.utf8Range]) == Array(token.surface.utf8))
            }
            let starts = tokens.map(\.range.lowerBound)
            #expect(starts == starts.sorted())
        }
        let cats = try tokeniser.tokenise(text).filter { $0.surface == "🐱" }
        #expect(!cats.isEmpty && cats.allSatisfy { text[$0.range].count == 1 && $0.utf8Range.count == 4 })
    }

    // [spec:hfst:req:swift.tokenise/test]
    @Test func tokensAreTheCGCohorts() throws {
        let tokeniser = try tokeniser()
        let tokens = try tokeniser.tokenise(text)
        let printed = try tokeniser.format(text, as: .cg, options: FormatOptions(printWeights: true))
        let cohorts = printed.components(separatedBy: "\n\n").filter {
            !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
        #expect(cohorts.map { String($0.split(separator: "\n")[0]) } == tokens.map { "\"<\($0.surface)>\"" })
        #expect(cohorts.map { $0.split(separator: "\n").count - 1 } == tokens.map(\.readings.count))
        let saw = try #require(tokens.first { $0.surface == "saw" })
        #expect(saw.analysed)
        #expect(saw.readings.prefix(2).map(\.analysis) == ["see+V+Past", "saw+N"])
        #expect(tokens.first { $0.surface == "blorf" }?.analysed == false)
    }

    // [spec:hfst:req:swift.tokenise/test]
    @Test func formatPrintsWhatHfstTokenisePrints() throws {
        let tokeniser = try tokeniser()
        let cases = try #require(Fixtures.json("tokenise-cases.json") as? [String: Any])
        let texts = try #require(cases["texts"] as? [String: String])
        let formats = try #require(cases["formats"] as? [[String: Any]])
        #expect(!formats.isEmpty)
        for format in formats {
            let name = try #require(format["name"] as? String)
            let textName = try #require(format["text"] as? String)
            let given = try #require(texts[textName])
            let formatName = try #require(format["format"] as? String)
            let kind = try #require(TokeniseFormat(rawValue: formatName))
            let options = formatOptions(try #require(format["options"] as? [String: Any]))
            let got = try tokeniser.format(given, as: kind, options: options)
            #expect(got == (try Fixtures.text("tokenise.\(name).out")), "\(name)")
        }
    }

    // [spec:hfst:req:swift.errors/test]
    @Test func optionValuesAreChecked() throws {
        let tokeniser = try tokeniser()
        let refused: [TokeniseOptions] = [
            TokeniseOptions(maxWeightClasses: 0),
            TokeniseOptions(maxWeightClasses: -3),
            TokeniseOptions(beam: -0.5),
            TokeniseOptions(beam: .nan),
            TokeniseOptions(weightCutoff: .nan),
            TokeniseOptions(timeLimit: -1),
        ]
        for options in refused {
            #expect(thrown { _ = try tokeniser.tokenise("cat", options: options) }?.code == .invalidArgument)
        }
        _ = try tokeniser.tokenise("cat", options: TokeniseOptions(maxWeightClasses: .max, weightCutoff: .infinity))
        #expect(thrown { _ = try tokeniser.format("cat", options: FormatOptions(beam: -1)) }?.code == .invalidArgument)
    }

    // [spec:hfst:req:swift.tokenise/test]
    @Test func formatNamesAreHfstTokenises() {
        #expect(TokeniseFormat.allCases.map(\.rawValue) == [
            "segment", "space-separated", "xerox", "cg", "finnpos", "giella-cg", "conllu", "visl",
        ])
    }

    // [spec:hfst:req:swift.tokenise/test]
    @Test func tokeniseOptionsChangeTheTokens() throws {
        let tokeniser = try tokeniser()
        #expect(try tokeniser.tokenise("The cat blorf").map(\.surface) == ["The", "cat", "blorf"])
        let every = try tokeniser.tokenise("The cat blorf", options: TokeniseOptions(printAll: true))
        #expect(every.map(\.surface) == ["The", " ", "cat", " ", "blorf"])
        #expect(every[1].readings.isEmpty && !every[1].analysed)
        let cut = try tokeniser.tokenise("The cat blorf", options: TokeniseOptions(weightCutoff: 50))
        #expect(cut.map(\.surface) == ["cat"])
        let one = try tokeniser.tokenise("saw", options: TokeniseOptions(maxWeightClasses: 1))
        #expect(one[0].readings.map(\.analysis) == ["see+V+Past"])
    }

    // [spec:hfst:req:swift.tokenise/test]
    @Test func tokensAreValues() throws {
        let tokeniser = try tokeniser()
        let cat = try #require(tokeniser.tokenise("cat").first)
        #expect(cat == (try tokeniser.tokenise("cat")[0]))
        #expect(cat.utf8Range == 0..<3)
        #expect(cat.readings.map(\.analysis) == ["cat+N", "cat ??"])
        #expect(cat.readings.map(\.weight) == [0, 100])
    }
}
