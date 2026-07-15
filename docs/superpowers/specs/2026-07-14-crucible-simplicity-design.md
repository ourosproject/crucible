# crucible — the Simplicity engine (design)

2026-07-14.

**Thesis, one line:** a truth instrument may only point at a fact it can measure — so
crucible measures the orthogonal axes of *simplicity* that actually exist in the syntax
tree, ranks them against your own codebase, and never emits a verdict it cannot defend.

---

## 1. What it is

crucible reads Rust source, parses it with `syn`, and reports, per function and per
module, five **orthogonal structural measurements** of how simple the code is. It emits
facts and rankings, never grades. It makes **zero model calls** — every number is
deterministic, reproducible, and traceable to a source span.

It is the first, deliberately-small slice of a larger engine (other quality dimensions,
a role-aware model — see §9), scoped here to *one dimension done honestly*.

**Constitution (inherited from canopy, narrowed to code):** *crucible tells you the
truth about your code — especially when it's unflattering. It never fabricates a
measurement it cannot take, and it never hides a bad one behind an average.*

## 2. Founding constraints (read first)

1. **Deterministic, zero model — in v1.** No LLM, no network, no Ollama. Every number is
   a pure function of the source. The model ("the fused limb", §9) is a separate future
   arc; crucible must stand and be useful without it.
2. **Rust only, via `syn`.** `syn`'s full AST is what lets crucible see *Rust idiom*
   (iterator chains, `?`, match ergonomics) rather than mere indentation — the thing a
   language-agnostic tree-sitter pass would miss. Other languages are a later, additive
   concern; the analyzer is written against a small internal node model so a second
   front-end could feed it, but that is not built now.
3. **Relative, not absolute.** There is no honest universal "complexity N = bad". crucible
   ranks each unit *within the analyzed codebase* (percentile) and surfaces the worst
   offenders. It does not ship magic thresholds. Role-appropriate yardsticks are exactly
   what the future model supplies (§9); until then, crucible does not fake them.
4. **Fact-first, never a laundered verdict.** The raw measurement is always shown and
   always traces to a `file:line:col` span. crucible never collapses the numbers into a
   single opaque score.
5. **Never a hiding mean.** Aggregation reports the distribution and the worst offenders.
   A mean that lets one monster average away into a sea of trivial functions is banned by
   construction (§5).

## 3. Exit criterion

> Run `crucible <path>` on a real Rust repo (canopy itself, first) and get an honest
> report: the worst-offending functions by each primitive, each with its number, its
> percentile rank within the repo, and its exact source span — with no fabricated grade,
> no averaged-away outlier, and no threshold crucible can't defend. Reproducible: the
> same source yields the same numbers, every run.

## 4. The measurements — five orthogonal primitives

The design rule is **measure orthogonal primitives, not overlapping composites.** Most
tools ship cyclomatic *and* cognitive *and* nesting complexity — but cognitive complexity
is *made of* branching + nesting, so that is the same signal counted thrice. crucible
measures the independent axes of "simple," each catching something the others cannot:

| # | axis | primitive | definition (what is counted) |
|---|------|-----------|------------------------------|
| P1 | **branching** | cyclomatic complexity | 1 + count of branch points in the fn body: `if`, `else if`, each `match` arm beyond the first, `while`, `loop`, `for`, `&&`, `\|\|`, and `?` (each is a path the reader must hold). |
| P2 | **depth** | max control-flow nesting | the deepest nesting of control constructs (`if`/`match`/`while`/`for`/`loop`/closures with a block body) reached anywhere in the fn. |
| P3 | **size** | statement count | number of statements/expression-statements in the fn body (NOT raw LOC — LOC is comment/whitespace noise; count AST statements). |
| P4 | **state** | live local bindings | count of `let` bindings introduced in the fn (a first, honest proxy for "how much you must hold in your head"); a refinement to *max simultaneously-live* is noted as a later sharpening, not v1. |
| P5 | **density** | max expression / method-chain depth | the longest method-call / `?` / `.await` chain and the deepest expression nesting — the *flat but dense* case (`a.iter().map(..).filter(..).flat_map(..)...`) that P2 scores as shallow and is anything but. **This is what `syn` buys over tree-sitter.** |

