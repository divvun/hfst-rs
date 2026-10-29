// Runs the JavaScript test suite against the built packages, under Node and
// Bun.
//
//   node js/build.mjs --test && node js/test/run.mjs [--runtime node] [--runtime bun]
//
// Each scenario variant runs in a process of its own, once per runtime and
// package, because a started or poisoned module instance cannot be reset.
// Scenarios that need the test hooks run against the test build in
// target/js-test; the rest run against the packages as they would ship. A
// scenario marked `only` runs against that one package. The transducer
// fixtures the test build writes to target/js-test/fixtures are read by path.
// The runner also serves each package's .wasm file over HTTP, so the suite can
// start a package from an http: URL and from a fetched Response.

import { spawn } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const js = dirname(dirname(fileURLToPath(import.meta.url)));
const testBuilds = join(dirname(js), "target", "js-test");
const fixtures = `${pathToFileURL(join(testBuilds, "fixtures")).href}/`;

const packages = [
  { name: "@divvun/hfst-runtime", dir: "hfst-runtime", artifact: "hfst_wasm_runtime" },
  { name: "@divvun/hfst", dir: "hfst", artifact: "hfst_wasm" },
];

const scenarios = [
  {
    file: "init.ts",
    build: "release",
    variants: ["bytes", "file-url", "http-url", "response", "module", "default", "retry"],
  },
  { file: "contract.ts", build: "test", variants: ["main"] },
  { file: "poison.ts", build: "test", variants: ["function", "method"] },
  { file: "transducers.ts", build: "release", variants: ["main"] },
  { file: "transducers-full.ts", build: "release", variants: ["main"], only: "@divvun/hfst" },
  { file: "lookup.ts", build: "release", variants: ["main"] },
  { file: "lookup-full.ts", build: "release", variants: ["main"], only: "@divvun/hfst" },
  { file: "tokenise.ts", build: "release", variants: ["main"] },
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
  return chosen.length > 0 ? chosen : ["node", "bun"];
}

function locate(pkg) {
  const release = join(js, pkg.dir, "dist", pkg.dir);
  const test = join(testBuilds, pkg.dir, "dist", pkg.dir);
  const manifest = JSON.parse(readFileSync(join(js, pkg.dir, "package.json"), "utf8"));
  return {
    ...pkg,
    version: manifest.version,
    release: { module: join(release, "index.js"), wasm: join(release, "pkg", `${pkg.artifact}_bg.wasm`) },
    test: { module: join(test, "testing.js"), wasm: join(test, "pkg", `${pkg.artifact}_bg.wasm`) },
  };
}

function serve(located) {
  const server = createServer((request, response) => {
    const pkg = located.find((p) => request.url === `/${p.artifact}_bg.wasm`);
    if (pkg === undefined) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { "content-type": "application/wasm" });
    response.end(readFileSync(pkg.release.wasm));
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

const located = packages.map(locate);
const server = await serve(located);
const base = `http://127.0.0.1:${server.address().port}`;
let failures = 0;
let runs = 0;
for (const runtime of chosenRuntimes(process.argv.slice(2))) {
  for (const path of unitTests()) {
    runs += 1;
    if (!(await runFile(runtime, path, []))) failures += 1;
  }
  for (const pkg of located) {
    for (const { file, build, variants, only } of scenarios) {
      if (only !== undefined && only !== pkg.name) continue;
      for (const variant of variants) {
        const label = `${pkg.name} ${file} ${variant}`;
        const args = {
          module: pathToFileURL(pkg[build].module).href,
          wasm: pathToFileURL(pkg[build].wasm).href,
          http: `${base}/${pkg.artifact}_bg.wasm`,
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
}
server.close();
console.log(`${runs - failures} of ${runs} test files passed`);
if (failures > 0) process.exitCode = 1;
