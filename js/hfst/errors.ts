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
 *   transducer.
 *
 * From the library, which `crates/hfst-wasm/src/error.rs` maps here:
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

function isHfstErrorCode(code: string): code is HfstErrorCode {
  return (HFST_ERROR_CODES as readonly string[]).includes(code);
}

/**
 * The error for an object a glue call threw, or null when it is not one. The
 * glue throws plain objects with `code` and `message`.
 */
export function fromGlue(thrown: unknown): HfstError | null {
  if (typeof thrown !== "object" || thrown === null || thrown instanceof Error) return null;
  const code: unknown = Reflect.get(thrown, "code");
  const message: unknown = Reflect.get(thrown, "message");
  if (typeof code !== "string" || typeof message !== "string") return null;
  if (isHfstErrorCode(code)) return new HfstError(code, message);
  return new HfstError("internal", `${message} (the module sent the unknown code ${code})`);
}

/** The message of anything a caller might throw. */
export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