**Cognitive complexity** is computed too, but **explicitly labeled a *derived composite*
(P1 weighted by P2)** — a convenience summary, never presented as an independent
measurement, so nobody mistakes it for a sixth primitive.

**Deferred, and named so the boundary stays clean** — these are real signals that belong
to *other* dimensions and would muddy "simplicity":

- **parameter count** → the *interface / understandability* dimension.
- **`unwrap` / `expect` / `?`-vs-explicit-`match`** → the *robustness* dimension.
- **trait-bound / generic / where-clause complexity** → the *understandability* dimension.

crucible v1 computes none of these; it names where they go.

## 5. Honesty laws (the load-bearing part)

These are the invariants that make crucible a truth instrument rather than a metrics
soup. Each is enforced by a test (§8).

1. **No fabricated measurement.** Every number is a pure function of the AST. If a
   construct is not understood, it is *not counted and said so* — never estimated.
2. **Every number carries its span.** A reported measurement always names the
   `file:line:col` of the unit it describes. A number you cannot locate is a number you
   cannot check.
3. **Never a mean as the headline.** Rollups report the **distribution** (min / median /
   p90 / p99 / max) and the **named worst offenders**. The mean may appear only as one
   labeled entry in the distribution, never as *the* number. Rationale: the worst
   function is the story; an average buries it. (This is the canopy meter's
   blurry-sum lesson, applied to code.)
4. **Relative rank, not absolute verdict.** A unit's headline signal is its **percentile
   within the analyzed set** ("top 1% most complex here"), not a pass/fail against an
   invented threshold. crucible ships no universal thresholds.
5. **No collapsed grade.** The five primitives are reported side by side. crucible never
   fuses them into one "simplicity score" — that fusion would hide which axis is the
   problem, and weighting them requires a judgment crucible does not have.

## 6. Units and output

- **Unit of measurement:** the **function/method** (each `fn`, including methods in
  `impl`s and trait defaults, and closures reported under their enclosing fn).
- **Rollup levels:** **module** and **file** — each carrying the distribution (§5.3) of
  its functions' primitives and the worst offenders.
- **Report shape (v1, CLI, plain text):**
  - a per-primitive **"worst offenders"** table (top N functions by P1..P5), each row:
    `metric value · pN percentile · fn path · file:line`.
  - a per-file/module **distribution** line per primitive.
  - a stable, deterministic ordering (so diffs between runs are meaningful).
- **Machine-readable output** (JSON) is a named near-term follow-on so canopy can later
  consume it (§9), but v1's contract is the human report.

## 7. Architecture (units and boundaries)

Small, single-purpose files; each testable in isolation.

- `src/model.rs` — the internal **measurement record** types: `FnMetrics { p1..p5,
  cognitive_derived, span, path }`, `Rollup`, `Report`. Pure data.
- `src/parse.rs` — the `syn` front-end: source → a list of analyzable functions with
  spans and paths. The *only* file that knows `syn`; everything downstream sees the
  internal node model, so a second language front-end could be added here without
  touching the analyzers.
- `src/primitives/` — one analyzer per primitive (`branching.rs`, `depth.rs`, `size.rs`,
  `state.rs`, `density.rs`), each a pure `fn(&FnBody) -> u32`, each independently tested
  against known-count fixtures. `cognitive.rs` derives from branching+depth and is
  labeled derived.
- `src/rollup.rs` — aggregation + ranking: distributions and percentiles, enforcing §5.3
  (no headline mean) and §5.4 (relative rank).
