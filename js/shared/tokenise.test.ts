// The tokeniser wrapper both packages share, against a fake glue.

import { FakeGlue, FakeObject } from "../test/fake-glue.ts";
import { assertEqual, runTests, test, thrownCode } from "../test/harness.ts";
import { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import type { TokeniserGlue } from "./glue.ts";
import {
  type FormatOptions,
  type Token,
  type TokeniseFormat,
  TokeniserBase,
  type TokeniseOptions,
  tokeniserReader,
} from "./tokenise.ts";

class FakeTokeniser extends FakeObject implements TokeniserGlue {
  readonly calls: string[] = [];

  tokenise(text: string, options: TokeniseOptions): Token[] {
    this.calls.push(`tokenise ${text} ${JSON.stringify(options)}`);
    return [{ surface: text, start: 0, end: text.length, analysed: true, readings: [] }];
  }

  format(text: string, format: string, options: FormatOptions): string {
    this.calls.push(`format ${text} ${format} ${JSON.stringify(options)}`);
    return text;
  }
}

class Loaded extends TokeniserBase {
  constructor(core: Core, glue: TokeniserGlue) {
    super(core, glue);
  }
}

async function started() {
  const glue = new FakeGlue();
  const core = new Core(glue, new URL("http://127.0.0.1:9/never.wasm"));
  await core.init(new Uint8Array(0));
  const fake = new FakeTokeniser(glue, "tokeniser");
  const { readTokeniser } = tokeniserReader(() => core.call(() => new Loaded(core, fake)));
  return { glue, fake, readTokeniser };
}

// [spec:hfst:req:javascript.tokenise/test]
test("tokenise and format hand the text and options to the module", async () => {
  const { fake, readTokeniser } = await started();
  const tokeniser = readTokeniser(new Uint8Array(1));
  assertEqual(tokeniser.tokenise("cat", { printAll: true })[0]?.surface, "cat", "the module's tokens");
  assertEqual(tokeniser.tokenise("dog").length, 1, "no options");
  assertEqual(tokeniser.format("cat", "giella-cg", { superblanks: true }), "cat", "the module's text");
  assertEqual(
    fake.calls.join("; "),
    'tokenise cat {"printAll":true}; tokenise dog {}; format cat giella-cg {"superblanks":true}',
    "calls",
  );
});

// [spec:hfst:req:javascript.tokenise/test]
test("text that is not a string, an unknown format and bytes that are not a Uint8Array throw invalid-argument without entering the module", async () => {
  const { glue, fake, readTokeniser } = await started();
  const tokeniser = readTokeniser(new Uint8Array(1));
  const before = glue.entered;
  const notText = ["cat"] as unknown as string;
  assertEqual(thrownCode(HfstError, () => tokeniser.tokenise(notText)), "invalid-argument", "tokenise");
  assertEqual(thrownCode(HfstError, () => tokeniser.format(notText, "cg")), "invalid-argument", "format");
  const badFormat = "plain" as TokeniseFormat;
  assertEqual(thrownCode(HfstError, () => tokeniser.format("cat", badFormat)), "invalid-argument", "format name");
  const notBytes = "HFST" as unknown as Uint8Array;
  assertEqual(thrownCode(HfstError, () => readTokeniser(notBytes)), "invalid-argument", "bytes");
  assertEqual(glue.entered, before, "calls into the module");
  assertEqual(fake.calls.length, 0, "calls on the tokeniser");
});

// [spec:hfst:req:javascript.tokenise/test]
test("a freed tokeniser throws freed and frees its glue once", async () => {
  const { glue, readTokeniser } = await started();
  const tokeniser = readTokeniser(new Uint8Array(1));
  tokeniser.free();
  tokeniser.free();
  assertEqual(glue.freed.join(), "tokeniser", "freed");
  assertEqual(thrownCode(HfstError, () => tokeniser.tokenise("cat")), "freed", "tokenise");
  assertEqual(thrownCode(HfstError, () => tokeniser.format("cat", "cg")), "freed", "format");
});

await runTests("shared/tokenise");
