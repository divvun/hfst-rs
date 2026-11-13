// Builds the npm package @divvun/hfst from crates/hfst-wasm, behind the main
// entry point, and crates/hfst-wasm-compile, behind @divvun/hfst/compile.
//
//   node js/build.mjs          the package
//   node js/build.mjs --test   the package, then a test build of it
//
// This writes js/hfst/pkg and js/hfst/pkg-compile (the wasm-bindgen output of
// each module) and js/hfst/dist (the compiled wrapper, with a copy of both
// where the compiled binding modules import them from). It needs cargo with
// the wasm32-unknown-unknown target, the wasm-bindgen CLI at the version the
// crates pin, and npx. wasm-opt is used when it is on PATH.
//
// The release build fails when the main module carries code from the
// compilers, the foma backend, the tropical OpenFst backend or the transducer
// algebra. The check reads the module's function names, which wasm-opt
// strips, so it runs on a copy taken before wasm-opt, kept at target/js-check
// for the suite. The compile module carries all of them, and is not checked.
//
// A test build turns on the crates' test-hooks feature and adds the testing
// entry modules. It is written to target/js-test, never to js/, so the hooks
// cannot reach the published package. It also builds the hfst tools, writes
// the transducer fixtures to target/js-test/fixtures, installs hfst-js for the
// suite's comparison with it, and compiles the parity scenarios for browsers
// to target/js-test/browser.

import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { buildFixtures, buildTools } from "./test/fixtures.mjs";
import { installHfstJs } from "./test/hfst-js-fixtures.mjs";
import { assertReadOnlyModule } from "./test/read-only-module.ts";

const js = dirname(fileURLToPath(import.meta.url));
const root = dirname(js);
const packageDir = join(js, "hfst");
const testBuild = join(root, "target", "js-test");
const checkBuild = join(root, "target", "js-check");

/** The package's two modules: the crate each is built from, and the directory its glue goes to. */
const MODULES = {
  main: { crate: "hfst-wasm", out: "pkg" },
  compile: { crate: "hfst-wasm-compile", out: "pkg-compile" },
};

function run(command, args) {
  console.log(`$ ${command} ${args.join(" ")}`);
  execFileSync(command, args, { cwd: root, stdio: "inherit" });
}

function hasCommand(command) {
  const probe = spawnSync(command, ["--version"], { stdio: "ignore" });
  return probe.error === undefined && probe.status === 0;
}

// The CLI writes glue for its own version of the wasm-bindgen ABI, so it must
// be the version every crate the modules are built from pins.
function assertBindgenVersion() {
  const installed = execFileSync("wasm-bindgen", ["--version"], { encoding: "utf8" }).trim().split(" ")[1];
  for (const crate of ["hfst-wasm", "hfst-wasm-compile", "hfst-wasm-shared"]) {
    const manifest = readFileSync(join(root, "crates", crate, "Cargo.toml"), "utf8");
    const pinned = /^wasm-bindgen = "=([^"]+)"$/m.exec(manifest)?.[1];
    if (pinned === undefined) throw new Error(`crates/${crate}/Cargo.toml does not pin wasm-bindgen`);
    if (installed !== pinned) throw new Error(`the wasm-bindgen CLI is ${installed}; ${crate} pins ${pinned}`);
  }
}

// A test build uses the dev profile, so it never overwrites the release
// module. Each crate builds on its own, so the compile module's features never
// reach the main module's dependencies.
function cargoBuild(crate, testHooks) {
  const profile = testHooks ? "dev" : "wasm-release";
  const features = testHooks ? ["--features", "test-hooks"] : [];
  run("cargo", ["build", "--target", "wasm32-unknown-unknown", "--profile", profile, "-p", crate, ...features]);
  const profileDir = testHooks ? "debug" : "wasm-release";
  return join(root, "target", "wasm32-unknown-unknown", profileDir, `${crate.replaceAll("-", "_")}.wasm`);
}

function bindgen(wasm, outDir) {
  rmSync(outDir, { recursive: true, force: true });
  run("wasm-bindgen", ["--target", "web", "--out-dir", outDir, wasm]);
  return join(outDir, `${basename(wasm, ".wasm")}_bg.wasm`);
}

