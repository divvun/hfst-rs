// A stand-in for the generated glue, so the wrapper can be tested without a
// build. It behaves as the real module does at the boundary: it filters log
// events by the level set, reports a panic to the sink before it traps, and
// throws plain error objects.

import type { AcceptorInfo } from "../hfst/dhfst.ts";
import type {
  DhfstAcceptorGlue,
  DhfstErrorModelGlue,
  DhfstGlue,
  Glue,
  GlueInitInput,
  GlueObject,
  GlueSink,
  TokeniserGlue,
  TransducerGlue,
} from "../hfst/glue.ts";
import type { LookupResult } from "../hfst/lookup.ts";
import type { FormatOptions, Token, TokeniseOptions } from "../hfst/tokenise.ts";

export class FakeGlue implements Glue {
  readonly inputs: GlueInitInput[] = [];
  /** How many init calls fail before one succeeds. */
  failures = 0;
  sink: GlueSink | null = null;
  level = 0;
  installs = 0;
  levelChanges = 0;
  /** Calls into the module after init: every method below counts. */
  entered = 0;
  freed: string[] = [];
  /** What readTransducers hands out, as [format, name, ...property pairs]. */
  stream: string[][] = [];
  /** What readDhfst says it read; a type the wrapper does not know stands for a bad module. */
  dhfstType = "acceptor";
  /** Every transducer, tokeniser and DHFST model handed out, in order. */
  readonly made: FakeObject[] = [];

  default(options: { module_or_path: GlueInitInput }): Promise<unknown> {
    this.inputs.push(options.module_or_path);
    if (this.failures > 0) {
      this.failures -= 1;
      return Promise.reject(new Error("not a module"));
    }
    return Promise.resolve({});
  }

  install(sink: GlueSink, level: number): void {
    this.installs += 1;
    this.sink = sink;
    this.level = level;
  }

  setLogLevel(level: number): void {
    this.entered += 1;
    this.levelChanges += 1;
    this.level = level;
  }

  version(): string {
    this.entered += 1;
    return "0.0.0-fake";
  }

  readTransducers(_bytes: Uint8Array): TransducerGlue[] {
    this.entered += 1;
    const transducers = this.stream.map(
      ([format = "", name = "", ...pairs]) => new FakeTransducer(this, format, name, pairs),
    );
    this.made.push(...transducers);
    return transducers;
  }

  readTokeniser(_bytes: Uint8Array): TokeniserGlue {
    this.entered += 1;
    const tokeniser = new FakeTokeniser(this, "tokeniser");
    this.made.push(tokeniser);
    return tokeniser;
  }

  readDhfst(_bytes: Uint8Array): DhfstGlue {
    this.entered += 1;
    if (this.dhfstType === "error-model") {
      const model = new FakeDhfstErrorModel(this, this.dhfstType);
      this.made.push(model);
      return { type: "error-model", model };
    }
    const model = new FakeDhfstAcceptor(this, this.dhfstType);
    this.made.push(model);
    if (this.dhfstType === "acceptor") return { type: "acceptor", model };
    const unknown: unknown = { type: this.dhfstType, model };
    return unknown as DhfstGlue;
  }

  /** Logs as the module does: only at or above the level set. */
  log(level: number, message: string): void {
    this.entered += 1;
    if (level <= this.level) this.sink?.log(level, "fake", message);
  }

  /** Panics with `message`, or traps without one when it is null. */
  trap(message: string | null): never {
    this.entered += 1;
    if (message !== null) this.sink?.panic(message);
    throw new WebAssembly.RuntimeError("unreachable");
  }

  /** Throws the way a failing export does. */
  fail(code: string, message: string): never {
    this.entered += 1;
    throw { code, message };
  }

  object(label: string): FakeObject {
    this.entered += 1;
    return new FakeObject(this, label);
  }
}

export class FakeObject implements GlueObject {
  readonly #glue: FakeGlue;
  readonly label: string;
  /** When set, free() traps and frees nothing. */
  trapOnFree = false;

  constructor(glue: FakeGlue, label: string) {
    this.#glue = glue;
    this.label = label;
  }

  free(): void {
    if (this.trapOnFree) this.#glue.trap("free failed");
    this.#glue.entered += 1;
    this.#glue.freed.push(this.label);
  }
}

export class FakeTransducer extends FakeObject implements TransducerGlue {
  readonly #format: string;
  readonly #pairs: string[];

  constructor(glue: FakeGlue, format: string, name: string, pairs: string[]) {
    super(glue, name);
    this.#format = format;
    this.#pairs = pairs;
  }

  format(): string {
    return this.#format;
  }

  name(): string {
    return this.label;
  }

  propertyPairs(): string[] {
    return this.#pairs;
  }

  /** The arguments of every lookup, in order. */
  readonly lookups: [string, number | undefined, number | undefined][] = [];

  /** Answers every input with one result whose output is the input. */
  lookup(input: string, limit: number | undefined, timeLimitMs: number | undefined): LookupResult[] {
    this.lookups.push([input, limit, timeLimitMs]);
    return [{ output: input, symbols: [...input], weight: 0 }];
  }
}

export class FakeTokeniser extends FakeObject implements TokeniserGlue {
  /** Every call, with its arguments. */
  readonly calls: string[] = [];

  /** Answers every text with one analysed token covering it. */
  tokenise(text: string, options: TokeniseOptions): Token[] {
    this.calls.push(`tokenise ${text} ${JSON.stringify(options)}`);
    return [{ surface: text, start: 0, end: text.length, analysed: true, readings: [] }];
  }

  format(text: string, format: string, options: FormatOptions): string {
    this.calls.push(`format ${text} ${format} ${JSON.stringify(options)}`);
    return text;
  }
}

const FAKE_INFO: AcceptorInfo = {
  ids: 3,
  slots: 9,
  usedSlots: 4,
  checkBytes: 1,
  labelMax: 5,
  recordBytes: 1,
  listRecords: 0,
  arcs: 4,
  freeArcs: 1,
  freePairs: 1,
  weights: 2,
  finals: 1,
  distances: true,
};

export class FakeDhfstAcceptor extends FakeObject implements DhfstAcceptorGlue {
  /** Every word looked up, in order. */
  readonly lookups: string[] = [];

  version(): number {
    return 1;
  }

  symbols(): string[] {
    return ["@_EPSILON_SYMBOL_@", "a", "b"];
  }

  meta(): string | undefined {
    return undefined;
  }

  info(): AcceptorInfo {
    return { ...FAKE_INFO };
  }

  /** Accepts the words of a and b only, each at a weight of its length. */
  lookup(word: string): number | undefined {
    this.lookups.push(word);
    return /^[ab]*$/.test(word) ? word.length : undefined;
  }
}

export class FakeDhfstErrorModel extends FakeObject implements DhfstErrorModelGlue {
  version(): number {
    return 1;
  }

  stateCount(): number {
    return 2;
  }

  symbols(): string[] {
    return ["@_EPSILON_SYMBOL_@", "a"];
  }

  meta(): string | undefined {
    return '{"writer":"fake"}';
  }
}
