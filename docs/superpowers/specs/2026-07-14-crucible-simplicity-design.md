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

## 4. The measurements — five primitives (nominally distinct, empirically correlated)

The design rule is **do not ship overlapping composites as if they were independent
axes.** Most tools present cyclomatic *and* cognitive *and* nesting complexity as three
findings — but cognitive complexity is *made of* branching + nesting, the same signal
counted thrice. crucible reports the primitives below as five *nominally distinct* axes —
but it does **not** claim they are orthogonal, because they aren't: the empirical record is
that cyclomatic complexity ≈ source lines of code (Landman/Serebrenik/Vinju, ICSME'14), so
**branching (P1) and size (P3) share variance in real code.** Rather than paper over that,
crucible **measures and displays the inter-primitive correlation on the analyzed codebase**
(§5.6) — so a function flagged "branchy" that is really just "big" is *visible as such*.
Honesty about the overlap is the feature; a false claim of independence is not.

**Size (P3) is the baseline.** It is the single most empirically robust correlate of the
outcomes people care about (Isong, systematic review), and it is the hidden driver behind
cyclomatic. Every other primitive is read *against* size, not instead of it.

| # | axis | primitive | definition (what is counted) | empirical standing |
|---|------|-----------|------------------------------|--------------------|
| P1 | **branching** | cyclomatic complexity | 1 + count of branch points in the fn body: `if`, `else if`, each `match` arm beyond the first, `while`, `loop`, `for`, `&&`, `\|\|`, and `?` (each is a path the reader must hold). | established metric; **strongly correlated with P3** — much of its variance *is* size. |
| P2 | **depth** | max control-flow nesting | the deepest nesting of control constructs (`if`/`match`/`while`/`for`/`loop`/closures with a block body) reached anywhere in the fn. | validated only *indirectly*, via cognitive complexity's nesting-weighted increments. |
| P3 | **size** | statement count | number of statements/expression-statements in the fn body (NOT raw LOC — LOC is comment/whitespace noise; count AST statements). | **the baseline** — most robust correlate (Isong review). |
| P4 | **state** | live local bindings | count of `let` bindings introduced in the fn (a first, honest proxy for "how much you must hold in your head"); a refinement to *max simultaneously-live* is a later sharpening, not v1. | **hypothesis-driven / unvalidated** — motivated by working-memory theory; no direct empirical study found. |
| P5 | **density** | max expression / method-chain depth | the longest method-call / `?` / `.await` chain and the deepest expression nesting — the *flat but dense* case (`a.iter().map(..).filter(..).flat_map(..)...`) that P2 scores as shallow and is anything but. **This is what `syn` buys over tree-sitter.** | **hypothesis-driven / unvalidated** — but fills a real gap the validated metrics *ignore* (they miss dense flat expressions entirely). |

The **empirical-standing column is not decoration — it ships.** P4 and P5 are reported with
an explicit "unvalidated / intuition" tag in the output, because a truth instrument does not
present a hunch as a measured fact. crucible measures them because they plausibly matter and
nothing else covers them; it does not pretend they are settled science.

**Cognitive complexity** is computed too, but **explicitly labeled a *derived composite*
(P1 weighted by P2)** — never presented as an independent measurement, so nobody mistakes it
for a sixth primitive. It is, however, the **only** metric with a dedicated positive
validation against *human* understandability (Muñoz Barón meta-analysis, ~24k evaluations —
though the effect is partial, and head-to-head only *comparable* to cyclomatic, Esposito).
Because it is the one number with a human-comprehension link, crucible **surfaces it as the
headline human-facing signal** — derived, labeled as derived, but promoted in the report
rather than buried as an afterthought.

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
6. **Show what correlates.** crucible computes and displays the empirical correlation
   between primitives on the analyzed codebase (especially branching~size, which the
   literature says is strong). It does not claim an independence it cannot demonstrate —
   a unit that scores high on P1 *and* P3 together is flagged as "big" as much as
   "branchy," so the reader is never fooled into treating one signal as two.
7. **Measures structure, does not predict defects.** crucible reports how *complex* code
   is, never how *buggy* or *unmaintainable* it is. The complexity→defect link is weak,
   repeatedly-disconfirmed folklore (most internal metrics show little or no correlation
   with fault-proneness; better complexity numbers have coincided with *more* bugs). A
   percentile here is a structural fact, not a risk score, and the output says so — the
   moment crucible implied "high complexity ⇒ likely bug," it would be fabricating a
   prediction the evidence does not support.

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
  the role decision form). This is its own dedicated arc. The literature sweep pins down
  the shape it must take (all deferred, none built here):
  - **The model is tiny and from-scratch, not a fine-tuned LM.** Its input is ~6
    deterministic scalars (P1–P5 + the derived composite) — a *tabular* problem, best served
    by a 2–3 layer MLP of thousands of weights (or even a GBDT). There is nothing to
    transfer from a pretrained LM, because crucible already did the comprehension; the model
    only learns to *use* the signal. (Small task-specific supervised nets beat large general
    zero-shot models on narrow classification, repeatedly.)
  - **The read must be *trained in*, not added at inference.** Inference-time steering is
    weak on tuned models and degrades fluency; a model *can* be trained to read an injected
    residual channel (Steering Awareness, 2511.21399, reached 95.5% at *detecting* a signal
    and 71% at *identifying* it — and the limb's job of *reading a value and reasoning with
    it* is a further step still, so that 95% is the floor-difficulty evidence, not the target).
  - **Gate per code-unit.** A per-unit metric written into the residual stream would bleed
    into the next unit via the KV cache — unit-level gating is required, or the "every number
    traces to its own span" law breaks.
  - **Verify the write with a *consequence* read, never a logit-lens read.** A residual write
    can be causally strong yet invisible to in-place vocab projection (steerable ≠ decodable,
    2604.02608). Confirm the injected sense via eyeofrah's J-lens / a Patchscopes-style
    read-through-computation — the logit lens is provably blind to exactly this intervention.
  - **The role head may take a few extra tabular columns** (import/call shape, AST node-type
    counts) if P1–P5 under-determine role — but those feed *only* the role head; the raw
    primitive numbers are reported untouched.
  - **Positioning honesty:** eyeofrah's J-lens is not a novel *mechanism* — its math is the
    attribution-patching family (Jacobian-of-the-tail), "read-through-downstream" is
    Future Lens + Patchscopes. What is fresh is framing it as a *readout lens for meaning*
    and computing it by finite differences through an owned forward pass (more faithful to the
    nonlinear tail than one backward pass). "Storage vs consequence" is a sharper *name* for
    the literature's "decodable vs steerable," not a discovery. Claim the framing, not the
    mechanism.
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
- **NO defect / maintainability prediction** — crucible measures structure; the
  complexity→bug link is disconfirmed folklore (§5.7). A percentile is not a risk score.