function assertReadOnly(module) {
  rmSync(checkBuild, { recursive: true, force: true });
  mkdirSync(checkBuild, { recursive: true });
  const named = join(checkBuild, basename(module));
  cpSync(module, named);
  const checked = assertReadOnlyModule(readFileSync(named));
  console.log(`read-only module: none of ${checked} functions comes from a compiler, foma, OpenFst or the algebra`);
}

// The release build never turns test-hooks on; this makes that a checked fact.
function assertNoTestHooks(pkgDir, crate) {
  const glue = readFileSync(join(pkgDir, `${crate.replaceAll("-", "_")}.d.ts`), "utf8");
  if (glue.includes("testPanic")) throw new Error(`the release glue of ${crate} has test hooks`);
}

function tsc(project, outDir) {
  rmSync(outDir, { recursive: true, force: true });
  run("npx", ["-y", "-p", "typescript@5", "tsc", "-p", project, "--outDir", outDir]);
}

function* declarations(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      if (!Object.values(MODULES).some(({ out }) => out === entry)) yield* declarations(path);
    } else if (entry.endsWith(".d.ts")) {
      yield path;
    }
  }
}

// [spec:hfst:req:javascript.typed-surface+1]
// The wrapper's declarations are the public API, so none of them may use any.
// The generated glue under pkg and pkg-compile is internal and is not checked.
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

/** Builds the package and returns the size of each module's .wasm file in bytes. */
function buildPackage() {
  assertBindgenVersion();
  const dist = join(packageDir, "dist");
  const modules = {};
  for (const [name, { crate, out }] of Object.entries(MODULES)) {
    const pkgDir = join(packageDir, out);
    modules[name] = { crate, pkgDir, module: bindgen(cargoBuild(crate, false), pkgDir) };
  }
  assertReadOnly(modules.main.module);
  for (const { crate, pkgDir, module } of Object.values(modules)) {
    if (hasCommand("wasm-opt")) run("wasm-opt", ["-Oz", module, "-o", module]);
    assertNoTestHooks(pkgDir, crate);
  }
  tsc(join(packageDir, "tsconfig.json"), dist);
  for (const { out } of Object.values(MODULES)) {
    cpSync(join(packageDir, out), join(dist, out), { recursive: true });
  }
  // The library is LGPLv3, which is the GPLv3 with extra permissions, so the
  // package carries both texts.
  cpSync(join(root, "COPYING"), join(dist, "COPYING.LESSER"));
  cpSync(join(root, "crates", "hfst-cli", "COPYING"), join(dist, "COPYING"));
  assertNoAny(dist);
  return Object.fromEntries(Object.entries(modules).map(([name, { module }]) => [name, statSync(module).size]));
}

// tsc type-checks the testing entries against the release glue in js/hfst/pkg
// and js/hfst/pkg-compile, so the release build must come first. The test hooks are looked up at run
// time, which the release glue's types do not need to know about.
function buildTestPackage() {
  const dist = join(testBuild, "dist");
  tsc(join(packageDir, "tsconfig.test.json"), dist);
  for (const { crate, out } of Object.values(MODULES)) {
    bindgen(cargoBuild(crate, true), join(dist, out));
  }
  // A browser runs no TypeScript, so the parity scenarios it runs are compiled.
  tsc(join(js, "test", "tsconfig.browser.json"), join(testBuild, "browser"));
}

// The tests import this module for its checks, so it builds only when run.
if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const sizes = buildPackage();
  if (process.argv.includes("--test")) {
    buildTestPackage();
    buildFixtures(buildTools(root, run), join(testBuild, "fixtures"));
    installHfstJs();
  }
  for (const [name, bytes] of Object.entries(sizes)) {
    const entry = name === "main" ? "@divvun/hfst" : `@divvun/hfst/${name}`;
    console.log(`${entry}: ${bytes} bytes of WebAssembly (${(bytes / 1024).toFixed(1)} KiB)`);
  }
}
