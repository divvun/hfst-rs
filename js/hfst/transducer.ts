import { HfstError } from "../shared/errors.ts";
import {
  RUNTIME_TRANSDUCER_TYPES,
  single,
  TransducerBase,
  transducerReaders,
} from "../shared/transducer.ts";
import { core } from "./binding.ts";
import type { FullTransducerGlue } from "./glue.ts";
import * as glue from "./pkg/hfst_wasm.js";

// [spec:hfst:req:javascript.load-bytes+1]
/**
 * The transducer types `@divvun/hfst` reads and writes, under the names
 * `hfst fst2fst --format` gives them: the two optimized-lookup types the
 * runtime reads, and
 *
 * - `openfst-tropical`: OpenFst's vector format over the tropical semiring
 *   (`TROPICAL_OPENFST`), the type the compilers build.
 * - `foma`: foma's format (`FOMA`).
 */
export const TRANSDUCER_TYPES = [...RUNTIME_TRANSDUCER_TYPES, "openfst-tropical", "foma"] as const;

export type TransducerType = (typeof TRANSDUCER_TYPES)[number];

function checkType(type: unknown): asserts type is TransducerType {
  if (!(TRANSDUCER_TYPES as readonly unknown[]).includes(type)) {
    throw new HfstError(
      "invalid-argument",
      `there is no transducer type called ${String(type)}; the types are ${TRANSDUCER_TYPES.join(", ")}`,
    );
  }
}

let wrap: (glue: FullTransducerGlue) => Transducer;

// [spec:hfst:req:javascript.load-bytes+1]
/**
 * A transducer of any type the library reads. It has every member the
 * runtime package's `Transducer` has, with the same behaviour, and can also
 * be written, converted and printed. It owns module memory: call `free()`
 * when done with it, or declare it with `using`.
 */
export class Transducer extends TransducerBase<TransducerType, FullTransducerGlue> {
  private constructor(glue: FullTransducerGlue) {
    super(core, glue, TRANSDUCER_TYPES);
  }

  static {
    wrap = (glue) => new Transducer(glue);
  }

  /**
   * An HFST3 stream holding this transducer in its own type: the bytes the
   * hfst library writes for it.
   */
  toBytes(): Uint8Array {
    return this.use((g) => g.toBytes());
  }

  /**
   * A new transducer of `type`, converted as `hfst fst2fst` converts it, with
   * the same name and properties. This transducer is unchanged.
   */
  convert(type: TransducerType): Transducer {
    checkType(type);
    return this.use((g) => wrap(g.convert(type)));
  }

  /**
   * The transducer as AT&T text, as `hfst fst2txt` prints it: with weights,
   * except for a foma transducer, which has none.
   */
  toAtt(): string {
    return this.use((g) => g.toAtt());
  }
}

export const { readTransducers, readTransducer } = transducerReaders((bytes) =>
  core.call(() => glue.readAnyTransducers(bytes).map((transducer) => wrap(transducer))),
);

// [spec:hfst:req:javascript.load-bytes+1]
/**
 * One HFST3 stream holding `transducers` in order, as the hfst library
 * writes them. They must all be of one type, or it throws `type-mismatch`;
 * an empty array throws `invalid-argument`, as a stream holds at least one.
 */
export function writeTransducers(transducers: readonly Transducer[]): Uint8Array {
  const [first] = transducers;
  if (first === undefined) {
    throw new HfstError("invalid-argument", "writeTransducers needs at least one transducer");
  }
  for (const transducer of transducers) {
    if (!(transducer instanceof Transducer)) {
      throw new HfstError("invalid-argument", "writeTransducers takes Transducer objects");
    }
    if (transducer.type !== first.type) {
      throw new HfstError(
        "type-mismatch",
        `a stream holds transducers of one type, and these are ${first.type} and ${transducer.type}`,
      );
    }
  }
  const members = transducers.map((transducer) => transducer.toBytes());
  const stream = new Uint8Array(members.reduce((total, member) => total + member.length, 0));
  let offset = 0;
  for (const member of members) {
    stream.set(member, offset);
    offset += member.length;
  }
  return stream;
}

// [spec:hfst:req:javascript.load-bytes+1]
/**
 * The transducer in AT&T text, made into `type` as `hfst txt2fst` makes it,
 * with `@0@` for epsilon. Text holding several transducers, separated by
 * `--` lines, throws `transducer-count`; malformed text throws
 * `invalid-format`.
 */
export function readAtt(text: string, type: TransducerType = "openfst-tropical"): Transducer {
  if (typeof text !== "string") {
    throw new HfstError("invalid-argument", "readAtt takes the AT&T text as a string");
  }
  checkType(type);
  const all = core.call(() => glue.readAtt(text, type).map((transducer) => wrap(transducer)));
  return single(all, "readAtt", "the text");
}
