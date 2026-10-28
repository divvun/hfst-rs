// Builds the two npm packages from the binding crates.
//
//   node js/build.mjs
//
// For each package this writes js/<package>/pkg (the wasm-bindgen output) and
// js/<package>/dist (the compiled wrapper, with a copy of pkg where the
// compiled binding module imports it from). It needs cargo with the
// wasm32-unknown-unknown target, the wasm-bindgen CLI at the version
// Cargo.lock pins, and npx. wasm-opt is used when it is on PATH.

import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const js = dirname(fileURLToPath(import.meta.url));
const root = dirname(js);

const packages = [
  { name: "@divvun/hfst-runtime", dir: "hfst-runtime", crate: "hfst-wasm-runtime", artifact: "hfst_wasm_runtime" },
  { name: "@divvun/hfst", dir: "hfst", crate: "hfst-wasm", artifact: "hfst_wasm" },
];

function run(command, args) {
  console.log(`$ ${command} ${args.join(" ")}`);
  execFileSync(command, args, { cwd: root, stdio: "inherit" });
}

function hasCommand(command) {
  const probe = spawnSync(command, ["--version"], { stdio: "ignore" });
  return probe.error === undefined && probe.status === 0;
}

// One cargo invocation per crate. Cargo unifies features across the packages
// of one invocation, which would build the runtime module with hfst's foma
// backend that the full crate turns on.
function cargoBuild(pkg) {
  run("cargo", [
    "build",
    "--target",
    "wasm32-unknown-unknown",
    "--profile",
    "wasm-release",
    "-p",
    pkg.crate,
  ]);
  return join(root, "target", "wasm32-unknown-unknown", "wasm-release", `${pkg.artifact}.wasm`);
}

function bindgen(wasm, outDir) {
  rmSync(outDir, { recursive: true, force: true });
  run("wasm-bindgen", ["--target", "web", "--out-dir", outDir, wasm]);
  const module = join(outDir, `${basename(wasm, ".wasm")}_bg.wasm`);
  if (hasCommand("wasm-opt")) run("wasm-opt", ["-Oz", module, "-o", module]);
  return module;
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

// [spec:hfst:req:javascript.typed-surface]
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

function buildPackage(pkg) {
  const dir = join(js, pkg.dir);
  const pkgDir = join(dir, "pkg");
  const dist = join(dir, "dist");
  const module = bindgen(cargoBuild(pkg), pkgDir);
  tsc(join(dir, "tsconfig.json"), dist);
  cpSync(pkgDir, join(dist, pkg.dir, "pkg"), { recursive: true });
  assertNoAny(dist);
  return { name: pkg.name, bytes: statSync(module).size };
}

// The tests import this module for its checks, so it builds only when run.
if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const sizes = packages.map(buildPackage);
  for (const { name, bytes } of sizes) {
    console.log(`${name}: ${bytes} bytes of WebAssembly (${(bytes / 1024).toFixed(1)} KiB)`);
  }
}
