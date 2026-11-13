import type { CompiledGlue, CompileGlue, FilesGlue, OptionsGlue } from "./compile-glue.ts";
import { type Transducer, wrapTransducer } from "./compile-transducer.ts";
import { strings, type TransducerType } from "./compile-transducer-base.ts";
import type { Core } from "./core.ts";
import { type Diagnostic, HfstError } from "./errors.ts";

// [spec:hfst:req:javascript.compile-files]
/**
 * The files a compile may read, and no others: a record or a `Map` from
 * names to their text or bytes, or a function that takes a name and returns
 * the file's text or bytes, or `undefined` for a file it does not have. The
 * function is called while the compile runs, with each name as the compiler
 * looks it up, and a file it gives is kept, so it is not asked for that file
 * again. A name the files do not supply fails the compile, naming the file.
 * Left out, a compile reads no files.
 */
export type CompileFiles =
  | Readonly<Record<string, string | Uint8Array>>
  | ReadonlyMap<string, string | Uint8Array>
  | ((name: string) => string | Uint8Array | undefined);

/** What every compiler takes beside its source and its tool's options. */
export interface SourceOptions {
  /**
   * The file name the tool would read the source from, which diagnostics
   * give it and some results are named after. Default `<stdin>`, or
   * `<twolc>` for twolc, as the tools name their standard input.
   */
  name?: string;
  /** The files the source names. */
  files?: CompileFiles;
}

/** `hfst regexp2fst`'s options, which `regex` takes. */
export interface RegexOptions extends SourceOptions {
  /** The type to compile to. Default `openfst-tropical`. */
  format?: TransducerType;
  /** Each expression ends with `;`, rather than a line. */
  semicolon?: boolean;
  /** One transducer, the union of the expressions. */
  disjunct?: boolean;
  /** Does not expand `?` by the other operand's symbols. */
  doNotHarmonize?: boolean;
  harmonizeFlags?: boolean;
  /** Determinizes each result rather than minimizing it. */
  doNotMinimize?: boolean;
  encodeWeights?: boolean;
  /** Flag diacritics are ordinary symbols in composition. */
  xeroxComposition?: boolean;
  /** A one-sided flag diacritic is an epsilon in composition, as `--xfst flag-is-epsilon`. */
  flagIsEpsilon?: boolean;
  /** Compiles each expression, one parallel optional replace rule, as one rewrite pass. */
  replacePass?: boolean;
  /** Says what the compile does, at the info level of the logger, under the target `hfst`. */
  verbose?: boolean;
}

/** `hfst lexc`'s options, which `compileLexc` takes. */
export interface LexcOptions extends Omit<SourceOptions, "name"> {
  /**
   * The file names the tool would read the sources from, one for each,
   * which diagnostics and the result's name give them. Default `<stdin>`
   * for each.
   */
  names?: readonly string[];
  format?: TransducerType;
  /** Aligns the characters of a string pair one to one. */
  alignStrings?: boolean;
  /** Joins the lexicons with flag diacritics rather than composing them. */
  withFlags?: boolean;
  minimizeFlags?: boolean;
  renameFlags?: boolean;
  /** Composes as Xerox tools do. Default `true`, as the tool's. */
  xeroxComposition?: boolean;
  flagIsEpsilon?: boolean;
  encodeWeights?: boolean;
  /** Does not read the source's text as Unicode characters. */
  splitCharacters?: boolean;
  /** A warning fails the compile, as `-Werror`. */
  werror?: boolean;
  warnOneSidedFlags?: boolean;
  warnUnusedLexicons?: boolean;
  warnRepeatedLexicons?: boolean;
  warnMissingLexicons?: boolean;
  warnMissingAlphabets?: boolean;
  warnUnnecessaryEscapes?: boolean;
  /** No warnings or notes. */
  quiet?: boolean;
  /** Progress as well. */
  verbose?: boolean;
}

