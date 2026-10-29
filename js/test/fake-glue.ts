// A stand-in for the generated glue, so the shared wrapper can be tested
// without a build. It behaves as the real module does at the boundary: it
// filters log events by the level set, reports a panic to the sink before it
// traps, and throws plain error objects.

import type { GlueInitInput, GlueObject, GlueSink, RuntimeGlue } from "../shared/glue.ts";

export class FakeGlue implements RuntimeGlue {
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
  fail(code: string, message: string, diagnostics?: unknown[]): never {
    this.entered += 1;
    throw diagnostics === undefined ? { code, message } : { code, message, diagnostics };
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
