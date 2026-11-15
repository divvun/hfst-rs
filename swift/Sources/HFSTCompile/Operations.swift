import Foundation
internal import HFSTCompileGlue

// [spec:hfst:req:swift.operations]
/// A side of a transducer, as `hfst project --project` names it.
public enum Tape: String, Sendable, Hashable, CaseIterable {
    case input, output
}

// [spec:hfst:req:swift.operations]
/// Where `pushWeights` and `pushLabels` push, as `--push` names it.
public enum PushDirection: String, Sendable, Hashable, CaseIterable {
    case initial, final
}

// [spec:hfst:req:swift.operations]
/// Which affixes `affixGuessify` guesses by.
public enum GuessDirection: String, Sendable, Hashable, CaseIterable {
    case prefix, suffix
}

// [spec:hfst:req:swift.operations]
/// The side `expandEquivalences` expands, as `--level` names it.
public enum EquivalenceLevel: String, Sendable, Hashable, CaseIterable {
    case first, second, both
}

// [spec:hfst:req:swift.operations]
/// The functions `reweight` applies to a weight, as `hfst reweight
/// --function` names them.
public enum ReweightFunction: String, Sendable, Hashable, CaseIterable {
    case cos, sin, tan, acos, asin, atan, cosh, sinh, tanh, exp, log, log10, sqrt, floor, ceil
}

// [spec:hfst:req:swift.operations]
/// A bound of `repeat`: a number of times, or none, as `hfst repeat` reads
/// `inf`. An integer literal is a number of times.
public enum RepeatCount: Sendable, Hashable, ExpressibleByIntegerLiteral {
    case times(Int)
    case unbounded

    public init(integerLiteral value: Int) {
        self = .times(value)
    }

    var glue: Int64? {
        guard case let .times(count) = self else { return nil }
        return Int64(count)
    }
}

// [spec:hfst:req:swift.operations]
// [spec:hfst:req:swift.threads]
// Each operation changes the transducer, as the hfst tool it is named after
// changes the transducer it reads, and returns nothing. One that reads a
// second transducer locks both in an order the library fixes, and reads the
// transducer itself, as in t.compose(t), as it was before the change.
extension Transducer {
    /// Converts the transducer to `type`, in place, as `hfst fst2fst`
    /// converts it. `quick` packs optimized-lookup tables less tightly, as
    /// `--quick`.
    public func convert(to type: TransducerType, quick: Bool = false) throws(HFSTError) {
        try calling { try glue.convert(format: type.rawValue, quick: quick) }
    }

    /// Determinizes the transducer, as `hfst determinize`. `encodeWeights`
    /// treats each weight as part of its arc's label.
    public func determinize(encodeWeights: Bool = false) throws(HFSTError) {
        try calling { try glue.determinize(encodeWeights: encodeWeights) }
    }

    /// Minimizes the transducer, as `hfst minimize`.
    public func minimize(encodeWeights: Bool = false) throws(HFSTError) {
        try calling { try glue.minimize(encodeWeights: encodeWeights) }
    }

    /// Swaps the input and output sides, as `hfst invert`.
    public func invert() throws(HFSTError) {
        try calling { try glue.invert() }
    }

    /// Reverses every path, as `hfst reverse`.
    public func reverse() throws(HFSTError) {
        try calling { try glue.reverse() }
    }

    /// Removes the epsilon arcs, as `hfst remove-epsilons`.
    public func removeEpsilons() throws(HFSTError) {
        try calling { try glue.removeEpsilons() }
    }

    /// Drops the symbols no arc uses from the alphabet, as `hfst
    /// prune-alphabet`; `force` prunes even when an arc uses the unknown or
    /// identity symbol.
    public func pruneAlphabet(force: Bool = false) throws(HFSTError) {
        try calling { try glue.pruneAlphabet(force: force) }
    }

    /// Makes the transducer the identity on one side's language, as `hfst
    /// project`.
    public func project(_ tape: Tape = .output) throws(HFSTError) {
        try calling { try glue.project(project: tape.rawValue) }
    }

    /// Pushes the weights towards the initial state or the final states, as
    /// `hfst push-weights`.
    public func pushWeights(to direction: PushDirection = .final) throws(HFSTError) {
        try calling { try glue.pushWeights(push: direction.rawValue) }
    }

    /// Pushes the labels towards the initial state or the final states, as
    /// `hfst push-labels`.
    public func pushLabels(to direction: PushDirection = .final) throws(HFSTError) {
        try calling { try glue.pushLabels(push: direction.rawValue) }
    }

    /// Realigns the symbols of each path, as `hfst realign`.
    public func realign() throws(HFSTError) {
        try calling { try glue.realign() }
    }

    /// Makes the transducer repeat at least `from` and at most `to` times,
    /// as `hfst repeat`.
    public func `repeat`(from: RepeatCount = 0, to: RepeatCount = .unbounded) throws(HFSTError) {
        try calling { try glue.repeat(from: from.glue, to: to.glue) }
    }

