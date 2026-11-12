// [spec:hfst:req:javascript.errors+1]
/**
 * Every code an {@link HfstError} can carry. Code may test `error.code`
 * against these; they do not change between releases.
 *
 * From the wrapper:
 * - `not-initialised`: a call came before `init()` had resolved.
 * - `init-failed`: `init()` could not load, compile or start the module.
 * - `poisoned`: the module trapped, for instance on a panic. Its memory cannot
 *   be trusted after that, so every later call throws this without entering it.
 * - `freed`: a call on an object after its `free()`.
 * - `transducer-count`: `readTransducer` got bytes holding more than one
 *   transducer, or the module found more than one in the bytes
 *   `readOptimisedTransducer` got, or a compiler of `@divvun/hfst/compile`
 *   that returns one transducer compiled a source to several.
 *
 * From `@divvun/hfst/compile`:
 * - `compile-failed`: a source that did not compile, or an xfst script that
 *   stopped on a command that failed. The message is the library's, and
 *   `diagnostics` holds every diagnostic the compile made.
 *
 * From the library, which `crates/hfst-embed/src/error.rs` maps here:
 * - `type-mismatch`: transducers of different implementation types, or one of
 *   the wrong type for the operation.
 * - `unsupported-format`: bytes in a format the package does not read. The
 *   message names the format.
 * - `invalid-format`: input that is not well formed in the format expected,
 *   such as bytes that are not a transducer stream, or that end partway
 *   through one.
 * - `io`: a stream that cannot be read or written.
 * - `not-automaton`: an operation that needs automata got transducers.
 * - `cyclic`: an operation that needs a finite language got a cyclic one.
 * - `invalid-argument`: an argument the operation cannot use, such as an
 *   empty string, an unknown symbol, a bad state number or bad UTF-8.
 * - `internal`: a failure inside the library that no input should cause.
 * - `failed`: any other failure the library reports; the message says what.
 *
 * Each entry point starts, and is poisoned, on its own: `not-initialised` and
 * `poisoned` speak of the module of the entry point that threw them.
 */
export const HFST_ERROR_CODES = [
  "not-initialised",
  "init-failed",
  "poisoned",
  "freed",
  "transducer-count",
  "type-mismatch",
  "unsupported-format",
  "invalid-format",
  "io",
  "not-automaton",
  "cyclic",
  "invalid-argument",
  "internal",
  "failed",
  "compile-failed",
] as const;

export type HfstErrorCode = (typeof HFST_ERROR_CODES)[number];

// [spec:hfst:req:javascript.compile-errors]
/**
 * Where a diagnostic points in its source, as offsets in UTF-16 code units,
 * which is how a JavaScript string is indexed: `source.slice(start, end)`.
 */
export interface DiagnosticSpan {
  start: number;
  end: number;
}

/** A piece of a compile's source a diagnostic points at. */
export interface DiagnosticLabel {
  span: DiagnosticSpan;
  message: string;
}

// [spec:hfst:req:javascript.compile-errors]
/** Something a compile said about its source: an error, a warning or a note. */
export interface Diagnostic {
  severity: "error" | "warning" | "info";
  message: string;
  /** The name of the source it is about, as the compile was given it. */
  sourceName: string;
  /** Where its primary label is in the source; `null` when it points at no piece of it. */
  span: DiagnosticSpan | null;
  /** Every label, the primary one first. */
  labels: DiagnosticLabel[];
  notes: string[];
  help: string | null;
  /** The report the hfst tools print for it, without colour. */
  rendered: string;
}

/** An error from Divvun HFST. `code` says what went wrong, in a form code can test. */
export class HfstError extends Error {
  readonly code: HfstErrorCode;
  /**
   * Every diagnostic the compile behind the error made, for an error from a
   * compiler of `@divvun/hfst/compile`; empty for any other.
   */
  readonly diagnostics: readonly Diagnostic[];

  constructor(
    code: HfstErrorCode,
    message: string,
    options?: ErrorOptions & { diagnostics?: readonly Diagnostic[] },
  ) {
    super(message, options);
    this.name = "HfstError";
    this.code = code;
    this.diagnostics = options?.diagnostics ?? [];
  }
}

function isHfstErrorCode(code: string): code is HfstErrorCode {
  return (HFST_ERROR_CODES as readonly string[]).includes(code);
}

/**
 * The error for an object a glue call threw, or null when it is not one. The
 * glue throws plain objects with `code` and `message`, and with
 * `diagnostics` for a compile, which the module builds as `Diagnostic`s.
 */
export function fromGlue(thrown: unknown): HfstError | null {
  if (typeof thrown !== "object" || thrown === null || thrown instanceof Error) return null;
  const code: unknown = Reflect.get(thrown, "code");
  const message: unknown = Reflect.get(thrown, "message");
  if (typeof code !== "string" || typeof message !== "string") return null;
  if (!isHfstErrorCode(code)) {
    return new HfstError("internal", `${message} (the module sent the unknown code ${code})`);
  }
  const diagnostics: unknown = Reflect.get(thrown, "diagnostics");
  return new HfstError(code, message, Array.isArray(diagnostics) ? { diagnostics } : {});
}

/** The message of anything a caller might throw. */
export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
