// Runs the JavaScript test suite against the built package, under Node and
// Bun, and its parity scenarios, the main entry point's against the native
// library and the compile entry point's against the tools, under Node, Bun
// and Deno and in a headless Chromium (js/test/parity-runner.mjs).
//
//   node js/build.mjs --test && node js/test/run.mjs [--runtime node|bun|deno|chromium]... [--require-all]
//
// Deno and Chromium are skipped with a message when they are not installed.
// --require-all, or HFST_JS_TEST_REQUIRE_ALL=1, fails the run instead, and
// fails it when --runtime leaves any of the four out. HFST_JS_TEST_BROWSER
// names the browser to run.
//
// Each scenario variant runs in a process of its own, once per runtime,
// because a started or poisoned module instance cannot be reset. Scenarios
// that need the test hooks run against the test build in target/js-test; the
// rest run against the package as it would ship. A scenario runs against the
// main entry point unless it names the compile one. The transducer fixtures
// the test build writes to target/js-test/fixtures are read by path. The
// runner also serves each entry point's .wasm file over HTTP, so the suite
// can start either from an http: URL and from a fetched Response.

import { spawn } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { PARITY_ONLY, requireAll, runParityElsewhere } from "./parity-runner.mjs";

const js = dirname(dirname(fileURLToPath(import.meta.url)));
const testBuild = join(dirname(js), "target", "js-test");
const fixtures = `${pathToFileURL(join(testBuild, "fixtures")).href}/`;

// Each entry point's .wasm file, under the package's dist/.
const WASM = { main: "pkg/hfst_wasm_bg.wasm", compile: "pkg-compile/hfst_wasm_compile_bg.wasm" };

const INIT_VARIANTS = ["bytes", "file-url", "http-url", "response", "module", "default", "retry"];

const scenarios = [
  { file: "init.ts", build: "release", variants: INIT_VARIANTS },
  { file: "init.ts", build: "release", entry: "compile", variants: INIT_VARIANTS },
  { file: "contract.ts", build: "test", variants: ["main"] },
  { file: "compile-contract.ts", build: "release", entry: "compile", variants: ["main"] },
  { file: "compile-depth.ts", build: "release", entry: "compile", variants: ["main"] },
  { file: "poison.ts", build: "test", variants: ["function", "method"] },
  { file: "poison-isolation.ts", build: "test", variants: ["main", "compile"] },
  { file: "transducers.ts", build: "release", variants: ["main"] },
  { file: "lookup.ts", build: "release", variants: ["main"] },
  { file: "tokenise.ts", build: "release", variants: ["main"] },
  { file: "dhfst.ts", build: "release", variants: ["main"] },
  { file: "parity-runtime.ts", build: "release", variants: ["main"] },
  { file: "hfst-js-parity.ts", build: "release", variants: ["fixtures", "analysers"] },
  { file: "optimised.ts", build: "release", variants: ["main", "analysers"] },
  { file: "parity-runtime.ts", build: "release", entry: "compile", variants: ["compile"] },
];

// Node only collects garbage on request with --expose-gc; Bun has Bun.gc.
const runtimeFlags = { node: ["--expose-gc"] };

// Unit and package tests: every *.test.ts and *.test.mjs outside build output.
function unitTests(dir = js) {
  const found = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (!["dist", "pkg", "node_modules"].includes(entry.name)) found.push(...unitTests(path));
    } else if (/\.test\.m?[jt]s$/.test(entry.name)) {
      found.push(path);
    }
  }
  return found.sort();
}

function chosenRuntimes(argv) {
  const chosen = [];
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === "--runtime" && argv[i + 1] !== undefined) chosen.push(argv[(i += 1)]);
  }
  return chosen.length > 0 ? chosen : ["node", "bun", ...PARITY_ONLY];
}

function locate() {
  const release = join(js, "hfst", "dist");
  const test = join(testBuild, "dist");
  const manifest = JSON.parse(readFileSync(join(js, "hfst", "package.json"), "utf8"));
  const build = (dir, main, compile) => ({
    module: join(dir, main),
    main: { module: join(dir, main), wasm: join(dir, WASM.main) },
    compile: { module: join(dir, compile), wasm: join(dir, WASM.compile) },
  });
  return {
    name: manifest.name,
    version: manifest.version,
    release: build(release, "index.js", "compile.js"),
    test: build(test, "testing.js", "compile-testing.js"),
  };
}

/** Serves the release build's .wasm files, each at /<entry>.wasm. */
function serve(pkg) {
  const server = createServer((request, response) => {
    const entry = Object.keys(WASM).find((name) => request.url === `/${name}.wasm`);
    if (entry === undefined) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { "content-type": "application/wasm" });
    response.end(readFileSync(pkg.release[entry].wasm));
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => resolve(server));
  });
}

function runFile(runtime, path, args) {
  return new Promise((resolve) => {
    const flags = runtimeFlags[runtime] ?? [];
    const child = spawn(runtime, [...flags, path, ...args], { stdio: ["ignore", "inherit", "inherit"] });
    child.on("error", (error) => {
      console.log(`not ok - ${runtime} ${path}: cannot start ${runtime}: ${error.message}`);
      resolve(false);
    });
    child.on("close", (code) => resolve(code === 0));
  });
}

const pkg = locate();
const server = await serve(pkg);
const base = `http://127.0.0.1:${server.address().port}`;
let failures = 0;
let runs = 0;
const runtimes = chosenRuntimes(process.argv.slice(2));
for (const runtime of runtimes.filter((name) => !PARITY_ONLY.includes(name))) {
  for (const path of unitTests()) {
    runs += 1;
    if (!(await runFile(runtime, path, []))) failures += 1;
  }
  for (const { file, build, entry = "main", variants } of scenarios) {
    for (const variant of variants) {
      const label = `${pkg.name}${entry === "main" ? "" : "/compile"} ${file} ${variant}`;
      const entryArgs = (name) => ({
        module: pathToFileURL(pkg[build][name].module).href,
        wasm: pathToFileURL(pkg[build][name].wasm).href,
        http: `${base}/${name}.wasm`,
      });
      const args = {
        ...entryArgs(entry),
        entries: { main: entryArgs("main"), compile: entryArgs("compile") },
        package: pkg.name,
        version: pkg.version,
        fixtures,
        variant,
        label,
      };
      const passed = await runFile(runtime, join(js, "test", file), [JSON.stringify(args)]);
      runs += 1;
      if (!passed) failures += 1;
    }
  }
}
const required = requireAll(process.argv.slice(2));
const parity = await runParityElsewhere({ runtimes, required, pkg, testBuild });
runs += parity.runs;
failures += parity.failures;
server.close();
console.log(`${runs - failures} of ${runs} test files passed`);
if (failures > 0) process.exitCode = 1;
