// A small test harness for Node and Bun. Nothing here has node: types, so
// deno check can check the suite without @types/node. Each scenario runs in a
// process of its own, because a module instance, once started, stays started
// for the life of the process.

import type * as RuntimePackage from "../hfst-runtime/index.ts";

/** The surface both packages export; the full package adds to it. */
export type Package = typeof RuntimePackage;

/** What `run.mjs` hands each scenario process, as JSON in its first argument. */
export interface ScenarioArgs {
  /** URL of the package entry module under test. */
  module: string;
  /** `file:` URL of the package's `.wasm` file. */
  wasm: string;
  /** `http:` URL the runner serves the same `.wasm` file at. */
  http: string;
  /** The version the package declares. */
  version: string;
  /** Which variant of the scenario to run. */
  variant: string;
  /** Names the package, scenario and variant in the test output. */
  label: string;
}

export function scenarioArgs(): ScenarioArgs {
  const argv: unknown = Reflect.get(Reflect.get(globalThis, "process"), "argv");
  if (!Array.isArray(argv) || typeof argv[2] !== "string") {
    throw new Error("a scenario takes its arguments as JSON in argv[2]");
  }
  return JSON.parse(argv[2]);
}

export async function loadPackage(args: ScenarioArgs): Promise<Package> {
  return await import(args.module);
}

interface NodeFs {
  readFile(path: URL): Promise<Uint8Array<ArrayBuffer>>;
}

// A variable specifier keeps deno check from asking for node:fs types.
export async function readBytes(url: string): Promise<Uint8Array<ArrayBuffer>> {
  const specifier = "node:fs/promises";
  const fs: NodeFs = await import(specifier);
  return await fs.readFile(new URL(url));
}

type Body = () => void | Promise<void>;

const tests: { name: string; body: Body }[] = [];

export function test(name: string, body: Body): void {
  tests.push({ name, body });
}

const runtime = Reflect.has(globalThis, "Bun") ? "bun" : "node";

/** Runs the registered tests in order and throws if any failed. */
export async function runTests(name: string): Promise<void> {
  const label = `${runtime} ${name}`;
  let failed = 0;
  for (const { name, body } of tests) {
    try {
      await body();
      console.log(`ok - ${label}: ${name}`);
    } catch (error) {
      failed += 1;
      console.log(`not ok - ${label}: ${name}`);
      console.log(error instanceof Error ? (error.stack ?? error.message) : String(error));
    }
  }
  if (failed > 0) throw new Error(`${label}: ${failed} of ${tests.length} tests failed`);
}

export function assert(condition: boolean, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

export function assertEqual<T>(actual: T, expected: T, what: string): void {
  if (!Object.is(actual, expected)) {
    throw new Error(`${what}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

/** The `HfstError` class under test: a package's export, or the shared one. */
export type ErrorClass = abstract new (...args: never[]) => Error & { code: string };

/** The `code` of the `HfstError` that `f` throws. Fails if it returns or throws anything else. */
export function thrownCode(errors: ErrorClass, f: () => unknown): string {
  try {
    f();
  } catch (error) {
    assert(error instanceof errors, `expected an HfstError, got ${String(error)}`);
    return error.code;
  }
  throw new Error("expected an HfstError, but nothing was thrown");
}

/** The `code` of the `HfstError` that `promise` rejects with. */
export async function rejectedCode(errors: ErrorClass, promise: Promise<unknown>): Promise<string> {
  try {
    await promise;
  } catch (error) {
    assert(error instanceof errors, `expected an HfstError, got ${String(error)}`);
    return error.code;
  }
  throw new Error("expected an HfstError rejection, but the promise resolved");
}
