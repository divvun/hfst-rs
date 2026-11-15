import Foundation
package import HFSTGlue

// [spec:hfst:req:swift.errors]
/// A failure the hfst library reports. `code` says what went wrong, in a form
/// code can test, as `URLError` and `CocoaError` do; the codes are
/// `@divvun/hfst`'s and do not change between releases.
public struct HFSTError: Error, Sendable, Hashable, CustomStringConvertible, LocalizedError {
    public enum Code: String, Sendable, Hashable, CaseIterable {
        /// Transducers of different types, or one of the wrong type for the
        /// operation.
        case typeMismatch = "type-mismatch"
        /// Bytes in a format the package does not read. The message names
        /// it.
        case unsupportedFormat = "unsupported-format"
        /// Input that is not well formed in the format expected, such as
        /// bytes that end partway through a transducer.
        case invalidFormat = "invalid-format"
        /// A stream or file that cannot be read or written.
        case io
        /// An operation that needs automata got transducers.
        case notAutomaton = "not-automaton"
        /// An operation that needs a finite language got a cyclic one.
        case cyclic
        /// An argument the call cannot use, such as a negative limit, an
        /// empty string or an unknown symbol.
        case invalidArgument = "invalid-argument"
        /// A failure inside the library that no input should cause.
        case `internal`
        /// Any other failure the library reports; the message says what.
        case failed
        /// A reader that takes one transducer got bytes holding more, or a
        /// compiler that returns one made several.
        case transducerCount = "transducer-count"
        /// A source that did not compile, or an xfst script that stopped on
        /// a command that failed. `diagnostics` says why.
        case compileFailed = "compile-failed"
    }

    public let code: Code
    /// What went wrong, in the library's words.
    public let message: String
    /// Every diagnostic the compile behind the error made, for an error from
    /// a compiler; empty for any other.
    public let diagnostics: [Diagnostic]

    package init(code: Code, message: String, diagnostics: [Diagnostic] = []) {
        self.code = code
        self.message = message
        self.diagnostics = diagnostics
    }

    /// `error` from the glue. A code the glue sends that this version does
    /// not know is reported as `internal`, naming it.
    package init(_ error: GlueError, sources: [String]) {
        switch error {
        case let .Failed(code, message, diagnostics):
            let known = Code(rawValue: code)
            self.init(
                code: known ?? .internal,
                message: known == nil ? "\(message) (the library sent the unknown code \(code))" : message,
                diagnostics: diagnostics.map { Diagnostic($0, sources: sources) }
            )
        }
    }

    public var description: String { message }
    public var errorDescription: String? { message }
}

// [spec:hfst:req:swift.errors]
/// Something a compile said about its source: an error, a warning or a note.
public struct Diagnostic: Sendable, Hashable {
    public enum Severity: String, Sendable, Hashable {
        case error, warning, info
    }

    /// A piece of a compile's source a diagnostic points at.
    public struct Label: Sendable, Hashable {
        /// Where the piece is in the source string the caller gave the
        /// compile; `nil` for a file the compile read.
        public let range: Range<String.Index>?
        /// Where the piece is in the source's UTF-8 bytes.
        public let utf8Range: Range<Int>
        public let message: String
    }

    public let severity: Severity
    public let message: String
    /// The name of the source it is about, as the compile was given it.
    public let sourceName: String
    /// Where its primary label is in the source string the caller gave the
    /// compile; `nil` when it points at no piece of that string.
    public let range: Range<String.Index>?
    /// Where its primary label is in the source's UTF-8 bytes; `nil` when it
    /// points at no piece of the source.
    public let utf8Range: Range<Int>?
    /// Every label, the primary one first.
    public let labels: [Label]
    public let notes: [String]
    public let help: String?
    /// The report the hfst tools print for it, without colour.
    public let rendered: String

    package init(_ diagnostic: HFSTGlue.Diagnostic, sources: [String]) {
        let source = diagnostic.source.flatMap { Int($0) < sources.count ? sources[Int($0)] : nil }
        let place = { (span: Span) -> (Range<String.Index>?, Range<Int>) in
            let bytes = Int(span.start)..<Int(max(span.start, span.end))
            return (source.flatMap { $0.range(ofUTF8: bytes) }, bytes)
        }
        severity = switch diagnostic.severity {
        case .error: .error
        case .warning: .warning
        case .info: .info
        }
        message = diagnostic.message
        sourceName = diagnostic.sourceName
        if let span = diagnostic.span {
            (range, utf8Range) = place(span)
        } else {
            (range, utf8Range) = (nil, nil)
        }
        labels = diagnostic.labels.map { label in
            let (range, bytes) = place(label.span)
            return Label(range: range, utf8Range: bytes, message: label.message)
        }
        notes = diagnostic.notes
        help = diagnostic.help
        rendered = diagnostic.rendered
    }
}

extension String {
    /// The range of the characters at `bytes` of this string's UTF-8, or
    /// `nil` when the bytes run past its end.
    package func range(ofUTF8 bytes: Range<Int>) -> Range<String.Index>? {
        guard bytes.lowerBound >= 0, bytes.upperBound <= utf8.count else { return nil }
        let start = utf8.index(utf8.startIndex, offsetBy: bytes.lowerBound)
        let end = utf8.index(start, offsetBy: bytes.count)
        return start..<end
    }
}
