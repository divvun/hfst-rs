import { assert, assertEqual, runTests, test } from "../test/harness.ts";
import { fromGlue, HfstError, messageOf } from "./errors.ts";

// [spec:hfst:req:javascript.errors+1/test]
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

// [spec:hfst:req:javascript.errors+1/test]
test("a glue error object becomes an HfstError with its code and message", () => {
  const error = fromGlue({ code: "type-mismatch", message: "transducer type mismatch: x" });
  assert(error instanceof HfstError, "an HfstError");
  assertEqual(error.code, "type-mismatch", "code");
  assertEqual(error.message, "transducer type mismatch: x", "message");
});

// [spec:hfst:req:javascript.compile-errors/test]
test("a compile error object keeps its diagnostics, and any other error has none", () => {
  const diagnostic = {
    severity: "error" as const,
    message: "expected RightBracket",
    sourceName: "<stdin>",
    span: { start: 2, end: 2 },
    labels: [{ span: { start: 2, end: 2 }, message: "expected RightBracket" }],
    notes: [],
    help: null,
    rendered: "Error: expected RightBracket\n",
  };
  const error = fromGlue({ code: "compile-failed", message: "XRE parsing failed", diagnostics: [diagnostic] });
  assertEqual(error?.code, "compile-failed", "code");
  assertEqual(error?.diagnostics.length, 1, "diagnostics");
  assert(error?.diagnostics[0] === diagnostic, "the module's own object");
  assertEqual(new HfstError("failed", "x").diagnostics.length, 0, "none on another error");
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

await runTests("hfst/errors");
