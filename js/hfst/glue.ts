// The exports only the full package's generated glue has, from
// crates/hfst-wasm. js/shared/glue.ts describes the ones both packages carry.

import type { TransducerGlue } from "../shared/glue.ts";

/** A transducer of any format the full package carries. */
export interface FullTransducerGlue extends TransducerGlue {
  /** An HFST3 stream holding this transducer, in its own format. */
  toBytes(): Uint8Array;
  /** A new transducer of the format named `format`. */
  convert(format: string): FullTransducerGlue;
  /** The transducer as AT&T text. */
  toAtt(): string;
}

export interface FullGlue {
  /** Every transducer in a stream of any format the library reads. */
  readAnyTransducers(bytes: Uint8Array): FullTransducerGlue[];
  /** Every transducer in AT&T text, in the format named `format`. */
  readAtt(text: string, format: string): FullTransducerGlue[];
}
