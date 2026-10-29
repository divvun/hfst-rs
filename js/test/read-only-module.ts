// The check js/build.mjs runs on the package's WebAssembly module, and the
// suite runs again on what the build kept. It takes the module's bytes, so it
// needs no node: types and deno check can check it.

/** A part of the library the module must not carry, and the Rust paths its code sits under. */
export interface ForbiddenPart {
  readonly part: string;
  /** Prefixes of demangled paths, each matched at the start of a path segment. */
  readonly paths: readonly string[];
}

// The nfst_* crates are the compilers' parser front-ends. Code under the
// tropical backend's own types is named by rustfst paths: StdVectorFst is
// rustfst's VectorFst, and a trait impl's name gives the type, not the module
// the impl sits in. Every operation of the algebra runs through the
// HfstTransducer facade or an AlgebraBackend method. The runtime reads and
// looks up through hfst::transducer and hfst::pmatch, so the facade has no
// part in it.
export const FORBIDDEN: readonly ForbiddenPart[] = [
  { part: "the regex compiler", paths: ["hfst::xre::", "nfst_xre::"] },
  { part: "the lexc compiler", paths: ["hfst::lexc::", "nfst_lexc::"] },
  { part: "the twolc compiler", paths: ["hfst::twolc::", "nfst_twolc::"] },
  { part: "the pmatch compiler", paths: ["hfst::pmatch_compiler::", "nfst_pmatch::"] },
  { part: "the xfst compiler", paths: ["hfst::xfst_compiler::", "nfst_xfst::"] },
  { part: "the foma backend", paths: ["foma::", "hfst::backend_foma::", "hfst::backend_foma_sigma::"] },
  {
    part: "the tropical OpenFst backend",
    paths: [
      "rustfst::",
      "hfst_openfst::",
      "hfst::tropical_weight_transducer::",
      "hfst::convert_tropical_weight_transducer::",
    ],
  },
  {
    part: "the transducer algebra",
    paths: [
      "hfst::hfst_transducer::",
      "hfst::backend::AlgebraBackend",
      "hfst::hfst_rules::",
      "hfst::hfst_xerox_rules::",
      "hfst::compose_intersect_",
      "hfst::harmonize_unknown_and_identity_symbols::",
    ],
  },
];

const segmentStart = "(?<![A-Za-z0-9_])";

