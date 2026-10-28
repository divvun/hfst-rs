import { assert, assertEqual, runTests, test } from "../test/harness.ts";
import { HfstError, messageOf } from "./errors.ts";

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

test("messageOf reads an Error's message and stringifies anything else", () => {
  assertEqual(messageOf(new TypeError("bad")), "bad", "an Error");
  assertEqual(messageOf("plain"), "plain", "a string");
  assertEqual(messageOf(42), "42", "a number");
});

await runTests("shared/errors");