/** `hfst twolc`'s options, which `compileTwolc` takes. */
export interface TwolcOptions extends SourceOptions {
  format?: "openfst-tropical" | "foma";
  /** Resolves left-arrow conflicts. */
  resolve?: boolean;
  dontResolveRight?: boolean;
  quiet?: boolean;
  verbose?: boolean;
}

/** `hfst xfst`'s options, which `compileXfst` takes. */
export interface XfstOptions extends SourceOptions {
  /** The type the script works in. */
  format?: "openfst-tropical" | "foma";
  /** The standard input the script's commands read. Default `""`. */
  input?: string;
  /** Prints weights where words are printed. */
  printWeight?: boolean;
  /** Reads and writes only in the working directory, and runs no shell command. */
  restrictedMode?: boolean;
  /** Prints no command output. */
  silent?: boolean;
}

/** `hfst pmatch2fst`'s options, which `compilePmatch` takes. */
export interface PmatchCompileOptions extends SourceOptions {
  /** Compiles every RTN into the network that uses it. */
  flatten?: boolean;
  /** Includes the cosine distances `Like()` reads. */
  cosineDistances?: boolean;
  verbose?: boolean;
}

/** `hfst strings2fst`'s options, which `compileStrings` takes. */
export interface StringsOptions extends SourceOptions {
  format?: TransducerType;
  /** One transducer, the union of the lines. */
  disjunctStrings?: boolean;
  /** Divides each weight by the sum of all of them, with `disjunctStrings`. */
  norm?: boolean;
  /** Takes the negative natural logarithm of each weight. */
  log?: boolean;
  /** Takes the negative base 10 logarithm of each weight. */
  log10?: boolean;
  /** The lines are pair strings, `a:b c`, rather than string pairs, `ac:bc`. */
  pairstrings?: boolean;
  /** Symbols are separated by spaces. */
  hasSpaces?: boolean;
  /** What the lines spell the epsilon as. Default `@0@`. */
  epsilon?: string;
  /** The strings read as one symbol each, the lines of the tool's `--multichar-symbols` file. */
  multicharSymbols?: readonly string[];
  /** Warns of each negative weight, as `-Wnegative-weights`. Default `true`. */
  warnNegativeWeights?: boolean;
  werror?: boolean;
}

/** `hfst txt2fst`'s options for AT&T text, which `parseAtt` takes. */
export interface AttOptions extends SourceOptions {
  format?: TransducerType;
  /** What the text spells the epsilon as. Default `@0@`. */
  epsilon?: string;
  /** One transducer, the union of the text's. */
  disjunct?: boolean;
  checkNegativeEpsilonCycles?: boolean;
  /** Default `true`. */
  warnNegativeWeights?: boolean;
}

/** `hfst txt2fst --prolog`'s options, which `parseProlog` takes. */
export interface PrologOptions extends SourceOptions {
  format?: TransducerType;
  checkNegativeEpsilonCycles?: boolean;
}

// [spec:hfst:req:javascript.compile-errors]
/** A transducer a compiler made, and the warnings it gave on the way. */
export interface Compiled {
  transducer: Transducer;
  /**
   * Each warning the compile gave, and each error it made without failing.
   * What it says at the info level goes to the logger instead.
   */
  warnings: Diagnostic[];
}

/** The rule transducers `compileTwolc` made, and its warnings. */
export interface CompiledRules {
  transducers: Transducer[];
  warnings: Diagnostic[];
}

/** The archive `compilePmatch` made, which `readTokeniser` reads, and its warnings. */
export interface CompiledArchive {
  archive: Uint8Array;
  warnings: Diagnostic[];
}

/** What an xfst script left behind. */
export interface XfstResult {
  /** What it printed: the tool's standard output. */
  output: string;
  /** Its stack, the bottom first. */
  stack: Transducer[];
  /** The networks it defined, by name. */
  definitions: Record<string, Transducer>;
  /** Each file it wrote, by name, holding what the script left in it. */
  written: Record<string, Uint8Array>;
  /** Each warning, and each error of a command that failed without stopping the script. */
  warnings: Diagnostic[];
}

