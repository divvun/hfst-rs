// The exports only the full package's generated glue has, from
// crates/hfst-wasm. js/shared/glue.ts describes the ones both packages carry.

import type { Diagnostic } from "../shared/errors.ts";
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
  // Each compile takes its options as JSON.
  compileRegex(source: string, options: string, files: CompileFilesGlue): CompiledGlue;
  compileLexc(source: string, options: string, files: CompileFilesGlue): CompiledGlue;
  compileTwolc(source: string, options: string): CompiledGlue;
  /** The container is in `bytes`. */
  compilePmatch(source: string, options: string, files: CompileFilesGlue): CompiledGlue;
}

/** The files a compile may read, by name, from `crates/hfst-wasm/src/compile.rs`. */
export type CompileFilesGlue = Record<string, Uint8Array>;

/** What a compile made, as a plain object. */
export interface CompiledGlue {
  transducers: FullTransducerGlue[];
  diagnostics: Diagnostic[];
  bytes?: Uint8Array;
}
