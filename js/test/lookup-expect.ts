// What `hfst lookup -X print-space` printed for the lookup fixtures
// (js/test/lookup-fixtures.mjs), read back into the shape a package's lookup
// returns, so the two can be compared result for result.

import type { LookupResult } from "../shared/lookup.ts";
import { assert, assertEqual } from "./harness.ts";

/** A result as the comparison sees it: its weight printed as `hfst lookup` prints weights. */
export interface Printed {
  output: string;
  symbols: string[];
  weight: string;
}

/** The lines of a text file, without the empty string after its final newline. */
export function linesOf(text: string): string[] {
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
}

/**
 * The results `hfst lookup` printed, one array per input in input order. It
 * prints a block of `input TAB output TAB weight` lines for each input and
 * ends the block with an empty line. Under print-space the output's symbols
 * are separated by spaces, and a flag or an epsilon prints as nothing between
 * two of them. An input with no results prints one line whose weight is
 * `inf`.
 */
export function parsePrint(text: string): Printed[][] {
  const blocks: Printed[][] = [];
  let block: Printed[] = [];
  for (const line of linesOf(text)) {
    if (line === "") {
      blocks.push(block);
      block = [];
      continue;
    }
    const fields = line.split("\t");
    assert(fields.length === 3, `hfst lookup printed an unexpected line: ${JSON.stringify(line)}`);
    const [, spaced = "", weight = ""] = fields;
    if (weight === "inf") continue;
    const symbols = spaced.split(" ").filter((symbol) => symbol !== "");
    block.push({ output: symbols.join(""), symbols, weight });
  }
  assertEqual(block.length, 0, "results after the last empty line");
  return blocks;
}

function compareCodePoints(a: string, b: string): number {
  const x = Array.from(a, (c) => c.codePointAt(0) ?? 0);
  const y = Array.from(b, (c) => c.codePointAt(0) ?? 0);
  for (let i = 0; i < Math.min(x.length, y.length); i += 1) {
    const difference = (x[i] ?? 0) - (y[i] ?? 0);
    if (difference !== 0) return difference;
  }
  return x.length - y.length;
}

function compareSymbols(a: readonly string[], b: readonly string[]): number {
  for (let i = 0; i < Math.min(a.length, b.length); i += 1) {
    const difference = compareCodePoints(a[i] ?? "", b[i] ?? "");
    if (difference !== 0) return difference;
  }
  return a.length - b.length;
}

/** The order lookup promises: by weight, then output, then symbols, in code point order. */
export function compareResults(
  a: { output: string; symbols: readonly string[]; weight: number | string },
  b: { output: string; symbols: readonly string[]; weight: number | string },
): number {
  return (
    Number(a.weight) - Number(b.weight) ||
    compareCodePoints(a.output, b.output) ||
    compareSymbols(a.symbols, b.symbols)
  );
}

export function printed(results: readonly LookupResult[]): Printed[] {
  return results.map(({ output, symbols, weight }) => ({ output, symbols, weight: weight.toFixed(6) }));
}

/**
 * Checks that `actual` holds what `hfst lookup` printed, in lookup's order.
 * The print is in the library's set order, so it is sorted before comparing.
 */
export function assertLookup(actual: readonly LookupResult[], expected: readonly Printed[], what: string): void {
  for (let i = 1; i < actual.length; i += 1) {
    const [before, after] = [actual[i - 1], actual[i]];
    assert(before !== undefined && after !== undefined, `${what}: result ${i}`);
    assert(compareResults(before, after) <= 0, `${what}: results ${i - 1} and ${i} are out of order`);
  }
  const sorted = [...expected].sort(compareResults);
  assertEqual(JSON.stringify(printed(actual)), JSON.stringify(sorted), what);
}
