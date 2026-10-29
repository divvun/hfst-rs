import { type Diagnostic, HfstError } from "../shared/errors.ts";
import { core } from "./binding.ts";
import type { CompiledGlue, CompileFilesGlue } from "./glue.ts";
import * as glue from "./pkg/hfst_wasm.js";
import { type Transducer, type TransducerType, wrapTransducer } from "./transducer.ts";

/** What every compile takes. */
export interface CompileOptions {
  /**
   * The files the source names, under the names it gives them: bytes, or
   * text, which is read as UTF-8. Every file a compile reads comes from
   * here, including the files those files name. A name the map lacks fails
   * the compile with an error that names the file.
   */
  files?: Readonly<Record<string, Uint8Array | string>>;
  /**
   * The name diagnostics show for the source, where a command-line tool
   * shows the file name. `hfst lexc` also names what it makes after it. The
   * default is `<regex>`, `<lexc>`, `<twolc>` or `<pmatch>`.
   */
  sourceName?: string;
}

/** The `hfst regexp2fst` options that change what it makes or reports. */
export interface RegexOptions extends CompileOptions {
  /**
   * The type to make, as `-f`. The default is `openfst-tropical`; the source
   * compiles at that type and converts to any other, as the tool converts it.
   */
  type?: TransducerType;
  /**
   * Read expressions that each end in `;` and may span lines, as `-S`. By
   * default each line holds one expression, as `-l`.
   */
  semicolon?: boolean;
  /**
   * Make the union of every expression, as `-j`. Without it, a source that
   * holds several expressions throws `transducer-count`.
   */
  disjunct?: boolean;
  /**
   * Expand `?` against the other operand's alphabet in binary operations.
   * The default is true; false is `-H`.
   */
  harmonize?: boolean;
  /** Harmonise flag diacritics too, as `-F`. */
  harmonizeFlags?: boolean;
  /** Encode weights into labels while minimising, as `-E`. */
  encodeWeights?: boolean;
  /** Minimise the result. The default is true; false only determinises it, as `-M`. */
  minimize?: boolean;
  /** Compose with flag diacritics as ordinary symbols, as `-x true`. */
  xeroxComposition?: boolean;
  /** Compose with flag diacritics as epsilons, as `-X flag-is-epsilon`. */
  flagIsEpsilon?: boolean;
  /**
   * Compile each expression, which must be one parallel optional replace
   * rule P, as one rewrite pass: the relation of `?* P ?*` as a small
   * automaton that is not determinised, as `--replace-pass`.
   */
  replacePass?: boolean;
  /**
   * Report the warnings `-v` adds, such as for a symbol spelled like one of
   * HFST's special symbols.
   */
  verbose?: boolean;
}

/**
 * A `-W` switch of `hfst lexc`. `all` turns every warning on, `error` makes
 * every warning that is on an error, and the others turn one warning on, or
 * off with `no-` before its name.
 */
export type LexcWarningSwitch =
  | "all"
  | "error"
  | "one-sided-flags"
  | "no-one-sided-flags"
  | "missing-lexicons"
  | "no-missing-lexicons"
  | "unused-lexicons"
  | "no-unused-lexicons"
  | "repeated-lexicons"
  | "no-repeated-lexicons"
  | "missing-alphabets"
  | "no-missing-alphabets"
  | "unnecessary-escapes"
  | "no-unnecessary-escapes";

/** The `hfst lexc` options that change what it makes or reports. */
export interface LexcOptions extends CompileOptions {
  /**
   * The type to make, as `-f`. The default is `openfst-tropical`; the source
   * compiles at that type and converts to any other, as the tool converts it.
   */
  type?: TransducerType;
  /** Align the characters of input and output strings, as `-A`. */
  alignStrings?: boolean;
  /** Hyperminimise the result with flag diacritics, as `-F`. */
  withFlags?: boolean;
  /** With `withFlags`, use as few flags as can be, as `-M`. */
  minimizeFlags?: boolean;
  /** With `withFlags` and `minimizeFlags`, rename the flags, as `-R`. */
  renameFlags?: boolean;
  /** Encode weights into labels while minimising, as `-E`. */
  encodeWeights?: boolean;
  /**
   * Compose with flag diacritics as ordinary symbols, as Xerox's tools do.
   * The default is true; false is `-x false`.
   */
  xeroxComposition?: boolean;
  /** Compose with flag diacritics as epsilons, as `-X flag-is-epsilon`. */
  flagIsEpsilon?: boolean;
  /** The `-W` switches, applied in order. Every warning starts off. */
  warnings?: readonly LexcWarningSwitch[];
  /** Read entries a code point at a time rather than a grapheme at a time, as `-9`. */
  splitCharacters?: boolean;
  /**
   * Report less, as `-q`: the embedded regular expression compiler leaves out
   * the warnings it gives only in its verbose mode.
   */
  quiet?: boolean;
  /**
   * Report more, as `-v`: also warn about a multicharacter symbol a regular
   * expression uses that no `Multichar_Symbols` declares. `quiet` wins.
   */
  verbose?: boolean;
}

