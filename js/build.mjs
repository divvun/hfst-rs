// Builds the npm package @divvun/hfst from crates/hfst-wasm.
//
//   node js/build.mjs          the package
//   node js/build.mjs --test   the package, then a test build of it
//
// This writes js/hfst/pkg (the wasm-bindgen output) and js/hfst/dist (the
// compiled wrapper, with a copy of pkg where the compiled binding module
// imports it from). It needs cargo with the wasm32-unknown-unknown target, the
// wasm-bindgen CLI at the version Cargo.lock pins, and npx. wasm-opt is used
// when it is on PATH.
//
// A test build turns on the crate's test-hooks feature and adds the testing
// entry module. It is written to target/js-test, never to js/, so the hooks
// cannot reach the published package. It also builds the hfst tools and
// writes the transducer fixtures to target/js-test/fixtures.

import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { buildFixtures, buildTools } from "./test/fixtures.mjs";

const js = dirname(fileURLToPath(import.meta.url));
const root = dirname(js);
const packageDir = join(js, "hfst");
const testBuild = join(root, "target", "js-test");

function run(command, args) {
  console.log(`$ ${command} ${args.join(" ")}`);
  execFileSync(command, args, { cwd: root, stdio: "inherit" });
}

function hasCommand(command) {
  const probe = spawnSync(command, ["--version"], { stdio: "ignore" });
  return probe.error === undefined && probe.status === 0;
}

// A test build uses the dev profile, so it never overwrites the release module.
function cargoBuild(testHooks) {
  const profile = testHooks ? "dev" : "wasm-release";
  const features = testHooks ? ["--features", "test-hooks"] : [];
  run("cargo", ["build", "--target", "wasm32-unknown-unknown", "--profile", profile, "-p", "hfst-wasm", ...features]);
  const profileDir = testHooks ? "debug" : "wasm-release";
  return join(root, "target", "wasm32-unknown-unknown", profileDir, "hfst_wasm.wasm");
}

function bindgen(wasm, outDir, optimise) {
  rmSync(outDir, { recursive: true, force: true });
  run("wasm-bindgen", ["--target", "web", "--out-dir", outDir, wasm]);
  const module = join(outDir, `${basename(wasm, ".wasm")}_bg.wasm`);
  if (optimise && hasCommand("wasm-opt")) run("wasm-opt", ["-Oz", module, "-o", module]);
  return module;
}

// The release build never turns test-hooks on; this makes that a checked fact.
function assertNoTestHooks(pkgDir) {
  const glue = readFileSync(join(pkgDir, "hfst_wasm.d.ts"), "utf8");
  if (glue.includes("testPanic")) throw new Error("the release glue has test hooks");
}

function tsc(project, outDir) {
  rmSync(outDir, { recursive: true, force: true });
  run("npx", ["-y", "-p", "typescript@5", "tsc", "-p", project, "--outDir", outDir]);
}

function* declarations(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      if (entry !== "pkg") yield* declarations(path);
    } else if (entry.endsWith(".d.ts")) {
      yield path;
    }
  }
}

// [spec:hfst:req:javascript.typed-surface+1]
// The wrapper's declarations are the public API, so none of them may use any.
// The generated glue under pkg is internal and is not checked.
export function assertNoAny(dist) {
  const offenders = [];
  for (const file of declarations(dist)) {
    const code = readFileSync(file, "utf8")
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/\/\/.*$/gm, "");
    if (/\bany\b/.exec(code) !== null) offenders.push(file);
  }
  if (offenders.length > 0) {
    throw new Error(`declarations use any: ${offenders.join(", ")}`);
  }
}

/** Builds the package and returns the size of its .wasm file in bytes. */
function buildPackage() {
  const pkgDir = join(packageDir, "pkg");
  const dist = join(packageDir, "dist");
  const module = bindgen(cargoBuild(false), pkgDir, true);
  assertNoTestHooks(pkgDir);
  tsc(join(packageDir, "tsconfig.json"), dist);
  cpSync(pkgDir, join(dist, "pkg"), { recursive: true });
  assertNoAny(dist);
  return statSync(module).size;
}

// tsc type-checks the testing entry against the release glue in js/hfst/pkg,
// so the release build must come first. The test hooks are looked up at run
// time, which the release glue's types do not need to know about.
function buildTestPackage() {
  const dist = join(testBuild, "dist");
  tsc(join(packageDir, "tsconfig.test.json"), dist);
  bindgen(cargoBuild(true), join(dist, "pkg"), false);
}

// The tests import this module for its checks, so it builds only when run.
if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const bytes = buildPackage();
  if (process.argv.includes("--test")) {
    buildTestPackage();
    buildFixtures(buildTools(root, run), join(testBuild, "fixtures"));
  }
  console.log(`@divvun/hfst: ${bytes} bytes of WebAssembly (${(bytes / 1024).toFixed(1)} KiB)`);
}
