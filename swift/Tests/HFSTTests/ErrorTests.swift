import Foundation
import HFST
import Testing

@Suite struct ErrorTests {
    // [spec:hfst:req:swift.errors/test]
    @Test func theCodesAreThoseOfTheNpmPackage() throws {
        let errors = try String(
            contentsOf: URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent()
                .appendingPathComponent("../../../js/hfst/errors.ts"),
            encoding: .utf8
        )
        // The npm package's wrapper codes stand for its module: starting it,
        // its traps and freeing its objects, which a native library has no
        // counterpart for.
        let wrapper: Set = ["not-initialised", "init-failed", "poisoned", "freed"]
        let listed = errors
            .components(separatedBy: "HFST_ERROR_CODES = [")[1]
            .components(separatedBy: "]")[0]
            .split(separator: "\"")
            .enumerated()
            .filter { $0.offset % 2 == 1 }
            .map { String($0.element) }
        #expect(Set(HFSTError.Code.allCases.map(\.rawValue)) == Set(listed).subtracting(wrapper))
    }

    // [spec:hfst:req:swift.errors/test]
    @Test func anErrorCarriesTheLibrarysMessage() throws {
        let error = try #require(thrown { _ = try Transducer.all(in: Data("not a transducer stream".utf8)) })
        #expect(error.code == .invalidFormat)
        #expect(error.message.hasPrefix("the bytes are not a transducer stream"))
        #expect(error.description == error.message)
        #expect(error.localizedDescription == error.message)
        #expect(error.diagnostics.isEmpty)
    }

    // [spec:hfst:req:swift.errors/test]
    @Test func anObjectStaysUsableAfterACallOnItFails() throws {
        let transducer = try Transducer(contentsOf: Fixtures.url("words.optimized-lookup-weighted.hfst"))
        #expect(thrown { _ = try transducer.lookup("cat", limit: -1) }?.code == .invalidArgument)
        #expect(try transducer.lookup("cat").map(\.output) == ["dog"])
    }
}
