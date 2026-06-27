# Wave-2 remaining work — execution plan (clean-context handoff)

Pick this up cold. Goal: finish the **in-scope function/method gaps** of the
C++ -> Rust Wave-2 port (the `literal-port` nplan node).

## Current state (regenerate live numbers before starting)

- Last commit referenced: `a17c58c4`. Run `mcp__nplan__nplan_port_check` (wave 2)
  or `nplan port status` for live coverage. At handoff: **2221 / 2593 ported (85.7%)**.
- Wave-1 markup is 100%. Wave-2 translate is the open gate.
- Build: `cargo build -p hfst`. Tests: **`cargo nextest run -p hfst`** (NOT plain
  `cargo test` — see guardrails). The fork is the `rustfst` git submodule.

## The target: 105 in-scope function/method gaps

Snapshot in `docs/wave2-gap-list.txt` (regenerate with the script below; the list
shifts as work lands). Split:

- **22 `IMPL?`** — a `fn <name>` already exists in `crates/hfst/src`. Almost
  certainly **mis-annotated** (the Rust impl exists; the manifest id just is not
  on it). Action: find the right fn and add the annotation. Cheap.
- **83 `PORT`** — no matching `fn`. Likely **genuinely unported**. Action: port
  the C++ function 1:1 into the corresponding Rust module and annotate. Real work.

By file (PORT unless noted): pmatch_utils 19 (+9 IMPL?), HfstTransitionGraph 16
(+3 IMPL?), xre_utils 9, transducer 6, lexc-utils 5, XfstCompiler 4,
HfstTransducer 4, PmatchCompiler 3 (+3 IMPL?), HfstSymbolDefs 3,
ConvertTransducerFormat 3, LexcCompiler 2, HfstStrings2FstTokenizer 2,
ConvertOlTransducer 2, convert 2, plus singles.

> Note: `ConvertTransducerFormat`/`ConvertOlTransducer` `hfst_transducer_to_hfst_ol`
> / `hfst_ol_to_hfst_transducer` are the generic conversion-bridge decls; check
> whether a concrete impl is already credited under a different id (decl/def
> sibling) before porting — likely a credit, not a port.

## Method, per symbol

1. Regenerate the list (script below) — `IMPL?` vs `PORT`.
2. **IMPL?**: locate `fn <name>` in the Rust file that corresponds to the C++ file
   (e.g. `HfstTransitionGraph.h` -> `hfst_basic_transducer.rs`, `pmatch_utils.*`
   -> `pmatch_compiler.rs`/`pmatch.rs`, `xre_utils.cc` -> `xre.rs`). If the fn
   genuinely implements that manifest symbol, add the two comment lines directly
   above the `fn` (matching its indentation):
   `// [spec:hfst:def:<id>]` and `// [spec:hfst:sem:<id>]`.
   ONLY credit when the file corresponds AND the name is unique in that file —
   loose cross-file name matching produces false credits (verified: `getline`,
   `parse`, `push_back`, `getinput` all matched wrong symbols; discarded).
3. **PORT**: read the C++ (`file` + `name` in the manifest row) and translate it
   1:1 into the corresponding Rust module, carrying the `def`+`sem` annotation.
   Faithful Wave-2 rules apply (mirror control flow; `throw` -> `panic_any`;
   `std::set` -> `BTreeSet`; `arc` -> `transition`; renamed overloads; unsafe/raw
   pointers OK). Build until clean.
4. Commit per coherent batch via
   `mcp__nplan__nplan_commit ids:["literal-port"] type:"progress"` with violations
   `["file-size=...","function-density=...","duplication=...","complexity=..."]`
   for the big faithful files (and `stub-found=...` if you leave an honest
   `unimplemented!`/`#[ignore]`).
5. After each batch: `nplan_port_check` (wave 2) to confirm the number moved.

## Guardrails (do not skip)

- **NEVER put a backtick in any comment.** The nplan target scan zeroes ALL of a
  file's annotation credit if a single backtick appears in any comment line.
  String-literal backticks are fine. After editing, `grep -c '\x60'` a touched
  file and confirm new comments are backtick-free. (Memory: `nplan-target-scan-no-backticks`.)
- **Run tests with `cargo nextest run`** (per-process isolation, like the C++
  tests). Plain `cargo test` shares one process and masks an order-sensitivity in
  the global symbol-number tables. (Memory: `compare-order-sensitivity-bug`.)
- Edition 2024: `#[unsafe(no_mangle)]`; bind `&mut *ptr` before indexing a raw
  deref; copy a `static mut` to a local before `format!`.
- Do not credit C++ STL iterator/container boilerplate (`begin`/`end`/`operator++`
  on `ConstContainerIterator`/`VariableBlock`/`VariableContainer`, the TWOLC
  `where`-clause classes) — Rust restructures those into native iterators, so
  there is no 1:1 symbol. These are NOT functional gaps; leave them.

## Explicitly OUT of this task (do not chase for this node)

- ~29 out of scope: SFST/foma/xfsm backend, `CommandLine.cc`,
  `xfst_help_message.cc`, `xfst-utils.cc`.
- 15 C API (`libhfst_c.*`): tracked in the **`hfst-c` crate**, not `crates/hfst`.
  Separate pass; `crates/hfst-c/src/lib.rs` already has some annotations.
- ~103 TWOLC iterator/container boilerplate (above).
- 37 types/typedefs: mostly aliases; only credit if a Rust type alias genuinely
  implements the symbol in the corresponding module.
