import { HfstError } from "./errors.ts";

// [spec:hfst:req:javascript.lookup]
/** One result of a lookup: a path through the transducer for the input. */
export interface LookupResult {
  /** The path's output symbols, concatenated. */
  output: string;
  /**
   * The path's output symbols. Flag diacritics and epsilons are left out, as
   * `hfst lookup` leaves them out of what it prints.
   */
  symbols: string[];
  /** The path's weight: the sum of the weights along it. */
  weight: number;
}

// [spec:hfst:req:javascript.lookup]
/** Limits on one lookup. Leave either out for no limit. */
export interface LookupOptions {
  /**
   * The most results to return, an integer 0 or more. The search stops once
   * it has found this many paths, so they are the first it found, not the
   * lightest.
   */
  limit?: number;
  /**
   * Milliseconds of search, 0 or more, after which the lookup returns the
   * results it has found so far. 0 sets no limit, as `hfst lookup
   * --time-cutoff=0` does.
   */
  timeLimitMs?: number;
}

export type GlueLookupArgs = [input: string, limit: number | undefined, timeLimitMs: number | undefined];

/** The largest result limit the glue takes; more than any lookup can find. */
const MAX_LIMIT = 0xffffffff;

/**
 * Checks what a caller passed to `lookup` and turns it into the glue's
 * arguments. Anything it cannot use throws `invalid-argument`, before the
 * module is entered.
 */
export function lookupArgs(input: unknown, options: unknown): GlueLookupArgs {
  if (typeof input !== "string") {
    throw new HfstError("invalid-argument", `lookup takes the input as a string, not ${typeof input}`);
  }
  if (options === undefined) return [input, undefined, undefined];
  if (typeof options !== "object" || options === null) {
    throw new HfstError("invalid-argument", "lookup takes its options as an object");
  }
  const limit: unknown = Reflect.get(options, "limit");
  const timeLimitMs: unknown = Reflect.get(options, "timeLimitMs");
  if (limit !== undefined && !(typeof limit === "number" && Number.isInteger(limit) && limit >= 0)) {
    throw new HfstError("invalid-argument", `a lookup limit is an integer 0 or more, not ${String(limit)}`);
  }
  if (
    timeLimitMs !== undefined &&
    !(typeof timeLimitMs === "number" && Number.isFinite(timeLimitMs) && timeLimitMs >= 0)
  ) {
    throw new HfstError(
      "invalid-argument",
      `a lookup time limit is a finite number of milliseconds, 0 or more, not ${String(timeLimitMs)}`,
    );
  }
  return [input, limit === undefined ? undefined : Math.min(limit, MAX_LIMIT), timeLimitMs];
}
