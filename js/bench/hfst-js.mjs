// hfst-js, resolved from this directory, where package.json pins it. The
// tests and the benchmark import it through this module, so nothing outside
// js/bench depends on it and the published package never does.
export { Analyzer, HfstInputStream } from "hfst-js";