export interface Compilers {
  /**
   * The transducer `hfst regexp2fst` writes of the regular expression
   * `source`. With `disjunct` a source of several expressions, one a line
   * or with `semicolon` each ending in `;`, compiles to their union; without
   * it, a source of more than one throws `transducer-count`. A source that
   * does not compile throws `compile-failed`, with every diagnostic it made.
   */
  regex(source: string, options?: RegexOptions): Compiled;
  /**
   * The transducer `hfst lexc` writes of the lexc `sources`, compiled
   * together and named after the first.
   */
  compileLexc(sources: readonly string[], options?: LexcOptions): Compiled;
  /** The rule transducers `hfst twolc` writes of the twolc grammar `source`. */
  compileTwolc(source: string, options?: TwolcOptions): CompiledRules;
  /**
   * Runs the xfst script `source` as `hfst xfst -F` runs it: what it
   * printed, its stack, its defined networks and the files it wrote. Each
   * file it reads comes from `files`, and each it writes comes back in
   * `written`. A command that fails is a warning, and the script throws
   * `compile-failed` only when it stops on one.
   */
  compileXfst(source: string, options?: XfstOptions): XfstResult;
  /**
   * The archive `hfst pmatch2fst` writes of the pmatch script `source`. A
   * relative `@bin"FILE"` or `@txt"FILE"` is found beside `name`, as the
   * tool finds one beside its input.
   */
  compilePmatch(source: string, options?: PmatchCompileOptions): CompiledArchive;
  /**
   * The transducer `hfst strings2fst` writes of the strings `source`, one a
   * line, each with an optional tab and weight. With `disjunctStrings` the
   * lines compile to their union; without it, a source of more than one
   * line throws `transducer-count`.
   */
  compileStrings(source: string, options?: StringsOptions): Compiled;
  /**
   * The transducer `hfst txt2fst` writes of the AT&T text `source`, named
   * after `name`. With `disjunct` a text of several transducers, separated
   * by `--` lines, reads as their union; without it, a text of more than one
   * throws `transducer-count`.
   */
  parseAtt(source: string, options?: AttOptions): Compiled;
  /**
   * The transducer `hfst txt2fst --prolog` writes of the Prolog text
   * `source`, named after `name`. A text of more than one network throws
   * `transducer-count`.
   */
  parseProlog(source: string, options?: PrologOptions): Compiled;
}

function invalid(message: string): HfstError {
  return new HfstError("invalid-argument", message);
}

function isFile(content: unknown): content is string | Uint8Array {
  return typeof content === "string" || content instanceof Uint8Array;
}

// [spec:hfst:req:javascript.compile-files]
/** The files as the module takes them: a function as it is, a record or a Map as name and content pairs. */
function filesGlue(call: string, files: unknown): FilesGlue {
  if (files === undefined) return undefined;
  if (typeof files === "function") return (name) => Reflect.apply(files, undefined, [name]);
  if (typeof files !== "object" || files === null || Array.isArray(files)) {
    throw invalid(`${call}'s files are a record or a Map from names to text or bytes, or a function`);
  }
  const entries: [unknown, unknown][] = files instanceof Map ? [...files.entries()] : Object.entries(files);
  return entries.map(([name, content]) => {
    if (typeof name !== "string") throw invalid(`a file's name is a string, not ${typeof name}`);
    if (!isFile(content)) throw invalid(`the file ${name} is a string or a Uint8Array, not ${typeof content}`);
    return [name, content];
  });
}

/** `options`, refused unless an object or left out. */
function optionsOf(call: string, options: unknown): Record<string, unknown> {
  if (options === undefined) return {};
  if (typeof options !== "object" || options === null || Array.isArray(options)) {
    throw invalid(`${call} takes its options as an object`);
  }
  return { ...options };
}