- `src/report.rs` — rendering the human report; deterministic ordering.
- `src/main.rs` — CLI: walk a path (`walkdir`), parse, analyze, roll up, report.
  Alarm-don't-crash: an unparseable file is skipped and *counted/named*, never fatal.

**Forward-compat seam:** `FnMetrics` carries an optional `role: Option<Role>` and the
rollup an optional role-yardstick, both `None` in v1. This is the socket the future model
plugs into (§9) — present in the types so adding it later is additive, not a rewrite.

## 8. Testing

A metrics engine's tests must pin *exact known values*, or they assert nothing.

- **Known-count fixtures:** inline Rust source strings with a hand-computed expected value
  per primitive (e.g. a fn with exactly 3 `if`s + one `&&` → cyclomatic 5), asserted
  exactly. One fixture family per primitive.
- **Mutation-verify each primitive:** change the analyzer to miscount (drop the `&&`
  case), watch the fixture fail, restore. A metric test that passes when the metric is
  broken is worthless.
- **Honesty-property tests (the ones that matter most):**
  - *A monster is never hidden by a mean:* a file of 40 trivial fns + 1 extreme fn — the
    report's headline for that file must surface the extreme fn, and must not present a
    mean as the file's number.
  - *Ranking is relative:* the same fn ranks differently in a simple codebase vs a complex
    one; assert the percentile is computed against the analyzed set, not a constant.
  - *Every number has a span:* no reported measurement has an empty/zero span.
  - *Determinism:* analyzing the same input twice yields byte-identical reports.
- **Dogfood gate:** `crucible .` on crucible's own source runs clean and produces a
  sane report (and, delightfully, crucible's own worst offenders become its first
  backlog).

## 9. The road ahead (named, explicitly out of scope for v1)

These exist so the boundaries are clean and v1 is built forward-compatible — none are
built now:

- **Other dimensions** — robustness, cohesion, understandability-as-friction — each a
  sibling analyzer set producing its own primitives, same honesty laws.
- **The fused limb** — a small, glass-box model built on **eyeofrah**, trained to read
  crucible's measurements as a *sense* (an extension of eyeofrah's `Steer` residual-write
  primitive), classify each unit's **role**, and select a role-fair **yardstick**. The
  role/yardstick fills the `Option` seam left in §7; the raw measurement never moves, the
  role rides beside it as an *attributed, inspectable* opinion (glass-box: you can watch
  the role decision form). This is its own dedicated arc.
- **canopy integration** — crucible's JSON report feeding canopy's forest node health, so
  "sloppy is shown as sloppy" in the surface you already look at.
- **Other languages** — a second front-end at `parse.rs`, feeding the same analyzers.

## 10. Non-goals (consequence lines)

- **NO grade / single score** — it would hide which axis is the problem and require an
  indefensible weighting (§5.5).
- **NO universal thresholds** — none is honest; rank relative instead (§2.3, §5.4).
- **NO mean as a headline** — it buries the outlier that is the whole point (§5.3).
- **NO model, no network, in v1** — the limb is a separate arc (§9); crucible stands
  without it.
- **NO fabricated measurement** — an un-understood construct is skipped and named, never
  estimated (§5.1).
- **NO other dimensions or languages in v1** — scoped to Simplicity, Rust (§2.2).

## 11. Build order (proposal)

1. `model.rs` + `parse.rs`: source → analyzable functions with spans/paths (the smallest
   thing that produces a real list from real code). Test: parse a fixture, assert the fns
   found + their spans.
2. `primitives/branching.rs` end-to-end through a minimal report — prove the full spine
   (parse → one primitive → rank → print) on canopy's source before adding breadth.
3. The remaining four primitives, each TDD'd against known-count fixtures + mutation.
4. `rollup.rs`: distributions + relative percentiles, honesty-property tests (§8).
5. `report.rs` + `main.rs`: the deterministic human report + `walkdir` CLI; dogfood gate.
6. `cognitive.rs` (derived, labeled) last — it depends on P1+P2 and is the least load-
   bearing.
