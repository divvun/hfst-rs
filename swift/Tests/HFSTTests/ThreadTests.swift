import Foundation
import HFST
import Testing

@Suite struct ThreadTests {
    // [spec:hfst:req:swift.threads/test]
    @Test func tasksShareAReader() async throws {
        let transducer = try Transducer(contentsOf: Fixtures.url("words.optimized-lookup-weighted.hfst"))
        let optimised = try OptimisedTransducer(contentsOf: Fixtures.url("words.optimized-lookup-weighted.hfst"))
        let tokeniser = try Tokeniser(contentsOf: Fixtures.url("tokenise.pmhfst"))
        let alone = (
            try transducer.lookup("mouse"),
            try optimised.lookup("mouse"),
            try tokeniser.tokenise("The cat saw a dog")
        )
        let together = try await withThrowingTaskGroup(of: Bool.self) { group in
            for _ in 0..<32 {
                group.addTask {
                    try transducer.lookup("mouse") == alone.0
                        && optimised.lookup("mouse") == alone.1
                        && tokeniser.tokenise("The cat saw a dog") == alone.2
                }
            }
            return try await group.reduce(into: [Bool]()) { $0.append($1) }
        }
        #expect(together.count == 32 && together.allSatisfy { $0 })
    }
}
