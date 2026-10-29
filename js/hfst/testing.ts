// The entry module of a test build: the package's API and its test hooks.
// js/build.mjs compiles it only with --test, into target/js-test, so the
// published package never contains it.

import { core } from "./binding.ts";
import type { Core } from "./core.ts";
import type { GlueObject } from "./glue.ts";
import { Handle } from "./handle.ts";
import { levelNumber, type LogEvent } from "./logging.ts";
import * as glue from "./pkg/hfst_wasm.js";

export * from "./index.ts";

interface TestObjectGlue extends GlueObject {
  label(): string;
  panicWhileBorrowed(message: string): void;
}

/** The exports of `crates/hfst-wasm/src/test_hooks.rs`. */
interface TestHooksGlue {
  TestObject: new (label: string) => TestObjectGlue;
  testPanic(message: string): void;
  testLog(level: number, message: string): void;
  testLibraryError(message: string): void;
}

/** A small object that owns module memory. Freeing it logs `dropped <label>` at debug level. */
export class TestObject extends Handle<TestObjectGlue> {
  constructor(core: Core, glue: TestObjectGlue) {
    super(core, glue);
  }

  label(): string {
    return this.use((glue) => glue.label());
  }

  /** Panics inside a method, while the module holds a borrow of the object. */
  panicWhileBorrowed(message: string): void {
    this.use((glue) => glue.panicWhileBorrowed(message));
  }
}

export interface TestHooks {
  createObject(label: string): TestObject;
  /** Panics in the module, which traps it. */
  panic(message: string): void;
  /** Logs `message` from the module at `level`, through the library's tracing. */
  log(level: LogEvent["level"], message: string): void;
  /** Fails as a library call does, with a type-mismatch error carrying `message`. */
  libraryError(message: string): void;
}

function hasTestHooks(glue: object): glue is TestHooksGlue {
  return ["TestObject", "testPanic", "testLog", "testLibraryError"].every((name) => name in glue);
}

function testHooks(core: Core, glue: object): TestHooks {
  if (!hasTestHooks(glue)) {
    throw new Error("this build has no test hooks: build it with node js/build.mjs --test");
  }
  return {
    createObject: (label) => new TestObject(core, core.call(() => new glue.TestObject(label))),
    panic: (message) => core.call(() => glue.testPanic(message)),
    log: (level, message) => core.call(() => glue.testLog(levelNumber(level), message)),
    libraryError: (message) => core.call(() => glue.testLibraryError(message)),
  };
}

export const testing: TestHooks = testHooks(core, glue);
