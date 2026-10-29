import type { TransducerGlue } from "../shared/glue.ts";
import {
  RUNTIME_TRANSDUCER_TYPES,
  type RuntimeTransducerType,
  TransducerBase,
  transducerReaders,
} from "../shared/transducer.ts";
import { core } from "./binding.ts";

let wrap: (glue: TransducerGlue) => Transducer;

// [spec:hfst:req:javascript.load-bytes+1]
/**
 * An optimized-lookup transducer loaded from bytes. It owns module memory:
 * call `free()` when done with it, or declare it with `using`.
 */
export class Transducer extends TransducerBase<RuntimeTransducerType, TransducerGlue> {
  private constructor(glue: TransducerGlue) {
    super(core, glue, RUNTIME_TRANSDUCER_TYPES);
  }

  static {
    wrap = (glue) => new Transducer(glue);
  }
}

export const { readTransducers, readTransducer } = transducerReaders((bytes) =>
  core.call((glue) => glue.readTransducers(bytes).map((transducer) => wrap(transducer))),
);
