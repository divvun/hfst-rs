// Checks the npm package must pass, run against its built package directory.

import { assert, assertEqual, readBytes, test } from "./harness.ts";

/** The fields of package.json these checks read. */
interface Manifest {
  name: string;
  version: string;
  description: string;
  license: string;
  author: string;
  type: string;
  files: string[];
  exports: Record<"." | "./compile", { types: string; default: string }>;
}

interface NodeFs {
  readdir(path: URL, options: { recursive: true }): Promise<string[]>;
}

async function readManifest(dir: URL): Promise<Manifest> {
  const bytes = await readBytes(new URL("package.json", dir).href);
  return JSON.parse(new TextDecoder().decode(bytes));
}

async function listFiles(dir: URL): Promise<string[]> {
  const specifier = "node:fs/promises";
  const fs: NodeFs = await import(specifier);
  return await fs.readdir(dir, { recursive: true });
}

/** The entry module `entry` (`.` or `./compile`) of the built package in `dir`. */
export async function entryOf(dir: URL, entry: "." | "./compile" = "."): Promise<Record<string, unknown>> {
  const manifest = await readManifest(dir);
  return await import(new URL(manifest.exports[entry].default, dir).href);
}

/** Registers the checks for the package in `dir`, which must be called `name`. */
export function packageTests(dir: URL, name: string): void {
  test("package.json names Divvun HFST, UiT and the licence", async () => {
    const manifest = await readManifest(dir);
    assertEqual(manifest.name, name, "name");
    assertEqual(manifest.version, "0.2.0", "version");
    assertEqual(manifest.license, "LGPL-3.0-or-later", "license");
    assertEqual(manifest.author, "UiT The Arctic University of Norway", "author");
    assertEqual(manifest.type, "module", "type");
    assert(manifest.description.includes("Divvun HFST"), "the description names Divvun HFST");
    assertEqual(manifest.files.join(" "), "dist MIGRATING.md BENCHMARKS.md", "files");
  });

  test("the package ships its README, its notes and both licence texts", async () => {
    const manifest = await readManifest(dir);
    const files = await listFiles(dir);
    for (const path of ["README.md", ...manifest.files.filter((file) => file !== "dist")]) {
      assert(files.includes(path), `${path} exists`);
    }
    const dist = await listFiles(new URL("dist/", dir));
    for (const licence of ["COPYING", "COPYING.LESSER"]) {
      assert(dist.includes(licence), `dist/${licence} exists`);
    }
  });

  // [spec:hfst:req:javascript.typed-surface+1/test]
  // [spec:hfst:def:javascript.packages+3/test]
  test("each entry module and its declarations are where the exports map says", async () => {
    const manifest = await readManifest(dir);
    const files = await listFiles(dir);
    assertEqual(Object.keys(manifest.exports).join(" "), ". ./compile", "the entry points");
    for (const entry of Object.values(manifest.exports)) {
      for (const path of [entry.default, entry.types]) {
        assert(files.includes(path.replace(/^\.\//, "")), `${path} exists`);
      }
    }
  });

  // [spec:hfst:def:javascript.packages+3/test]
  test("each entry point ships a .wasm file of its own, beside its entry module", async () => {
    const manifest = await readManifest(dir);
    const wasm = (await listFiles(new URL("dist/", dir))).filter((file) => file.endsWith(".wasm")).sort();
    assertEqual(wasm.join(" "), "pkg-compile/hfst_wasm_compile_bg.wasm pkg/hfst_wasm_bg.wasm", ".wasm files");
    for (const entry of Object.values(manifest.exports)) {
      const entryDir = entry.default.replace(/^\.\/dist\//, "").replace(/[^/]*$/, "");
      assert(wasm.every((file) => file.startsWith(entryDir)), `the .wasm files sit under ${entryDir}`);
    }
  });
}
