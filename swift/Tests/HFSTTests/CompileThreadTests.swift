import Foundation
import HFSTCompile
import Testing

private func words() throws -> Transducer {
    try Transducer(contentsOf: Fixtures.url("words.openfst-tropical.hfst"))
}

private func extracted(_ t: Transducer) throws -> [String] {
    try t.extractPaths().map { "\($0.input)\t\($0.output)\t\($0.weight)" }.sorted()
}

@Suite struct CompileThreadTests {
    // [spec:hfst:req:swift.threads/test]
    @Test func anOperationOnItselfReadsTheOriginal() throws {
        let operations: [(String, (Transducer, Transducer) throws -> Void)] = [
            ("compose", { try $0.compose($1) }),
            ("concatenate", { try $0.concatenate($1) }),
            ("disjunct", { try $0.disjunct($1) }),
            ("conjunct", { try $0.conjunct($1) }),
            ("subtract", { try $0.subtract($1) }),
        ]
        let original = try words()
        for (name, operation) in operations {
            let itself = try original.copy()
            try operation(itself, itself)
            let other = try original.copy()
            try operation(other, try original.copy())
            #expect(try extracted(itself) == extracted(other), "\(name)")
        }
        let lexicon = try original.copy()
        try lexicon.composeIntersect([lexicon])
        let expected = try original.copy()
        try expected.composeIntersect([try original.copy()])
        #expect(try extracted(lexicon) == extracted(expected))
    }

    // [spec:hfst:req:swift.threads/test]
    @Test func crossedOperationsTakeTurns() async throws {
        let a = try words()
        let b = try words()
        try await withThrowingTaskGroup(of: Void.self) { group in
            for (first, second) in [(a, b), (b, a)] {
                group.addTask {
                    for _ in 0..<30 {
                        try first.disjunct(second)
                        try first.minimize()
                        _ = try first.checkAlpha(second)
                    }
                }
            }
            try await group.waitForAll()
        }
        #expect(try a.lookup("cat").map(\.output) == ["dog"])
    }

    // [spec:hfst:req:swift.threads/test]
    @Test func aCallWaitsForAChange() async throws {
        let t = try words()
        let expected = try extracted(t)
        let seen = try await withThrowingTaskGroup(of: [String].self) { group in
            group.addTask {
                for _ in 0..<20 {
                    try t.convert(to: .foma)
                    try t.minimize()
                    try t.convert(to: .openFstTropical)
                    try t.determinize()
                }
                return []
            }
            for _ in 0..<4 {
                group.addTask {
                    (0..<20).compactMap { _ in
                        (try? t.lookup("cat"))?.map(\.output).joined()
                    }
                }
            }
            return try await group.reduce(into: [String]()) { $0 += $1 }
        }
        #expect(!seen.isEmpty && seen.allSatisfy { $0 == "dog" })
        #expect(try extracted(t).map { $0.split(separator: "\t").prefix(2).joined() }
            == expected.map { $0.split(separator: "\t").prefix(2).joined() })
    }
}
