import { assert, assertEqual, runTests, test } from "../test/harness.ts";
import { type Diagnostic, fromGlue, HfstCompileError, HfstError, messageOf } from "./errors.ts";

const diagnostic: Diagnostic = {
  severity: "error",
  message: "unexpected ;",
  source: "<test>",
  span: { start: 4, end: 5 },
  labels: [],
  notes: [],
  help: null,
  rendered: "error: unexpected ;",
};

// [spec:hfst:req:javascript.errors/test]
test("an HfstError is an Error with a name, a code, a message and a cause", () => {
  const cause = new Error("inner");
  const error = new HfstError("init-failed", "outer", { cause });
  assert(error instanceof Error, "an Error");
  assertEqual(error.name, "HfstError", "name");
  assertEqual(error.code, "init-failed", "code");
  assertEqual(error.message, "outer", "message");
  assert(error.cause === cause, "the cause is kept");
  assert(String(error.stack).includes("outer"), "the stack names the message");
});

// [spec:hfst:req:javascript.errors/test]
test("an HfstCompileError is an HfstError that carries the diagnostics", () => {
  const error = new HfstCompileError("lexc failed", [diagnostic]);
  assert(error instanceof HfstError, "an HfstError");
  assertEqual(error.name, "HfstCompileError", "name");
  assertEqual(error.code, "compile-error", "code");
  assertEqual(error.diagnostics[0]?.message, "unexpected ;", "the first diagnostic");
});

// [spec:hfst:req:javascript.errors/test]
test("a glue error object becomes an HfstError with its code and message", () => {
  const error = fromGlue({ code: "type-mismatch", message: "transducer type mismatch: x" });
  assert(error instanceof HfstError && !(error instanceof HfstCompileError), "an HfstError");
  assertEqual(error.code, "type-mismatch", "code");
  assertEqual(error.message, "transducer type mismatch: x", "message");
});

// [spec:hfst:req:javascript.errors/test]
test("a glue error with diagnostics becomes an HfstCompileError", () => {
  const error = fromGlue({ code: "compile-error", message: "failed", diagnostics: [diagnostic] });
  assert(error instanceof HfstCompileError, "an HfstCompileError");
  assertEqual(error.diagnostics.length, 1, "diagnostics");
});

test("an unknown code becomes internal and keeps the code in the message", () => {
  const error = fromGlue({ code: "brand-new", message: "something" });
  assertEqual(error?.code, "internal", "code");
  assert(error?.message.includes("brand-new") === true, "the message names the code");
});

test("anything that is not a glue error object is left alone", () => {
  for (const thrown of [new TypeError("x"), "text", 3, null, { code: 1, message: "m" }, {}]) {
    assertEqual(fromGlue(thrown), null, String(thrown));
  }
});

test("messageOf reads an Error's message and stringifies anything else", () => {
  assertEqual(messageOf(new TypeError("bad")), "bad", "an Error");
  assertEqual(messageOf("plain"), "plain", "a string");
  assertEqual(messageOf(42), "42", "a number");
});

await runTests("shared/errors");
