// Tests for the check that the package's module carries no compiler, foma,
// OpenFst or algebra code. The modules they check are what
// `node js/build.mjs --test` leaves in target/.

import { assert, assertEqual, readBytes, runTests, test } from "./harness.ts";
import { assertReadOnlyModule, FORBIDDEN, forbiddenFunctions, functionNames, plainPath } from "./read-only-module.ts";

const target = new URL("../../target/", import.meta.url);
const releaseModule = new URL("js-check/hfst_wasm_bg.wasm", target).href;
const testModule = new URL("js-test/dist/pkg/hfst_wasm_bg.wasm", target).href;

function leb(value: number): number[] {
  const bytes: number[] = [];
  let rest = value;
  do {
    const low = rest & 0x7f;
    rest >>>= 7;
    bytes.push(rest === 0 ? low : low | 0x80);
  } while (rest !== 0);
  return bytes;
}

function sized(bytes: number[]): number[] {
  return [...leb(bytes.length), ...bytes];
}

function text(value: string): number[] {
  return sized([...new TextEncoder().encode(value)]);
}

function section(id: number, content: number[]): number[] {
  return [id, ...sized(content)];
}

/**
 * A module with an empty type section, a producers section and, unless
 * `names` is undefined, a name section with a module name and these function
 * names. The sections around the name section are there to be skipped.
 */
function moduleNaming(names: string[] | undefined): Uint8Array {
  const bytes = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
  bytes.push(...section(1, [0]), ...section(0, [...text("producers"), 0]));
  if (names !== undefined) {
    const moduleName = [0, ...sized(text("synthetic"))];
    const functions = [1, ...sized([...leb(names.length), ...names.flatMap((name, i) => [...leb(i), ...text(name)])])];
    bytes.push(...section(0, [...text("name"), ...moduleName, ...functions]));
  }
  return new Uint8Array(bytes);
}

const own = "hfst_wasm[8fa90fcbd8ea6d6]::lookup::lookup";
const sliceDebug = "<&[f32] as core[608f92abc48d28da]::fmt::Debug>::fmt";

function failure(module: Uint8Array): string {
  try {
    assertReadOnlyModule(module);
  } catch (error) {
    assert(error instanceof Error, `expected an Error, got ${String(error)}`);
    return error.message;
  }
  throw new Error("the check passed a module it should have failed");
}

test("the reader lists the function names in a module's name section", () => {
  const names = functionNames(moduleNaming([own, sliceDebug]));
  assertEqual(JSON.stringify(names), JSON.stringify([own, sliceDebug]), "names");
});

test("a crate disambiguator is dropped and a slice type is kept", () => {
  assertEqual(plainPath(sliceDebug), "<&[f32] as core::fmt::Debug>::fmt", "plain path");
});

test("a module cut short fails to read", () => {
  const module = moduleNaming([own]);
  let message = "";
  try {
    functionNames(module.subarray(0, module.length - 3));
  } catch (error) {
    message = error instanceof Error ? error.message : String(error);
  }
  assert(message.includes("ends"), `the failure says where the module ends: ${message}`);
});

// Names as wasm-bindgen writes them, one from each part the module must not carry.
const offenders: { part: string; name: string }[] = [
  { part: "the regex compiler", name: "<hfst[3c5e1d]::xre::XreCompiler>::compile" },
  { part: "the lexc compiler", name: "<hfst[3c5e1d]::lexc::LexcCompiler>::parse" },
  { part: "the twolc compiler", name: "nfst_twolc[77aa01]::parser::parse" },
  { part: "the pmatch compiler", name: "<hfst[3c5e1d]::pmatch_compiler::PmatchCompiler>::compile" },
  { part: "the xfst compiler", name: "hfst[3c5e1d]::xfst_compiler::apply_up" },
  { part: "the foma backend", name: "core[608f92]::ptr::drop_in_place::<foma[12ab34]::Fsm>" },
  {
    part: "the tropical OpenFst backend",
    name: "<rustfst[9f00aa]::fst_impls::vector_fst::VectorFst<rustfst[9f00aa]::semirings::TropicalWeight> as hfst[3c5e1d]::backend::Backend>::copy",
  },
  {
    part: "the transducer algebra",
    name: "<hfst[3c5e1d]::hfst_transducer::HfstTransducer<hfst[3c5e1d]::transducer::Transducer<hfst[3c5e1d]::transducer::WeightedTables>>>::compose",
  },
];

// [spec:hfst:req:javascript.read-only-module/test]
test("a function from each forbidden part fails the check, which names it", () => {
  assertEqual(offenders.length, FORBIDDEN.length, "one offender per forbidden part");
  for (const { part, name } of offenders) {
    const message = failure(moduleNaming([own, name]));
    assert(message.includes(part), `the failure names ${part}: ${message}`);
    assert(message.includes(plainPath(name)), `the failure names the function: ${message}`);
  }
});

// [spec:hfst:req:javascript.read-only-module/test]
test("every forbidden path matches, also as a generic argument", () => {
  for (const { part, paths } of FORBIDDEN) {
    for (const path of paths) {
      const hashed = path.replace(/^([a-z_]+)::/, "$1[5f3a]::");
      for (const name of [`${hashed}f`, `core[608f]::ptr::drop_in_place::<${hashed}T>`]) {
        const found = forbiddenFunctions([name]);
        assertEqual(found[0]?.part, part, `the part ${name} comes from`);
      }
    }
  }
});

test("a generic function is put down to its own part, not its type arguments", () => {
  const name =
    "hfst[3c5e1d]::hfst_xerox_rules::bracketing::bracketed_replace::<rustfst[9f00aa]::fst_impls::vector_fst::VectorFst<rustfst[9f00aa]::semirings::TropicalWeight>>";
  assertEqual(forbiddenFunctions([name])[0]?.part, "the transducer algebra", "part");
});

test("names that only resemble a forbidden path pass", () => {
  const near = [
    "<hfst[3c5e1d]::pmatch::PmatchContainer>::match_",
    "hfst[3c5e1d]::pmatch_core::f",
    "<hfst[3c5e1d]::transducer::Transducer<hfst[3c5e1d]::transducer::WeightedTables> as hfst[3c5e1d]::backend::Backend>::copy",
    "hfst[3c5e1d]::hfst_data_types::f",
    "hfst_wasm[8fa90f]::tokenise::read_tokeniser",
    "notfoma[1a]::f",
    "nfst_xrefs[1a]::f",
  ];
  assertEqual(JSON.stringify(forbiddenFunctions(near)), "[]", "offences");
  assertEqual(assertReadOnlyModule(moduleNaming([own, ...near])), near.length + 1, "names checked");
});

// [spec:hfst:req:javascript.read-only-module/test]
test("a module whose names cannot show what it carries fails the check", () => {
  assert(failure(moduleNaming(undefined)).includes("no name section"), "a module without a name section fails");
  assert(
    failure(moduleNaming(["<hfst[3c5e1d]::pmatch::PmatchContainer>::match_"])).includes("no hfst_wasm function"),
    "a module that names none of the crate's own functions fails",
  );
});

// [spec:hfst:req:javascript.read-only-module/test]
test("the release module, as it was before wasm-opt, carries no forbidden code", async () => {
  const checked = assertReadOnlyModule(await readBytes(releaseModule));
  assert(checked > 0, "the release module names its functions");
});

// [spec:hfst:req:javascript.read-only-module/test]
test("the unoptimised test build carries no forbidden code", async () => {
  const checked = assertReadOnlyModule(await readBytes(testModule));
  assert(checked > 0, "the test build names its functions");
});

await runTests("read-only module");