- **NO claim of orthogonality** — the primitives overlap (branching~size); crucible
  measures and shows the overlap rather than pretending it away (§4, §5.6).
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

## 12. Empirical grounding

The honesty choices above are not taste — they are what the empirical literature supports.
Recorded so the design's evidence travels with it (a parallel arxiv sweep, 2026-07-14):

- **Cognitive complexity is the only code metric with dedicated *human*-understandability
  validation** — but partial, and only *comparable* to cyclomatic head-to-head. Muñoz Barón
  et al., arXiv:2007.12520 (ESEM'20); Esposito et al., arXiv:2303.07722. → §4 keeps it
  derived but surfaces it as the human-facing headline.
- **Cyclomatic complexity ≈ SLOC** — branching and size are not independent. Landman,
  Serebrenik & Vinju (ICSME'14); size's dominance corroborated by Isong, arXiv:1601.01447.
  → §4 retracts "orthogonal"; §5.6 measures and displays the correlation.
- **Complexity→defect is weak folklore** — most internal metrics show little/no
  fault-proneness correlation (Rahman, arXiv:2310.03673); signal concentrates in a few tail
  features (Lomio, arXiv:2103.11321); better complexity numbers coincided with *more* bugs
  (Bogner, arXiv:2203.11115). → §5.7, §10: crucible predicts nothing.
- **Context-relative thresholds beat universal ones** — De Luca, arXiv:2602.06831; and the
  difficulty-estimation field has abandoned static absolute formulas for distributional
  signals (Q-DAPS, arXiv:2605.12398). → §2.3, §5.4: percentile, not magic numbers.
- **P4 (state) and P5 (density) have no direct empirical study** — motivated by
  working-memory theory and the known blind spot for dense flat expressions, respectively. →
  §4 ships them explicitly tagged "unvalidated / intuition."

For the future arcs (§9): the fused-limb mechanism is a recombination of established parts —
a model *can* be trained to read an injected residual channel (Steering Awareness,
arXiv:2511.21399); tiny task-specific nets beat large zero-shot models on narrow
classification, and a precomputed-feature input is a tabular problem best served by a shallow
net or GBDT (Gorishniy, arXiv:2106.11959; Wave Network, arXiv:2411.02674). eyeofrah's J-lens
is a lens-framed, finite-difference member of the attribution-patching family (AtP*,
arXiv:2403.00745; Patchscopes, arXiv:2401.06102; Future Lens, arXiv:2311.04897), and its
"storage vs consequence" is the literature's empirically-validated "decodable vs steerable"
(arXiv:2604.02608). crucible-as-a-deterministic-difficulty-estimator (feeding an E3-style
estimate→execute→expand loop, arXiv:2607.13034) occupies genuine white space — every known
estimator is model-derived — and "cache-read as a cognitive-redundancy signal" is our own
contribution to defend, not a cited result.