- 46 MAIN_TEST mains + 7 HfstTransducer.cc test helpers: test-porting (the
  `HfstXeroxRulesTest.cc` 46-function suite is already ported in
  `crates/hfst/tests/test_xerox_rules.rs`). Lower value; a separate decision.

## Known Wave-3 punch list surfaced by the ported tests (NOT this task)

These are real bugs to fix later (Wave 3 = make tests pass), found by
`crates/hfst/tests/test_xerox_rules.rs` (26 `#[ignore]`) and others:
- `hfst_xerox_rules::restriction` panics on a Drop of an unavailable-type
  transducer (`hfst_transducer.rs:~1334`).
- `parallelBracketedReplace` panics at the same Drop site.
- empty-RHS mapping panics in `convert_tropical_weight_transducer
  handle_symbol_tables`.
- LOG-semiring `are_equivalent`/`intersect`/`subtract` still use two separate
  EncodeMappers (the tropical ones were fixed with the fork's `encode_into`);
  applying the same fix to LOG regresses `expanding_unknowns_log` via the fragile
  LOG `minimize`/`Equivalent` path — needs LOG to determinize-before-Equivalent.

## Regenerator + applier scripts (recreate in /tmp)

```python
# /tmp/gap_list.py  — regenerate docs/wave2-gap-list.txt
import re, os
manifest=open('plan/.port-manifest.styx').read()
mids=set(); meta={}
rec=re.compile(r'\{id ([^,]+), kind @(\w+), name ([^,]+), qualified [^,]+, signature ("(?:[^"\\]|\\.)*"|\S+), file ([^,]+),')
for m in rec.finditer(manifest):
    mids.add(m.group(1)); meta[m.group(1)]=dict(kind=m.group(2),name=m.group(3).strip('"'),file=m.group(5))
rust_ids=set(); alltext=''
for root,_,fs in os.walk('crates/hfst/src'):
    for fn in fs:
        if fn.endswith('.rs'):
            t=open(os.path.join(root,fn)).read(); alltext+=t+'\n'
            for mm in re.finditer(r'\[spec:hfst:def:([^\]]+)\]', t): rust_ids.add(mm.group(1).strip())
twolc={'TwolcCompiler','OtherSymbolTransducer','InputReader','HfstTwolcDefs','grammar_defs','ConstContainerIterator','VariableValueIterator','VariableBlockContainer','VariableBlock','MixedConstContainerIterator','VariableContainer','RuleVariablesConstIterator','VariableContainerBase','MatchedConstContainerIterator','VariableValues','RuleVariables','RuleSymbolVector'}
oos={'CommandLine','xfst_help_message','xfst-utils','xfst_utils'}
rows=[]
for y in sorted(mids):
    if y in rust_ids: continue
    f=meta[y]['file']; b=re.sub(r'\.(cc|h|hh|cpp)$','',os.path.basename(f)); k=meta[y]['kind']; nm=meta[y]['name']
    if f.endswith('Test.cc') or re.search(r'(^|[.-])(test\d*[a-z]?|subtest\d*)(-fn)?$',y): continue
    if y.endswith('.main-fn'): continue
    if 'xfsm' in y.lower() or 'foma' in y.lower() or 'sfst' in y.lower() or 'sfst' in f.lower(): continue
    if b.startswith('libhfst_c') or y.startswith('libhfst-c'): continue
    if b in twolc or b in oos: continue
    if k not in ('function','method','constructor'): continue
    if nm in ('operator','main',''): continue
    has=bool(re.search(r'\bfn '+re.escape(nm)+r'\b', alltext))
    rows.append((b, nm, k, 'IMPL?' if has else 'PORT', y))
rows.sort()
print(len(rows),'gaps;', sum(1 for r in rows if r[3]=='IMPL?'),'IMPL?,', sum(1 for r in rows if r[3]=='PORT'),'PORT')
for cf,nm,k,st,sid in rows: print(f"{st}  {cf:24s} {nm:34s} {k:11s} {sid}")
```

```python
# /tmp/apply_fn.py  — add a missing annotation directly above a fn.
# Input JSON: [{"file":"hfst_basic_transducer.rs","line":<0-based fn line>,"missing":"<id>"}]
import json, collections
fixes=json.load(open('/tmp/impl2.json'))
byfile=collections.defaultdict(list)
for f in fixes: byfile[f['file']].append((f['line'], f['missing']))
for fn, items in byfile.items():
    path='crates/hfst/src/'+fn; lines=open(path).read().split('\n')
    ins=collections.defaultdict(list)
    for line, Y in items:
        prefix=lines[line][:len(lines[line])-len(lines[line].lstrip())]
        ins[line].append((prefix,Y))
    for idx in sorted(ins, reverse=True):
        block=[]
        for prefix,Y in ins[idx]:
            block.append(f'{prefix}// [spec:hfst:def:{Y}]'); block.append(f'{prefix}// [spec:hfst:sem:{Y}]')
        lines[idx:idx]=block
    open(path,'w').write('\n'.join(lines))
```

## Suggested order

1. Knock out the 22 `IMPL?` first (cheap credits; same method as commit `a17c58c4`
   but per-symbol-verified for the ambiguous ones).
2. Then the `PORT` set, grouped by module (pmatch_utils, xre_utils, transducer,
   lexc-utils, HfstSymbolDefs, ...), one module per batch/commit. Consider a
   subagent per module (they touch different files -> safe in parallel; agents on
   the SAME crate file corrupt the tree, so one file = one agent).
3. Re-run `nplan_port_check` after each batch.
