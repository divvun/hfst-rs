// [spec:hfst:req:swift.compile-product+1]
// HFSTCompile: what @divvun/hfst/compile and divvun-hfst offer beyond HFST.
// It compiles regular expressions, lexc, twolc, xfst scripts, pmatch
// scripts, strings and AT&T and Prolog text; it reads transducers of every
// type the hfst library reads and transforms, inspects and writes them as
// the hfst tools do. It re-exports HFST, so one import gives both, and its
// Transducer, which holds any type, takes the place of HFST's under that
// import, as Swift resolves a type name to the re-exporting module's own
// declaration. HFST's Tokeniser gains pmatch.

@_exported public import HFST
import Foundation
internal import HFSTCompileGlue
internal import HFSTGlue

/// A compile's value and the warnings it gave on the way.
public struct Compiled<Value: Sendable>: Sendable {
    public let value: Value
    /// Each warning the compile gave, and each error it made without
    /// failing. What it says at the info level goes to the log instead.
    public let warnings: [HFST.Diagnostic]
}

/// The ranges of `diagnostics` made into ranges of `sources`.
func diagnostics(_ diagnostics: [HFSTGlue.Diagnostic], in sources: [String]) -> [HFST.Diagnostic] {
    diagnostics.map { HFST.Diagnostic($0, sources: sources) }
}
