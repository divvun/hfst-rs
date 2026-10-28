/**
 * The stable codes an {@link HfstError} carries.
 *
 * - `not-initialised`: a call came before `init()` had resolved.
 * - `init-failed`: `init()` could not load, compile or start the module.
 */
export type HfstErrorCode = "not-initialised" | "init-failed";

/** An error from Divvun HFST. `code` says what went wrong, in a form code can test. */
export class HfstError extends Error {
  readonly code: HfstErrorCode;

  constructor(code: HfstErrorCode, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "HfstError";
    this.code = code;
  }
}

/** The message of anything a caller might throw. */
export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
