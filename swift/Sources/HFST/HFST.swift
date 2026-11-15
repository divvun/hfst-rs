// [spec:hfst:def:swift.package]
// HFST: Divvun HFST for Swift. It reads optimized-lookup transducers, pmatch
// tokenisers and DHFST spellers, and looks words up, tokenises text and
// checks words against DHFST acceptors with them, as @divvun/hfst's main
// entry point does. HFSTCompile adds compiling, transforming and writing.
//
// The files whose package declarations take the glue's types import it at
// package level, so HFSTCompile, whose glue throws and returns the same
// types, shares their conversions. None of it is public.

import Foundation
internal import HFSTGlue

// [spec:hfst:req:swift.version]
/// The package's version. It is versioned on its own, apart from the hfst
/// library inside it.
public let version: String = HFSTGlue.version()

/// Runs a call into the glue with logging set up, turning what it throws
/// into an `HFSTError` whose diagnostics span `sources`, the sources the
/// caller gave the call in the order the glue numbers them.
package func calling<T>(
    sources: [String] = [],
    _ body: () throws -> T
) throws(HFSTError) -> T {
    HFSTLogging.install()
    do {
        return try body()
    } catch let error as GlueError {
        throw HFSTError(error, sources: sources)
    } catch {
        throw HFSTError(code: .internal, message: String(describing: error))
    }
}

/// Runs a call into the glue that cannot fail, with logging set up.
package func calling<T>(_ body: () -> T) -> T {
    HFSTLogging.install()
    return body()
}
