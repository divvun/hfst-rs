// Unit tests for the wrapper's module instance. They run against a fake glue,
// so they need no build.

import { assert, assertEqual, rejectedCode, runTests, test, thrownCode } from "../test/harness.ts";
import { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import type { GlueInitInput, RuntimeGlue } from "./glue.ts";

/** A glue that records what init hands it, and fails as often as it is told to. */
class FakeGlue implements RuntimeGlue {
  readonly inputs: GlueInitInput[] = [];
  failures = 0;

  default(options: { module_or_path: GlueInitInput }): Promise<unknown> {
    this.inputs.push(options.module_or_path);
    if (this.failures > 0) {
      this.failures -= 1;
      return Promise.reject(new Error("not a module"));
    }
    return Promise.resolve({});
  }

  version(): string {
    return "0.0.0-fake";
  }
}

const nowhere = new URL("http://127.0.0.1:9/never.wasm");
const emptyModule = new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]);

function version(core: Core): string {
  return core.call((glue) => glue.version());
}

/** Runs `body` with `fetch` answering every request with `status`, and returns what was asked for. */
async function withFetch(status: number, body: () => Promise<void>): Promise<string[]> {
  const requested: string[] = [];
  const realFetch = globalThis.fetch;
  globalThis.fetch = ((input: RequestInfo | URL) => {
    requested.push(input instanceof Request ? input.url : String(input));
    return Promise.resolve(new Response(emptyModule, { status }));
  }) as typeof fetch;
  try {
    await body();
  } finally {
    globalThis.fetch = realFetch;
  }
  return requested;
}

// [spec:hfst:req:javascript.init/test]
test("making a Core loads nothing, and a call before init throws not-initialised", () => {
  const glue = new FakeGlue();
  const core = new Core(glue, nowhere);
  assertEqual(glue.inputs.length, 0, "glue starts");
  assertEqual(thrownCode(HfstError, () => version(core)), "not-initialised", "a call");
});

// [spec:hfst:req:javascript.init/test]
test("init hands bytes, modules and responses to the glue as they are", async () => {
  const sources = [
    emptyModule,
    emptyModule.buffer,
    await WebAssembly.compile(emptyModule),
    new Response(emptyModule),
  ];
  for (const source of sources) {
    const glue = new FakeGlue();
    await new Core(glue, nowhere).init(source);
    assert(glue.inputs[0] === source, `${String(source)} reaches the glue unchanged`);
  }
});

// [spec:hfst:req:javascript.init/test]
test("init reads a file: URL from disk, given as a URL, a string or the default", async () => {
  for (const source of [new URL(import.meta.url), import.meta.url, undefined]) {
    const glue = new FakeGlue();
    await new Core(glue, new URL(import.meta.url)).init(source);
    const input = glue.inputs[0];
    assert(input instanceof Uint8Array && input.length > 0, "the glue gets the file's bytes");
  }
});

// [spec:hfst:req:javascript.init/test]
test("init fetches other URLs, and a string that is not a URL goes to fetch as it is", async () => {
  const glue = new FakeGlue();
  const requested = await withFetch(200, async () => {
    await new Core(glue, nowhere).init("https://example.invalid/hfst.wasm");
    await new Core(glue, nowhere).init("pkg/hfst.wasm");
  });
  assertEqual(requested.join(" "), "https://example.invalid/hfst.wasm pkg/hfst.wasm", "fetches");
  assert(glue.inputs.every((input) => input instanceof Response), "the glue gets the responses");
});

// [spec:hfst:req:javascript.init/test]
test("an error status fails init with init-failed", async () => {
  const glue = new FakeGlue();
  await withFetch(404, async () => {
    const code = await rejectedCode(HfstError, new Core(glue, nowhere).init(nowhere));
    assertEqual(code, "init-failed", "init from a missing file");
  });
  assertEqual(glue.inputs.length, 0, "glue starts");
  const notFound = new Core(glue, nowhere).init(new Response(null, { status: 404 }));
  assertEqual(await rejectedCode(HfstError, notFound), "init-failed", "init from a 404 response");
});

// [spec:hfst:req:javascript.init/test]
test("init twice returns the first promise and starts the glue once", async () => {
  const glue = new FakeGlue();
  const core = new Core(glue, nowhere);
  const first = core.init(emptyModule);
  assertEqual(thrownCode(HfstError, () => version(core)), "not-initialised", "a call while init runs");
  assert(core.init(new Uint8Array(0)) === first, "a second init while the first runs");
  await first;
  assert(core.init() === first, "a second init after the first resolved");
  assertEqual(glue.inputs.length, 1, "glue starts");
  assertEqual(version(core), "0.0.0-fake", "a call after init");
});

// [spec:hfst:req:javascript.init/test]
test("a failed init rejects with init-failed, and the next init tries again", async () => {
  const glue = new FakeGlue();
  glue.failures = 1;
  const core = new Core(glue, nowhere);
  const failed = core.init(emptyModule);
  const error = await failed.then(
    () => null,
    (reason: unknown) => reason,
  );
  assert(error instanceof HfstError && error.code === "init-failed", "the first init fails");
  assert(error.cause instanceof Error && error.cause.message === "not a module", "it keeps the cause");
  assertEqual(thrownCode(HfstError, () => version(core)), "not-initialised", "a call after it");
  const retried = core.init(emptyModule);
  assert(retried !== failed, "the next init starts again");
  await retried;
  assertEqual(version(core), "0.0.0-fake", "a call after the retry");
});

await runTests("shared/core");
