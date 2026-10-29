// [spec:hfst:req:javascript.errors]
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
 *
 * From the library, which `crates/hfst-wasm-runtime/src/error.rs` maps here:
 * - `type-mismatch`: transducers of different implementation types, or one of
 *   the wrong type for the operation.
 * - `unsupported-format`: a format or implementation type the package does
 *   not carry.
 * - `invalid-format`: input that is not well formed in the format expected.
 * - `io`: a stream that cannot be read or written.
 * - `not-automaton`: an operation that needs automata got transducers.
 * - `cyclic`: an operation that needs a finite language got a cyclic one.
 * - `invalid-argument`: an argument the operation cannot use, such as an
 *   empty string, an unknown symbol, a bad state number or bad UTF-8.
 * - `internal`: a failure inside the library that no input should cause.
 * - `failed`: any other failure the library reports; the message says what.
 * - `compile-error`: a compile failed. The error is an
 *   {@link HfstCompileError} with the compiler's diagnostics.
 */
export const HFST_ERROR_CODES = [
  "not-initialised",
  "init-failed",
  "poisoned",
  "freed",
  "type-mismatch",
  "unsupported-format",
  "invalid-format",
  "io",
  "not-automaton",
  "cyclic",
  "invalid-argument",
  "internal",
  "failed",
  "compile-error",
] as const;

export type HfstErrorCode = (typeof HFST_ERROR_CODES)[number];

/** An error from Divvun HFST. `code` says what went wrong, in a form code can test. */
export class HfstError extends Error {
  readonly code: HfstErrorCode;

  constructor(code: HfstErrorCode, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "HfstError";
    this.code = code;
  }
}

/** A range of UTF-8 bytes in a compiler's source text, end exclusive. */
export interface Span {
  start: number;
  end: number;
}

/** A further place a diagnostic points at, with what it says there. */
export interface DiagnosticLabel {
  span: Span;
  message: string;
}

/** One message from a compiler. */
export interface Diagnostic {
  severity: "error" | "warning" | "info";
  message: string;
  /** The name of the source the diagnostic points into. */
  source: string;
  /** The span of the primary label, or null when it points at nothing. */
  span: Span | null;
  /** The labels after the primary one. */
  labels: DiagnosticLabel[];
  notes: string[];
  help: string | null;
  /** The whole report, as the command-line tools print it. */
  rendered: string;
}

/** A failed compile, with every diagnostic the compiler produced. */
export class HfstCompileError extends HfstError {
  readonly diagnostics: readonly Diagnostic[];

  constructor(message: string, diagnostics: readonly Diagnostic[], options?: ErrorOptions) {
    super("compile-error", message, options);
    this.name = "HfstCompileError";
    this.diagnostics = diagnostics;
  }
}

function isHfstErrorCode(code: string): code is HfstErrorCode {
  return (HFST_ERROR_CODES as readonly string[]).includes(code);
}

/**
 * The error for an object a glue call threw, or null when it is not one. The
 * glue throws plain objects with `code` and `message`, and `diagnostics` for a
 * failed compile.
 */
export function fromGlue(thrown: unknown): HfstError | null {
  if (typeof thrown !== "object" || thrown === null || thrown instanceof Error) return null;
  const code: unknown = Reflect.get(thrown, "code");
  const message: unknown = Reflect.get(thrown, "message");
  if (typeof code !== "string" || typeof message !== "string") return null;
  const diagnostics: unknown = Reflect.get(thrown, "diagnostics");
  if (Array.isArray(diagnostics)) return new HfstCompileError(message, diagnostics);
  if (isHfstErrorCode(code)) return new HfstError(code, message);
  return new HfstError("internal", `${message} (the module sent the unknown code ${code})`);
}

/** The message of anything a caller might throw. */
export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