    /// Inserts `symbolPair`, `IN:OUT` with `@0@` for the epsilon, freely
    /// anywhere along each path, as `hfst insert-freely`. `harmonise`
    /// expands the identity and unknown symbols by the pair's symbols.
    public func insertFreely(_ symbolPair: String, harmonise: Bool = false) throws(HFSTError) {
        try calling { try glue.insertFreely(symbolPair: symbolPair, harmonise: harmonise) }
    }

    /// Compiles the flag diacritics of the feature `flag`, or of every
    /// feature, into the transducer's paths, as `hfst eliminate-flags`.
    public func eliminateFlags(_ flag: String? = nil) throws(HFSTError) {
        try calling { try glue.eliminateFlags(flag: flag) }
    }

    /// Prepares the transducer for conversion to optimized lookup, as `hfst
    /// preprocess-for-optimized-lookup-format`.
    public func preprocessForOptimizedLookupFormat() throws(HFSTError) {
        try calling { try glue.preprocessForOptimizedLookupFormat() }
    }

    /// Makes the transducer a guesser of the analyses of unknown words by
    /// their affixes, as `hfst affix-guessify`, each guessed character
    /// costing `weight`.
    public func affixGuessify(direction: GuessDirection = .suffix, weight: Float = 1) throws(HFSTError) {
        try calling { try glue.affixGuessify(direction: direction.rawValue, weight: weight) }
    }

    /// Lets each symbol stand for the symbols it is equivalent to, as `hfst
    /// expand-equivalences`: `from` for `to`, or each line of `tsv`,
    /// `FROM<TAB>TO<TAB>TO...`, on the `level` side.
    public func expandEquivalences(
        from: String? = nil,
        to: String? = nil,
        tsv: String? = nil,
        level: EquivalenceLevel = .first
    ) throws(HFSTError) {
        try calling { try glue.expandEquivalences(from: from, to: to, tsv: tsv, level: level.rawValue) }
    }

    /// Removes every arc carrying `symbol`, or any symbol `tsv` lists one a
    /// line, as `hfst kill-paths`.
    public func killPaths(symbol: String? = nil, tsv: String? = nil) throws(HFSTError) {
        try calling { try glue.killPaths(symbol: symbol, tsv: tsv) }
    }

    /// Changes the weights, as `hfst reweight`: each weight in bounds
    /// becomes `function(weight * multiplier + addition)`, on the arcs and
    /// final states `symbol`, `inputSymbol` and `outputSymbol` select, or by
    /// the rules of `tsv`, `SYMBOL<TAB>WEIGHT` a line. `upperBound` `nil`
    /// sets no upper bound.
    public func reweight(
        addition: Float = 0,
        multiplier: Float = 1,
        function: ReweightFunction? = nil,
        lowerBound: Float = 0,
        upperBound: Float? = nil,
        inputSymbol: String? = nil,
        outputSymbol: String? = nil,
        symbol: String? = nil,
        endStatesOnly: Bool = false,
        arcsOnly: Bool = false,
        tsv: String? = nil
    ) throws(HFSTError) {
        try calling {
            try glue.reweight(
                addition: addition,
                multiplier: multiplier,
                function: function?.rawValue,
                lowerBound: lowerBound,
                upperBound: upperBound,
                inputSymbol: inputSymbol,
                outputSymbol: outputSymbol,
                symbol: symbol,
                endStatesOnly: endStatesOnly,
                arcsOnly: arcsOnly,
                tsv: tsv
            )
        }
    }

    /// Sets the transducer's name, as `hfst name --name`, cut to
    /// `truncateLength` bytes unless that is 0.
    public func setName(_ name: String, truncateLength: Int = 0) throws(HFSTError) {
        try calling { try glue.setName(name: name, truncateLength: Int64(truncateLength)) }
    }

    /// Adds or replaces each property of `properties`, as `hfst
    /// edit-metadata --add NAME=VALUE`, each value cut to `truncateLength`
    /// bytes unless that is 0.
    public func editMetadata(adding properties: [String: String], truncateLength: Int = 0) throws(HFSTError) {
        try calling { try glue.editMetadata(add: properties, truncateLength: Int64(truncateLength)) }
    }

    /// Makes the transducer, a morphological analyser, a guesser of the
    /// analyses of unknown words, as `hfst guessify`: weighted optimized
    /// lookup that `guess` takes. Returns the model form generator the tool
    /// writes after the guesser, or `nil` with `doNotCompileGenerator`.
    @discardableResult
    public func guessify(
        defaultPenalty: Float = 1,
        doNotCompileGenerator: Bool = false
    ) throws(HFSTError) -> Transducer? {
        try calling {
            try glue.guessify(defaultPenalty: defaultPenalty, doNotCompileGenerator: doNotCompileGenerator)
        }.map(Transducer.init(glue:))
    }