function checkSource(call: string, source: unknown): asserts source is string {
  if (typeof source !== "string") throw invalid(`${call} takes the source as a string, not ${typeof source}`);
}

interface Split {
  name: string;
  files: FilesGlue;
  /** The tool's options, which the module reads as an object. */
  options: OptionsGlue;
}

/** Splits `options` into the name and files, which the module takes as arguments, and the tool's options. */
function split(call: string, options: Record<string, unknown>, defaultName = "<stdin>"): Split {
  const { name = defaultName, files, ...rest } = options;
  if (typeof name !== "string") throw invalid(`${call} takes the source's name as a string`);
  return { name, files: filesGlue(call, files), options: rest };
}

// [spec:hfst:req:javascript.compile-module]
// [spec:hfst:req:javascript.compile-files]
// [spec:hfst:req:javascript.compile-errors]
// [spec:hfst:req:javascript.compile-parity]
// The source, the files and the options reach the module as given, and what
// a compile makes comes back as the module wrote it, so a compile gives what
// divvun-hfst's gives: the tool's bytes.
/** The {@link Compilers} over the compile module instance `core`. */
export function compilers(core: Core<CompileGlue>): Compilers {
  const compiled = ({ transducer, warnings }: CompiledGlue): Compiled => ({
    transducer: wrapTransducer(core, transducer),
    warnings,
  });
  /** A compiler of one source, which `run` hands to the module. */
  const compiler =
    <O, R>(call: string, run: (glue: CompileGlue, source: string, split: Split) => R, defaultName?: string) =>
    (source: string, options?: O): R =>
      core.call((glue) => {
        checkSource(call, source);
        return run(glue, source, split(call, optionsOf(call, options), defaultName));
      });
  return {
    regex: compiler("regex", (glue, source, { name, files, options }) =>
      compiled(glue.regex(source, name, files, options)),
    ),
    compileLexc: (sources, options) =>
      core.call((glue) => {
        const texts = strings("compileLexc", sources);
        const { names, ...rest } = optionsOf("compileLexc", options);
        const named = names === undefined ? texts.map(() => "<stdin>") : strings("compileLexc", names);
        const { files, options: tool } = split("compileLexc", rest);
        return compiled(glue.compileLexc(texts, named, files, tool));
      }),
    compileTwolc: compiler(
      "compileTwolc",
      (glue, source, { name, files, options }) => {
        const { transducers, warnings } = glue.compileTwolc(source, name, files, options);
        return { transducers: transducers.map((t) => wrapTransducer(core, t)), warnings };
      },
      "<twolc>",
    ),
    compileXfst: (source, options) =>
      core.call((glue) => {
        checkSource("compileXfst", source);
        const { input = "", ...rest } = optionsOf("compileXfst", options);
        if (typeof input !== "string") throw invalid("compileXfst takes the script's input as a string");
        const { name, files, options: tool } = split("compileXfst", rest);
        const ran = glue.compileXfst(source, input, name, files, tool);
        return {
          output: ran.output,
          stack: ran.stack.map((t) => wrapTransducer(core, t)),
          definitions: Object.fromEntries(
            Object.entries(ran.definitions).map(([defined, t]) => [defined, wrapTransducer(core, t)]),
          ),
          written: ran.written,
          warnings: ran.warnings,
        };
      }),
    compilePmatch: compiler("compilePmatch", (glue, source, { name, files, options }) =>
      glue.compilePmatch(source, name, files, options),
    ),
    compileStrings: compiler("compileStrings", (glue, source, { name, files, options }) =>
      compiled(glue.compileText("strings", source, name, files, options)),
    ),
    parseAtt: compiler("parseAtt", (glue, source, { name, files, options }) =>
      compiled(glue.compileText("att", source, name, files, options)),
    ),
    parseProlog: compiler("parseProlog", (glue, source, { name, files, options }) =>
      compiled(glue.compileText("prolog", source, name, files, options)),
    ),
  };
}