/** The `hfst twolc` options that change what it makes or reports. */
export interface TwolcOptions extends CompileOptions {
  /** The type the rules compile at, as `-f`: `openfst-tropical`, the default, or `foma`. */
  type?: "openfst-tropical" | "foma";
  /** Resolve left-arrow conflicts, as `-R`. */
  resolveLeft?: boolean;
  /** Resolve right-arrow conflicts. The default is true; false is `-D`. */
  resolveRight?: boolean;
  /** Leave out warnings and conflict reports, as `-q`. Errors are still reported. */
  quiet?: boolean;
}

/** The `hfst pmatch2fst` options that change what it makes. */
export interface PmatchOptions extends CompileOptions {
  /** Compile every RTN into the transducers that insert it, as `--flatten`. */
  flatten?: boolean;
  /** Include cosine distances when compiling `Like()` operations, as `--cosine-distances`. */
  includeCosineDistances?: boolean;
}

/** One transducer a compile made, and its warnings and other diagnostics. */
export interface CompiledTransducer {
  transducer: Transducer;
  diagnostics: Diagnostic[];
}

/** The transducers a compile made, and its warnings and other diagnostics. */
export interface CompiledTransducers {
  transducers: Transducer[];
  diagnostics: Diagnostic[];
}

/** A compiled pmatch script: its container, as transducers and as bytes. */
export interface CompiledPmatch extends CompiledTransducers {
  /**
   * The container exactly as `hfst pmatch2fst` writes it, ready for a
   * tokeniser to load. `transducers` holds what `readTransducers` reads from
   * it: TOP, then each definition by name.
   */
  bytes: Uint8Array;
}

type OptionKind = "boolean" | "name" | "type" | "switches" | "files";
type OptionKinds<T> = { readonly [K in keyof Required<T>]: OptionKind };

const COMMON = { files: "files", sourceName: "name" } as const;

const REGEX: OptionKinds<RegexOptions> = {
  ...COMMON,
  type: "type",
  semicolon: "boolean",
  disjunct: "boolean",
  harmonize: "boolean",
  harmonizeFlags: "boolean",
  encodeWeights: "boolean",
  minimize: "boolean",
  xeroxComposition: "boolean",
  flagIsEpsilon: "boolean",
  replacePass: "boolean",
  verbose: "boolean",
};

const LEXC: OptionKinds<LexcOptions> = {
  ...COMMON,
  type: "type",
  alignStrings: "boolean",
  withFlags: "boolean",
  minimizeFlags: "boolean",
  renameFlags: "boolean",
  encodeWeights: "boolean",
  xeroxComposition: "boolean",
  flagIsEpsilon: "boolean",
  warnings: "switches",
  splitCharacters: "boolean",
  quiet: "boolean",
  verbose: "boolean",
};

const TWOLC: OptionKinds<TwolcOptions> = {
  ...COMMON,
  type: "type",
  resolveLeft: "boolean",
  resolveRight: "boolean",
  quiet: "boolean",
};

const PMATCH: OptionKinds<PmatchOptions> = {
  ...COMMON,
  flatten: "boolean",
  includeCosineDistances: "boolean",
};

function invalid(message: string): HfstError {
  return new HfstError("invalid-argument", message);
}

function isPlainObject(value: unknown): value is object {
  return typeof value === "object" && value !== null && !Array.isArray(value) && !(value instanceof Map);
}

function encodeFiles(what: string, files: unknown): CompileFilesGlue {
  if (!isPlainObject(files)) {
    throw invalid(`${what} takes files as an object of names to bytes or text`);
  }
  const encoder = new TextEncoder();
  const encoded: Record<string, Uint8Array> = {};
  for (const [name, contents] of Object.entries(files)) {
    if (contents instanceof Uint8Array) {
      encoded[name] = contents;
    } else if (typeof contents === "string") {
      encoded[name] = encoder.encode(contents);
    } else {
      throw invalid(`the file ${name} is neither bytes, as a Uint8Array, nor a string`);
    }
  }
  return encoded;
}

