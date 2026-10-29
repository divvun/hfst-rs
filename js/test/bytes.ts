// Comparing transducer bytes with what the hfst tools wrote, for the
// scenarios that check the full package against them.

import { assertEqual } from "./harness.ts";

/**
 * The bytes after the HFST3 header of a one-member stream. The tools name
 * what they make after the tool and the source file, so two transducers
 * built the same way differ in their headers and agree in their payloads.
 */
export function payload(bytes: Uint8Array): Uint8Array {
  return bytes.subarray(8 + (bytes[5] ?? 0) + 256 * (bytes[6] ?? 0));
}

async function gunzip(bytes: Uint8Array): Promise<Uint8Array> {
  const stream = new Blob([Uint8Array.from(bytes)]).stream().pipeThrough(new DecompressionStream("gzip"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

/**
 * Whether two payloads of `type` hold the same transducer, byte for byte. A
 * foma payload is gzip, and the hfst tools compress with flate2's zlib-rs
 * backend, which their zip dependency turns on, while the module links
 * miniz_oxide: the compressed bytes differ, so the text they hold is
 * compared.
 */
export async function assertPayload(
  type: string,
  actual: Uint8Array,
  expected: Uint8Array,
  what: string,
): Promise<void> {
  if (type !== "foma") return assertBytes(actual, expected, what);
  assertBytes(await gunzip(actual), await gunzip(expected), `${what}, uncompressed`);
}

/** {@link assertPayload} for a one-member stream, whose header must match too. */
export async function assertStream(
  type: string,
  actual: Uint8Array,
  expected: Uint8Array,
  what: string,
): Promise<void> {
  const header = expected.length - payload(expected).length;
  assertBytes(actual.subarray(0, header), expected.subarray(0, header), `${what}: header`);
  await assertPayload(type, payload(actual), payload(expected), what);
}

export function assertBytes(actual: Uint8Array, expected: Uint8Array, what: string): void {
  const length = Math.min(actual.length, expected.length);
  for (let i = 0; i < length; i += 1) {
    if (actual[i] !== expected[i]) throw new Error(`${what}: the bytes differ at ${i}`);
  }
  assertEqual(actual.length, expected.length, `${what}: length`);
}
