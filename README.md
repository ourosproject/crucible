# DISCLAIMER

How this was built

I designed crucible and built it with **heavy LLM assistance**, working from the spec and plan in docs/. The design decisions are mine and argued for in those documents, particularly the refusals: no grading, no averaging, no defect prediction. The commit history is the real sequence of work rather than a squashed dump.
Stating this because the README's whole argument is that a tool should only claim what it can back, and that applies to the tool's own provenance.


# crucible

**Put your code in, find out what it's really made of.**

A deterministic simplicity engine for Rust. It measures how complex your code
actually is, ranks every function against the rest of your own codebase, and
refuses to tell you anything it cannot back with a number and a source span.

No model calls. No network. No score out of ten.

```
$ crucible src

crucible — simplicity report
crucible measures structure, not defects. A percentile is not a bug risk.

85 functions measured.

## cognitive complexity (derived: branching x nesting; the human-facing headline)
  distribution: min 0 · median 0 · p90 1 · p99 12 · max 20 · (mean 0.7)
     20  p 99  walk_item                    src/parse.rs:27:4
     11  p 98  main                         src/main.rs:8:4
      5  p 96  pearson                      src/rollup.rs:62:8
      3  p 95  record_fn                    src/parse.rs:69:4
      2  p 92  Counter::visit_expr_closure  src/primitives/depth.rs:64:8
```

That is crucible run on itself. Its own parser walker is the most complex thing
in the repo, and the tool says so.

## Why it works this way

Most complexity tools hand you a grade. A 7.4, a letter, a red badge. That
number is a guess wearing a decimal point, because there is no honest universal
threshold for "too complex." What is perfectly reasonable in a parser is sloppy
in a config loader, and nothing in the syntax tree tells you which one you are
looking at.

So crucible does not grade. It ranks, relative to your codebase. The claim it
makes is small enough to actually defend: *this function sits in the top 1% most
complex in your repo, here is the number, here is the line.*

Three rules it holds itself to:

**It never averages.** A file with forty trivial functions and one monster does
not come out "fine." The monster is the story, and a mean would bury it. You get
distributions, never a headline mean.

**It never predicts bugs.** Complexity metrics get sold as defect risk
constantly, and the link is far weaker than the marketing implies. A percentile
is a structural fact about your code, not a probability that it breaks.

**It never estimates.** Every number is a pure function of the source text. Run
it twice on the same input and get the same output, forever. Unparseable files
are skipped and named out loud, because a silent omission reads as "everything
was measured."

## What it measures

Six numbers per function, each a separate pass over the syntax tree:

| | |
|---|---|
| **branching** | cyclomatic complexity: decision points plus one |
| **depth** | maximum control-flow nesting |
| **size** | statement count |
| **state** | mutable bindings in scope |
| **density** | expression chaining |
| **cognitive** | derived: branching weighted by the nesting it sits under |

Cognitive is the headline because it tracks what reading the function actually
feels like. Ten flat `if` statements and ten nested ones score identically on
cyclomatic complexity and nothing like identically on a Tuesday afternoon.

Nested functions are measured on their own row rather than folded into their
parent, so a trivial wrapper around a monster helper reads as exactly that.

## Install

```
git clone https://github.com/ourosproject/crucible
cd crucible
cargo build --release
./target/release/crucible path/to/your/rust/project
```

Point it at a file, a directory, or a whole workspace. It walks for `.rs` files
and measures everything it finds.

## Speed

Ten seconds for 117,045 functions across 2,620 files, on a laptop. There is no
inference step to wait on, because there is no model. The entire dependency tree
is ten crates and the binary is 2.5 MB.

## What it does not do yet

Rust only. `syn` does the parsing, so the language boundary is real rather than
a matter of bolting on more regexes.

It also has no idea what your code is *for*. It can tell you a function sits in
your top 1%, but not whether that is perfectly fine for a parser and alarming
for a config loader. Closing that gap needs role classification, and doing it
honestly needs a model that can show its work. That is a separate project and a
separate argument. crucible is built so that column can be filled in later
without rewriting anything underneath it.

Until then it does the smaller, checkable thing.

## Status

v1. Rust only, CLI output, 26 tests, dogfooded on real codebases including its
own.

## License

MIT.