function escaped(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const matchers = FORBIDDEN.map(({ part, paths }) => ({
  part,
  pattern: new RegExp(paths.map((path) => segmentStart + escaped(path)).join("|")),
}));

// The crate's own functions. A module whose names include none of them was
// stripped or mangled, and its names prove nothing.
const ownCode = new RegExp(`${segmentStart}hfst_wasm::`);

/**
 * A demangled name without the crate disambiguators v0 mangling adds, so
 * `hfst[1a2b]::xre::f` reads `hfst::xre::f`. A disambiguator follows an
 * identifier directly; a slice type such as `&[f32]` does not.
 */
export function plainPath(name: string): string {
  return name.replace(/(?<=[A-Za-z0-9_])\[[0-9a-f]+\]/g, "");
}

interface Cursor {
  readonly bytes: Uint8Array;
  at: number;
}

function byte(cursor: Cursor): number {
  const value = cursor.bytes[cursor.at];
  if (value === undefined) throw new Error(`the module ends early, at byte ${cursor.at}`);
  cursor.at += 1;
  return value;
}

function u32(cursor: Cursor): number {
  let value = 0;
  for (let shift = 0; shift <= 28; shift += 7) {
    const next = byte(cursor);
    value += (next & 0x7f) * 2 ** shift;
    if ((next & 0x80) === 0) return value;
  }
  throw new Error(`the module has an over-long integer before byte ${cursor.at}`);
}

const utf8 = new TextDecoder("utf-8", { fatal: true });

function name(cursor: Cursor): string {
  const length = u32(cursor);
  const end = cursor.at + length;
  if (end > cursor.bytes.length) throw new Error(`the module ends inside a name at byte ${cursor.at}`);
  const text = utf8.decode(cursor.bytes.subarray(cursor.at, end));
  cursor.at = end;
  return text;
}

function functionNameSubsection(cursor: Cursor, end: number): string[] {
  const names: string[] = [];
  while (cursor.at < end) {
    const id = byte(cursor);
    const size = u32(cursor);
    const next = cursor.at + size;
    if (id === 1) {
      const count = u32(cursor);
      for (let i = 0; i < count; i += 1) {
        u32(cursor);
        names.push(name(cursor));
      }
    }
    cursor.at = next;
  }
  return names;
}

/**
 * The function names in a module's name section, or undefined when it has
 * none. The module is read only as far as its section layout; it is not
 * validated.
 */
export function functionNames(module: Uint8Array): string[] | undefined {
  const header = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
  if (header.some((value, index) => module[index] !== value)) {
    throw new Error("not a version 1 WebAssembly module");
  }
  const cursor: Cursor = { bytes: module, at: header.length };
  let names: string[] | undefined;
  while (cursor.at < module.length) {
    const id = byte(cursor);
    const size = u32(cursor);
    const end = cursor.at + size;
    if (end > module.length) throw new Error(`the module ends inside section ${id}`);
    if (id === 0 && name(cursor) === "name") names = functionNameSubsection(cursor, end);
    cursor.at = end;
  }
  return names;
}

/** A function the module must not carry, and the part of the library it comes from. */
export interface Offence {
  readonly part: string;
  readonly name: string;
}

/**
 * The names, among `names`, that sit under a forbidden path. A name can hold
 * several paths, as in a generic function's type arguments; the first names
 * the function's own module or its Self type, so it decides the part.
 */
export function forbiddenFunctions(names: readonly string[]): Offence[] {
  const offences: Offence[] = [];
  for (const name of names) {
    const plain = plainPath(name);
    let first: { part: string; at: number } | undefined;
    for (const { part, pattern } of matchers) {
      const at = plain.search(pattern);
      if (at >= 0 && (first === undefined || at < first.at)) first = { part, at };
    }
    if (first !== undefined) offences.push({ part: first.part, name: plain });
  }
  return offences;
}

const shown = 10;

function report(offences: readonly Offence[]): string {
  const lines = [`the module carries code it must not: ${offences.length} functions`];
  for (const { part } of FORBIDDEN) {
    const found = offences.filter((offence) => offence.part === part);
    if (found.length === 0) continue;
    lines.push(`${part}, ${found.length} functions:`);
    for (const { name } of found.slice(0, shown)) lines.push(`  ${name}`);
    if (found.length > shown) lines.push(`  and ${found.length - shown} more`);
  }
  return lines.join("\n");
}

// [spec:hfst:req:javascript.read-only-module]
// wasm-ld and wasm-bindgen keep only the functions the exports reach, so every
// function a module's name section lists is reachable code. A forbidden
// function LLVM inlines into a permitted one leaves no name of its own. The
// forbidden parts are large, so code that reaches one brings in functions
// that keep their names, and the suite also checks the unoptimised test
// build, which inlines next to nothing.
/**
 * Throws, naming the offending functions, when the module carries code from
 * the compilers, the foma backend, the tropical OpenFst backend or the
 * transducer algebra. Also throws when the module's names cannot show that,
 * as after wasm-opt strips them. Returns how many function names it checked.
 */
export function assertReadOnlyModule(module: Uint8Array): number {
  const names = functionNames(module);
  if (names === undefined) {
    throw new Error("the module has no name section to check: check it before wasm-opt strips it");
  }
  if (!names.some((name) => ownCode.test(plainPath(name)))) {
    throw new Error("the module's name section names no hfst_wasm function, so it cannot be checked");
  }
  const offences = forbiddenFunctions(names);
  if (offences.length > 0) throw new Error(report(offences));
  return names.length;
}
