// Benchmarks @divvun/hfst against hfst-js 1.4.0 and writes js/hfst/BENCHMARKS.md.
//
//   HFST_JS_PARITY=a.hfstol=a-words.txt:b.hfstol=b-words.txt \
//     node js/bench/bench.mjs [--notes FILE] [--runtime node] [--runtime bun]
//   node js/bench/bench.mjs --render [--notes FILE]
//
// It runs on the transducers and word lists the real-analyser parity check
// reads (js/test/hfst-js-parity.ts), each transducer in a fresh process per
// runtime (js/bench/measure.mjs), three times over, and each figure is the
// median of the three. Each runtime's figures are kept in
// target/hfst-js-bench, so the runtimes can be measured one run at a time;
// BENCHMARKS.md shows every runtime kept there, and --render rewrites it from
// them without measuring. --notes names a Markdown file
// saying where the word lists came from, which goes into BENCHMARKS.md as it
// is. It needs `node js/build.mjs` and `npm ci` in js/bench first.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { cpus, loadavg, platform, release, totalmem } from "node:os";
import { basename, delimiter, dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const bench = dirname(fileURLToPath(import.meta.url));
const js = dirname(bench);
const root = dirname(js);

const PROCESSES = 3;
const LOADS = 7;
const PASSES = 3;
const SAMPLE = 500;

function options(argv) {
  const runtimes = [];
  let notes = null;
  let render = false;
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === "--runtime" && argv[i + 1] !== undefined) runtimes.push(argv[(i += 1)]);
    else if (argv[i] === "--notes" && argv[i + 1] !== undefined) notes = argv[(i += 1)];
    else if (argv[i] === "--render") render = true;
    else throw new Error(`unknown argument ${argv[i]}`);
  }
  if (render) return { runtimes: [], notes };
  return { runtimes: runtimes.length > 0 ? runtimes : ["node", "bun"], notes };
}

function pairs() {
  const listed = process.env.HFST_JS_PARITY;
  if (listed === undefined || listed === "") throw new Error("HFST_JS_PARITY lists no transducer=words pairs");
  return listed
    .split(delimiter)
    .filter((entry) => entry !== "")
    .map((entry) => {
      const [transducer, words] = entry.split("=");
      if (transducer === undefined || words === undefined) throw new Error(`not transducer=words: ${entry}`);
      return { transducer, words };
    });
}

/** A transducer's name in the tables: its language repository, or else its directory, and its file name. */
export function label(path) {
  const lang = /\/(lang-[^/]+)\//.exec(path);
  return `${lang === null ? basename(dirname(path)) : lang[1]} ${basename(path, ".hfstol")}`;
}