    /// Replaces labels, as `hfst substitute`: `fromLabel` by `toLabel` or by
    /// `toTransducer`, or each `FROM<TAB>TO` line of `fromFile`, the text of
    /// a relabel file, in turn with `inOrder`. A label is a symbol or a colon
    /// pair, `@0@` for the epsilon. `fileName` is the name of the file the
    /// tool would read `toTransducer` or `fromFile` from, which names the
    /// result. `compose` composes symbol-for-symbol substitutions in rather
    /// than rewriting arcs.
    public func substitute(
        fromLabel: String? = nil,
        toLabel: String? = nil,
        toTransducer: Transducer? = nil,
        fromFile: String? = nil,
        fileName: String = "",
        inOrder: Bool = false,
        doNotConvert: Bool = false,
        compose: Bool = false
    ) throws(HFSTError) {
        try calling {
            try glue.substitute(
                fromLabel: fromLabel,
                toLabel: toLabel,
                toTransducer: toTransducer?.glue,
                fromFile: fromFile,
                fileName: fileName,
                inOrder: inOrder,
                doNotConvert: doNotConvert,
                compose: compose
            )
        }
    }

    /// Composes `other` after the transducer, as `hfst compose`. Flag
    /// diacritics are harmonized unless `doNotHarmonize`; with
    /// `xeroxComposition` they are ordinary symbols, and with
    /// `flagIsEpsilon` a one-sided one is an epsilon. Transducers of two
    /// algebra types are converted to one as the tool converts them, unless
    /// `doNotConvert`.
    public func compose(
        _ other: Transducer,
        doNotHarmonize: Bool = false,
        harmonizeFlags: Bool = false,
        xeroxComposition: Bool = false,
        flagIsEpsilon: Bool = false,
        doNotConvert: Bool = false
    ) throws(HFSTError) {
        try calling {
            try glue.compose(
                other: other.glue,
                doNotHarmonize: doNotHarmonize,
                harmonizeFlags: harmonizeFlags,
                xeroxComposition: xeroxComposition,
                flagIsEpsilon: flagIsEpsilon,
                doNotConvert: doNotConvert
            )
        }
    }

    /// Concatenates `other` after the transducer, as `hfst concatenate`.
    public func concatenate(
        _ other: Transducer,
        doNotHarmonize: Bool = false,
        harmonizeFlags: Bool = false,
        doNotConvert: Bool = false
    ) throws(HFSTError) {
        try calling {
            try glue.concatenate(
                other: other.glue,
                doNotHarmonize: doNotHarmonize,
                harmonizeFlags: harmonizeFlags,
                doNotConvert: doNotConvert
            )
        }
    }

    /// Intersects the transducer with `other`, as `hfst conjunct`.
    public func conjunct(
        _ other: Transducer,
        doNotHarmonize: Bool = false,
        harmonizeFlags: Bool = false,
        doNotConvert: Bool = false
    ) throws(HFSTError) {
        try calling {
            try glue.conjunct(
                other: other.glue,
                doNotHarmonize: doNotHarmonize,
                harmonizeFlags: harmonizeFlags,
                doNotConvert: doNotConvert
            )
        }
    }

    /// Subtracts `other` from the transducer, as `hfst subtract`.
    public func subtract(
        _ other: Transducer,
        doNotHarmonize: Bool = false,
        harmonizeFlags: Bool = false,
        doNotConvert: Bool = false
    ) throws(HFSTError) {
        try calling {
            try glue.subtract(
                other: other.glue,
                doNotHarmonize: doNotHarmonize,
                harmonizeFlags: harmonizeFlags,
                doNotConvert: doNotConvert
            )
        }
    }

    /// Unites the transducer with `other`, as `hfst disjunct`.
    public func disjunct(
        _ other: Transducer,
        doNotHarmonize: Bool = false,
        doNotConvert: Bool = false
    ) throws(HFSTError) {
        try calling {
            try glue.disjunct(other: other.glue, doNotHarmonize: doNotHarmonize, doNotConvert: doNotConvert)
        }
    }

    /// Unites the transducer with the paths of `other` whose input it does
    /// not read, as `hfst priority-disjunct`.
    public func priorityDisjunct(_ other: Transducer, doNotConvert: Bool = false) throws(HFSTError) {
        try calling { try glue.priorityDisjunct(other: other.glue, doNotConvert: doNotConvert) }
    }

    /// Interleaves its paths with those of `other` in every order that keeps
    /// each path's own, as `hfst shuffle`.
    public func shuffle(_ other: Transducer, doNotConvert: Bool = false) throws(HFSTError) {
        try calling { try glue.shuffle(other: other.glue, doNotConvert: doNotConvert) }
    }

    /// Composes the transducer, a lexicon, with the intersection of `rules`,
    /// as `hfst compose-intersect`. The result is named after the lexicon,
    /// or `lexiconName` when it has no name, and `rulesName`, as the tool
    /// names it after its files.
    public func composeIntersect(
        _ rules: [Transducer],
        invert: Bool = false,
        fast: Bool = false,
        encodeWeights: Bool = false,
        harmonize: Bool = false,
        lexiconName: String = "",
        rulesName: String = "",
        doNotConvert: Bool = false
    ) throws(HFSTError) {
        try calling {
            try glue.composeIntersect(
                rules: rules.map(\.glue),
                invert: invert,
                fast: fast,
                encodeWeights: encodeWeights,
                harmonize: harmonize,
                lexiconName: lexiconName,
                rulesName: rulesName,
                doNotConvert: doNotConvert
            )
        }
    }
}