/** The options checked against `kinds`, as the JSON the glue takes, and the files. */
function prepare<T>(
  what: string,
  source: unknown,
  options: unknown,
  kinds: OptionKinds<T>,
): { json: string; files: CompileFilesGlue } {
  if (typeof source !== "string") throw invalid(`${what} takes its source as a string`);
  if (!isPlainObject(options)) throw invalid(`${what} takes its options as an object`);
  const known: Readonly<Record<string, OptionKind>> = kinds;
  const settings: Record<string, unknown> = {};
  let files: CompileFilesGlue = {};
  for (const [key, value] of Object.entries(options)) {
    const kind = Object.hasOwn(known, key) ? known[key] : undefined;
    if (kind === undefined) {
      throw invalid(`${what} has no option called ${key}; its options are ${Object.keys(known).join(", ")}`);
    }
    if (value === undefined) continue;
    const fits =
      kind === "files" ||
      (kind === "boolean" && typeof value === "boolean") ||
      (kind === "type" && typeof value === "string") ||
      (kind === "name" && typeof value === "string" && value !== "") ||
      (kind === "switches" && Array.isArray(value) && value.every((item) => typeof item === "string"));
    if (!fits) {
      const wanted = {
        files: "",
        boolean: "true or false",
        type: "a transducer type",
        name: "a string that is not empty",
        switches: "an array of switch names",
      }[kind];
      throw invalid(`the ${key} option of ${what} is ${wanted}, not ${JSON.stringify(value)}`);
    }
    if (kind === "files") files = encodeFiles(what, value);
    else settings[key] = value;
  }
  return { json: JSON.stringify(settings), files };
}

function run(compile: () => CompiledGlue): {
  transducers: Transducer[];
  diagnostics: Diagnostic[];
  bytes: Uint8Array | undefined;
} {
  return core.call(() => {
    const compiled = compile();
    return {
      transducers: compiled.transducers.map((transducer) => wrapTransducer(transducer)),
      diagnostics: compiled.diagnostics,
      bytes: compiled.bytes,
    };
  });
}

function one(what: string, transducers: Transducer[]): Transducer {
  const [transducer] = transducers;
  if (transducer === undefined) throw new HfstError("internal", `${what} made no transducer`);
  return transducer;
}

// [spec:hfst:req:javascript.compile]
/**
 * Compiles regular expressions as `hfst regexp2fst` compiles a file, to the
 * bytes it writes: the transducer is named `xre(?)`, as the tool names it. A
 * source holding several expressions throws `transducer-count` unless
 * `disjunct` makes their union. A source that does not compile throws an
 * `HfstCompileError` with every diagnostic; a success returns the warnings.
 * `@bin`, `@txt`, `@stxt`, `@pl` and `@re` read their files from `files`.
 */
export function compileRegex(source: string, options: RegexOptions = {}): CompiledTransducer {
  const { json, files } = prepare("compileRegex", source, options, REGEX);
  const { transducers, diagnostics } = run(() => glue.compileRegex(source, json, files));
  return { transducer: one("compileRegex", transducers), diagnostics };
}

// [spec:hfst:req:javascript.compile]
/**
 * Compiles a lexc source as `hfst lexc` compiles a file, to the bytes it
 * writes: the transducer is named `lexc(<sourceName>)`, as the tool names it
 * after the file. A source that does not compile throws an
 * `HfstCompileError` with every diagnostic; a success returns the warnings.
 * The regular expressions in it read their files from `files`.
 */
export function compileLexc(source: string, options: LexcOptions = {}): CompiledTransducer {
  const { json, files } = prepare("compileLexc", source, options, LEXC);
  const { transducers, diagnostics } = run(() => glue.compileLexc(source, json, files));
  return { transducer: one("compileLexc", transducers), diagnostics };
}

// [spec:hfst:req:javascript.compile]
/**
 * Compiles a twolc grammar as `hfst twolc` compiles a file: one transducer
 * for each rule, named after it, in the order the tool writes them. A
 * grammar that does not compile throws an `HfstCompileError` with every
 * diagnostic; a success returns the warnings. A grammar names no files, so
 * `files` is not read.
 */
export function compileTwolc(source: string, options: TwolcOptions = {}): CompiledTransducers {
  const { json } = prepare("compileTwolc", source, options, TWOLC);
  const { transducers, diagnostics } = run(() => glue.compileTwolc(source, json));
  return { transducers, diagnostics };
}

// [spec:hfst:req:javascript.compile]
/**
 * Compiles a pmatch script as `hfst pmatch2fst` compiles a file, to the
 * container it writes. A script that does not compile, or that compiles to
 * nothing to write, throws an `HfstCompileError` with every diagnostic.
 * `@include` and `@bin` read their files from `files`.
 */
export function compilePmatch(source: string, options: PmatchOptions = {}): CompiledPmatch {
  const { json, files } = prepare("compilePmatch", source, options, PMATCH);
  const { transducers, diagnostics, bytes } = run(() => glue.compilePmatch(source, json, files));
  if (bytes === undefined) throw new HfstError("internal", "compilePmatch wrote no container");
  return { transducers, diagnostics, bytes };
}