function command(file, args) {
  try {
    return execFileSync(file, args, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return null;
  }
}

function machine() {
  const cpu = cpus()[0]?.model ?? "unknown CPU";
  const memory = `${Math.round(totalmem() / 2 ** 30)} GiB`;
  const macos = platform() === "darwin" ? command("sw_vers", ["-productVersion"]) : null;
  const os = macos === null ? `${platform()} ${release()}` : `macOS ${macos}`;
  return `${cpu}, ${cpus().length} cores, ${memory}, ${os}`;
}

const ms = (value) => (value < 10 ? value.toFixed(2) : value.toFixed(1));
const us = (value) => (value < 10 ? value.toFixed(2) : value.toFixed(0));
const ratio = (ours, theirs) => `${(ours / theirs).toFixed(2)}×`;
const mib = (bytes) => `${(bytes / 2 ** 20).toFixed(1)} MiB`;

function table(results) {
  const lines = [
    "| Transducer | Size | Words timed | Load, hfst-js (ms) | Load, ours (ms) | Ours ÷ hfst-js | Lookup, hfst-js (µs/word) | Lookup, ours (µs/word) | Ours ÷ hfst-js | Lookup, hfst-js cached (µs/word) |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
  ];
  for (const r of results) {
    lines.push(
      `| ${label(r.transducer)} | ${mib(r.bytes)} | ${r.words} | ${ms(r.loadMs.hfstJs)} | ${ms(r.loadMs.ours)} | ${ratio(r.loadMs.ours, r.loadMs.hfstJs)} | ${us(r.lookupUs.hfstJs)} | ${us(r.lookupUs.ours)} | ${ratio(r.lookupUs.ours, r.lookupUs.hfstJs)} | ${us(r.lookupUs.hfstJsCached)} |`,
    );
  }
  return lines.join("\n");
}

const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

/** The figures of `samples`, one per process, with each timing the median of theirs. */
export function combined(samples) {
  const [first] = samples;
  const of = (pick) => median(samples.map(pick));
  return {
    ...first,
    initMs: of((s) => s.initMs),
    loadMs: { ours: of((s) => s.loadMs.ours), hfstJs: of((s) => s.loadMs.hfstJs) },
    lookupUs: {
      ours: of((s) => s.lookupUs.ours),
      hfstJs: of((s) => s.lookupUs.hfstJs),
      hfstJsCached: of((s) => s.lookupUs.hfstJsCached),
    },
  };
}

/** Plain sentences on where ours is slower or faster, from the figures. */
export function verdict(results) {
  const times = (x) => `${x.toFixed(1)} times`;
  const load = results.map((r) => r.loadMs.ours / r.loadMs.hfstJs);
  const sentences = [];
  const slowerLoads = load.filter((x) => x > 1);
  sentences.push(
    slowerLoads.length === load.length
      ? `@divvun/hfst loads every transducer more slowly than hfst-js, taking ${Math.min(...load).toFixed(1)} to ${times(Math.max(...load))} as long.`
      : `@divvun/hfst loads ${slowerLoads.length} of ${load.length} transducers more slowly than hfst-js, taking up to ${times(Math.max(...load))} as long.`,
  );
  const slower = results.filter((r) => r.lookupUs.ours > r.lookupUs.hfstJs);
  const faster = results.filter((r) => r.lookupUs.ours <= r.lookupUs.hfstJs);
  const lookup = (r) => r.lookupUs.ours / r.lookupUs.hfstJs;
  if (slower.length > 0) {
    sentences.push(
      `It looks words up more slowly than uncached hfst-js on ${slower.length} of ${results.length} transducers, taking up to ${times(Math.max(...slower.map(lookup)))} as long.`,
    );
  }
  if (faster.length > 0) {
    const labels = faster.map((r) => label(r.transducer));
    const names = labels.length === 1 ? labels[0] : `${labels.slice(0, -1).join(", ")} and ${labels.at(-1)}`;
    sentences.push(
      `It is faster on ${faster.length === results.length ? "every transducer" : names}, in as little as ${times(Math.min(...faster.map(lookup)))} the time.`,
    );
  }
  return sentences.join(" ");
}

function main(argv) {
  const { runtimes, notes } = options(argv);
  const kept = join(root, "target", "hfst-js-bench");
  mkdirSync(kept, { recursive: true });
  const commit = command("git", ["-C", root, "rev-parse", "--short", "HEAD"]) ?? "unknown";
  for (const runtime of runtimes) {
    const results = [];
    const loads = [];
    for (const { transducer, words } of pairs()) {
      const settings = JSON.stringify({ transducer, words, loads: LOADS, passes: PASSES, sample: SAMPLE });
      const samples = [];
      for (let i = 0; i < PROCESSES; i += 1) {
        loads.push(loadavg()[0]);
        const printed = execFileSync(runtime, [join(bench, "measure.mjs"), settings], { encoding: "utf8" });
        samples.push(JSON.parse(printed.trim().split("\n").at(-1)));
      }
      const result = combined(samples);
      console.log(`${runtime} ${label(transducer)}: ${JSON.stringify(result.loadMs)} ${JSON.stringify(result.lookupUs)}`);
      results.push(result);
    }
    const run = {
      runtime,
      version: command(runtime, ["--version"]) ?? "unknown",
      date: new Date().toISOString().slice(0, 10),
      machine: machine(),
      commit,
      wasmBytes: statSync(join(js, "hfst", "dist", "pkg", "hfst_wasm_bg.wasm")).size,
      loadAverage: [Math.min(...loads), Math.max(...loads)],
      results,
    };
    writeFileSync(join(kept, `${runtime}.json`), `${JSON.stringify(run, null, 2)}\n`);
  }

  const runtimeNames = { node: "Node", bun: "Bun" };
  const runs = ["node", "bun"]
    .filter((runtime) => existsSync(join(kept, `${runtime}.json`)))
    .map((runtime) => JSON.parse(readFileSync(join(kept, `${runtime}.json`), "utf8")));
  const name = (run) => `${runtimeNames[run.runtime] ?? run.runtime} ${run.version}`;
  const initText = runs
    .map((run) => {
      return `${ms(median(run.results.map((r) => r.initMs)))} ms under ${runtimeNames[run.runtime] ?? run.runtime}`;
    })
    .join(" and ");
  const measured = runs
    .map((run) => `${name(run)} on ${run.date}, with @divvun/hfst built from commit ${run.commit} (${(run.wasmBytes / 1024).toFixed(1)} KiB of WebAssembly)`)
    .join("; ");
  const machines = [...new Set(runs.map((run) => run.machine))].join("; ");
  const loadText = runs
    .map((run) => `${run.loadAverage.map((x) => x.toFixed(1)).join(" to ")} under ${runtimeNames[run.runtime] ?? run.runtime}`)
    .join(" and ");

  const sections = runs.map(
    (run) => `## ${name(run)}

${table(run.results)}

${verdict(run.results)}`,
  );

  const markdown = `# @divvun/hfst against hfst-js

@divvun/hfst replaces [hfst-js](https://www.npmjs.com/package/hfst-js) 1.4.0. These figures compare the two on the same transducers, word lists and machine. \`node js/bench/bench.mjs\` measured them and wrote this file.

Machine: ${machines}. Measured under ${measured}. hfst-js is 1.4.0.

## Method

- Each transducer is measured in ${PROCESSES} fresh processes per runtime, and each figure is the median of the ${PROCESSES}. The machine was shared with other work: its one-minute load average was ${loadText}. Load slows both libraries; the ratios, from the two taking turns in one process, move less than the times.
- **Load** is the time from a transducer's bytes in memory to a transducer ready to look up: \`readTransducer(bytes)\` in @divvun/hfst and \`new HfstInputStream(bytes, false).read()\` in hfst-js. In each process it is the median of ${LOADS} loads, the two libraries taking turns. @divvun/hfst is measured after \`init\` has compiled its WebAssembly module, which happens once per process and took a median of ${initText}.
- **Lookup** is the time per word over the first ${SAMPLE} words of the transducer's word list (fewer when the list is shorter), in each process the median of ${PASSES} passes after one untimed pass. hfst-js runs with its result cache off, so every lookup searches. @divvun/hfst keeps no result cache. **hfst-js cached** is the time per word when every word is already in hfst-js's cache, for context: it is the cost of a repeated word.
- Neither library is tuned for the benchmark.

${sections.join("\n\n")}
${notes === null ? "" : `\n## Word lists\n\n${readFileSync(notes, "utf8").trim()}\n`}`;

  const out = join(js, "hfst", "BENCHMARKS.md");
  writeFileSync(out, markdown);
  console.log(`wrote ${out}`);
}

// The tests import this module for its functions, so it runs only when run.
if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2));
}
