# crucible v1 — the Simplicity engine — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A deterministic, zero-model CLI that measures the *simplicity* of Rust code by five structural primitives, ranks each function relative to the analyzed codebase, and reports it honestly — never a mean as the headline, never a grade, never a defect prediction.

**Architecture:** A pure pipeline: `syn` parses each `.rs` file into a list of functions (`parse.rs`); five independent analyzers each compute one primitive over a function's AST (`primitives/`); a rollup computes per-primitive distributions, per-function percentiles, and inter-primitive correlation (`rollup.rs`); a reporter renders a deterministic human report (`report.rs`); the CLI walks a path and drives it (`main.rs`). Every number traces to a source span and is a pure function of the AST.

**Tech Stack:** Rust 2021, `syn` 2 (full AST + `visit`), `proc-macro2` (span-locations), `walkdir`. No async, no network, no model, no external process.

**Repo:** `~/Developer/crucible` (already scaffolded: `Cargo.toml` with deps, `README.md`, the spec). Run all commands from the repo root. Spec: `docs/superpowers/specs/2026-07-14-crucible-simplicity-design.md`.

## Global Constraints

- **Zero model, zero network, zero I/O beyond reading `.rs` files.** Every number is a pure function of the source (spec §2.1).
- **No fabricated measurement.** A construct the analyzer does not understand is *not counted*, never estimated (spec §5.1).
- **Every number carries its span** — `file:line:col` (spec §5.2). `line` is 1-based, `col` is 0-based (proc-macro2 convention), reported as `line:col+1` so both read 1-based to a human.
- **Never a mean as the headline.** Rollups report the distribution (min/median/p90/p99/max) + named worst offenders; the mean is one labeled entry, never *the* number (spec §5.3).
- **Relative rank, not absolute verdict** — a function's headline signal is its percentile within the analyzed set; ship no universal thresholds (spec §5.4).
- **No collapsed grade** — the five primitives are reported side by side, never fused into one score (spec §5.5).
- **Show what correlates** — compute and display the empirical correlation between primitives, especially branching~size (spec §5.6).
- **Measures structure, does not predict defects** — the output states, in words, that a percentile is a structural fact, not a bug risk (spec §5.7).
- **P4 (state) and P5 (density) ship tagged "unvalidated / intuition"** in the output; **cognitive complexity ships labeled "derived"** and is surfaced as the human-facing headline (spec §4).
- **Alarm-don't-crash:** an unparseable file is skipped and *counted/named*, never fatal (spec §7 `main.rs`).
- **Deterministic output:** the same input yields byte-identical output every run.
- **TDD, mutation-verify every new assertion, frequent commits.** Commit messages end with:
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`

## File structure (locked)

- `src/lib.rs` — crate root: `pub mod model; pub mod parse; pub mod primitives; pub mod rollup; pub mod report;` and the top-level `analyze_source` pipeline fn.
- `src/model.rs` — pure data types: `Span`, `FnMetrics`.
- `src/parse.rs` — the *only* file that knows `syn`'s item structure: source → `Vec<Function>` (path, span, body block).
- `src/primitives/mod.rs` — re-exports the five analyzers + `cognitive`.
- `src/primitives/branching.rs` `depth.rs` `size.rs` `state.rs` `density.rs` `cognitive.rs` — one analyzer each, `fn(&syn::Block) -> u32`.
- `src/rollup.rs` — `Distribution`, `distribution()`, `percentile_rank()`, `pearson()`.
- `src/report.rs` — render the human report from `&[FnMetrics]`, deterministic.
- `src/main.rs` — CLI: `walkdir` a path, parse+analyze each file, print the report.

---

### Task 1: Parse — source → functions with spans

**Files:**
- Create: `src/lib.rs`, `src/model.rs`, `src/parse.rs`, `src/main.rs` (placeholder)

**Interfaces:**
- Produces:
  - `model::Span { file: String, line: usize, col: usize }` (Clone, Debug, Default, PartialEq)
  - `parse::Function { path: String, span: model::Span, block: syn::Block }`
  - `parse::functions_in_source(file_name: &str, src: &str) -> Result<Vec<Function>, syn::Error>` — every free fn, every `impl` method, every module fn (recursively), and every trait method with a default body, each with a dotted `path` (`module::Type::method` / `module::free_fn`) and the span of its name.

- [ ] **Step 1: Write the failing test** — append to `src/parse.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_free_fns_impl_methods_mod_fns_and_trait_defaults_with_paths() {
        let src = r#"
            fn top() {}
            struct S;
            impl S {
                fn method(&self) {}
            }
            mod inner {
                fn nested() {}
            }
            trait T {
                fn required(&self);          // no body — not a unit
                fn defaulted(&self) {}       // default body — a unit
            }
        "#;
        let fns = functions_in_source("lib.rs", src).unwrap();
        let paths: Vec<&str> = fns.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["top", "S::method", "inner::nested", "T::defaulted"],
            "every fn with a body, in source order, dotted-pathed; the bodiless trait fn is not a unit"
        );
        // Spans are 1-based lines, non-zero, and name the file.
        assert!(fns.iter().all(|f| f.span.file == "lib.rs" && f.span.line >= 1));
    }

    #[test]
    fn a_syntax_error_is_an_err_not_a_panic() {
        assert!(functions_in_source("bad.rs", "fn oops( {").is_err());
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p crucible parse:: 2>&1 | tail -20`
Expected: FAIL to compile — `functions_in_source` / `Function` not found.

- [ ] **Step 3: Write `src/model.rs`**

```rust
//! Pure measurement-record types. No logic beyond construction.

/// Where a measured unit lives. `line` is 1-based; `col` is 0-based (proc-macro2
/// convention) — the reporter adds 1 to `col` so a human reads both 1-based.
/// A number crucible reports without a span is a bug (spec §5.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

/// One function's five primitives plus the derived cognitive composite.
/// Populated field-by-field as the analyzers land (Tasks 2–7); `Default` lets a
/// task fill only its own field. `path` and `span` come from `parse`, never default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FnMetrics {
    pub path: String,
    pub span: Span,
    pub branching: u32, // P1
    pub depth: u32,     // P2
    pub size: u32,      // P3
    pub state: u32,     // P4 — unvalidated
    pub density: u32,   // P5 — unvalidated
    pub cognitive: u32, // derived (P1 weighted by P2)
}
```

- [ ] **Step 4: Write `src/parse.rs`**

```rust
//! The `syn` front-end — the only module that walks `syn`'s *item* tree. It turns
//! a source string into the list of functions crucible measures, each carrying a
//! dotted path and the span of its name. Downstream analyzers see only a
//! `syn::Block`; a second-language front-end would be added here (spec §7, §9).

use crate::model::Span;
use syn::spanned::Spanned;

/// One measurable unit: a function with a body.
pub struct Function {
    pub path: String,
    pub span: Span,
    pub block: syn::Block,
}

/// Every free fn, impl method, module fn (recursively), and trait fn WITH a default
/// body — in source order, with dotted paths. A trait fn with no body is not a unit
/// (there is nothing to measure).
pub fn functions_in_source(file_name: &str, src: &str) -> Result<Vec<Function>, syn::Error> {
    let file = syn::parse_file(src)?;
    let mut out = Vec::new();
    for item in &file.items {
        walk_item(file_name, item, &mut Vec::new(), &mut out);
    }
    Ok(out)
}

fn walk_item(file: &str, item: &syn::Item, prefix: &mut Vec<String>, out: &mut Vec<Function>) {
    match item {
        syn::Item::Fn(f) => push(file, prefix, &f.sig.ident, (*f.block).clone(), out),
        syn::Item::Impl(imp) => {
            let ty = type_name(&imp.self_ty);
            prefix.push(ty);
            for it in &imp.items {
                if let syn::ImplItem::Fn(m) = it {
                    push(file, prefix, &m.sig.ident, m.block.clone(), out);
                }
            }
            prefix.pop();
        }
        syn::Item::Trait(tr) => {
            prefix.push(tr.ident.to_string());
            for it in &tr.items {
                if let syn::TraitItem::Fn(m) = it {
                    if let Some(block) = &m.default {
                        push(file, prefix, &m.sig.ident, block.clone(), out);
                    }
                }
            }
            prefix.pop();
        }
        syn::Item::Mod(m) => {
            if let Some((_, items)) = &m.content {
                prefix.push(m.ident.to_string());
                for it in items {
                    walk_item(file, it, prefix, out);
                }
                prefix.pop();
            }
        }
        _ => {}
    }
}

fn push(file: &str, prefix: &[String], ident: &syn::Ident, block: syn::Block, out: &mut Vec<Function>) {
    let mut path = prefix.to_vec();
    path.push(ident.to_string());
    let lc = ident.span().start();
    out.push(Function {
        path: path.join("::"),
        span: Span { file: file.to_string(), line: lc.line, col: lc.column },
        block,
    });
}

/// Last path segment of an impl's self type — `impl Foo::Bar` → "Bar". Anything
/// exotic (a tuple, a reference) reports "?", never a guess.
fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| "?".to_string()),
        _ => "?".to_string(),
    }
}
```

- [ ] **Step 5: Write `src/lib.rs` and a placeholder `src/main.rs`**

`src/lib.rs`:

```rust
//! crucible — a deterministic simplicity engine. Put your code in, find out what
//! it's really made of. Zero model, zero network; every number is a pure function
//! of the source and traces to a span.

pub mod model;
pub mod parse;
```

`src/main.rs`:

```rust
fn main() {
    // The CLI lands in Task 9. For now the crate is a library under test.
    eprintln!("crucible: CLI not yet wired (see the plan). Run `cargo test`.");
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p crucible parse:: 2>&1 | tail -12`
Expected: PASS (2 tests).

- [ ] **Step 7: Mutation-verify**

Break `push` to emit an empty path (`path.push(String::new())` instead of the ident) → `finds_free_fns...` fails on the path vector. Restore. Break the trait arm to also push bodiless fns (drop the `if let Some(block)`) → won't compile (no block) — instead push a dummy block and watch the path list gain `"T::required"` and fail. Restore. Record both observed failures.

- [ ] **Step 8: Commit**

```bash
cargo test -p crucible 2>&1 | tail -3
git add src/lib.rs src/model.rs src/parse.rs src/main.rs
git commit -m "parse: source -> functions with dotted paths and name spans

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: P1 branching — and the end-to-end spine

Proves the whole pipeline (parse → one analyzer → a `FnMetrics` list) end-to-end before adding breadth, per spec §11. Branching is cyclomatic complexity: `1 + branch points`.

**Files:**
- Create: `src/primitives/mod.rs`, `src/primitives/branching.rs`
- Modify: `src/lib.rs` (add `pub mod primitives;` + `analyze_source`)

**Interfaces:**
- Consumes: `parse::functions_in_source`, `model::FnMetrics`.
- Produces:
  - `primitives::branching(block: &syn::Block) -> u32`
  - `analyze_source(file_name: &str, src: &str) -> Result<Vec<model::FnMetrics>, syn::Error>` — parses, then fills each `FnMetrics` (only `branching` populated in this task; other primitives default to 0 until their task).

- [ ] **Step 1: Write the failing test** — `src/primitives/branching.rs`:

```rust
//! P1 — branching (cyclomatic complexity): 1 + the number of branch points a
//! reader must hold. A branch point is: each `if`/`else if`, each `while`/`for`/
//! `loop`, each `match` arm beyond the first, each `&&`/`||`, and each `?` (spec §4).
//! Reported honestly as structural branching — NOT a defect or maintainability
//! predictor (spec §5.7); much of its variance is size (spec §4).

use syn::visit::{self, Visit};

pub fn branching(block: &syn::Block) -> u32 {
    let mut c = Counter { count: 0 };
    c.visit_block(block);
    1 + c.count
}

struct Counter {
    count: u32,
}

impl<'ast> Visit<'ast> for Counter {
    fn visit_expr_if(&mut self, n: &'ast syn::ExprIf) {
        self.count += 1; // each `if` and each `else if` (else-if is a nested ExprIf)
        visit::visit_expr_if(self, n);
    }
    fn visit_expr_while(&mut self, n: &'ast syn::ExprWhile) {
        self.count += 1;
        visit::visit_expr_while(self, n);
    }
    fn visit_expr_for_loop(&mut self, n: &'ast syn::ExprForLoop) {
        self.count += 1;
        visit::visit_expr_for_loop(self, n);
    }
    fn visit_expr_loop(&mut self, n: &'ast syn::ExprLoop) {
        self.count += 1;
        visit::visit_expr_loop(self, n);
    }
    fn visit_expr_match(&mut self, n: &'ast syn::ExprMatch) {
        self.count += n.arms.len().saturating_sub(1) as u32; // each arm beyond the first
        visit::visit_expr_match(self, n);
    }
    fn visit_expr_binary(&mut self, n: &'ast syn::ExprBinary) {
        if matches!(n.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
            self.count += 1;
        }
        visit::visit_expr_binary(self, n);
    }
    fn visit_expr_try(&mut self, n: &'ast syn::ExprTry) {
        self.count += 1;
        visit::visit_expr_try(self, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(src: &str) -> u32 {
        let f = syn::parse_str::<syn::ItemFn>(src).unwrap();
        branching(&f.block)
    }

    #[test]
    fn straight_line_is_one() {
        assert_eq!(count("fn f() { let x = 1; let y = x + 1; }"), 1);
    }

    #[test]
    fn counts_each_branch_point_exactly() {
        // 2 ifs (the else-if is a second ExprIf) + 1 `&&` + 1 `?` + a 3-arm match (+2)
        // = 6 branch points → 1 + 6 = 7.
        let src = r#"
            fn f(a: bool, b: bool) -> Result<u8, ()> {
                if a && b {
                } else if a {
                }
                let _ = g()?;
                match a {
                    true => {}
                    false => {}
                    _ => {}
                }
                Ok(0)
            }
        "#;
        assert_eq!(count(src), 7);
    }

    #[test]
    fn loops_each_count_one() {
        let src = r#"fn f() { while true {} for _ in 0..1 {} loop { break } }"#;
        assert_eq!(count(src), 4); // 1 + while + for + loop
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p crucible branching 2>&1 | tail -15`
Expected: FAIL to compile — module not declared.

- [ ] **Step 3: Create `src/primitives/mod.rs`**

```rust
//! The five simplicity primitives (spec §4). Each analyzer is a pure
//! `fn(&syn::Block) -> u32`, independently tested against known-count fixtures.

mod branching;
pub use branching::branching;
```

- [ ] **Step 4: Wire `analyze_source` into `src/lib.rs`**

Replace `src/lib.rs` with:

```rust
//! crucible — a deterministic simplicity engine. Put your code in, find out what
//! it's really made of. Zero model, zero network; every number is a pure function
//! of the source and traces to a span.

pub mod model;
pub mod parse;
pub mod primitives;

use model::FnMetrics;

/// Parse a source string and measure every function in it. Primitives are filled
/// as their analyzers land (Tasks 2–7); an un-landed primitive is `0` (its
/// `Default`), never estimated.
pub fn analyze_source(file_name: &str, src: &str) -> Result<Vec<FnMetrics>, syn::Error> {
    let fns = parse::functions_in_source(file_name, src)?;
    Ok(fns
        .into_iter()
        .map(|f| FnMetrics {
            branching: primitives::branching(&f.block),
            path: f.path,
            span: f.span,
            ..Default::default()
        })
        .collect())
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;

    #[test]
    fn analyze_source_populates_branching_end_to_end() {
        let src = "fn simple() {} fn branchy(a: bool) { if a {} else {} }";
        let m = analyze_source("x.rs", src).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].path, "simple");
        assert_eq!(m[0].branching, 1);
        assert_eq!(m[1].path, "branchy");
        assert_eq!(m[1].branching, 2); // one `if`
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p crucible 2>&1 | tail -8`
Expected: PASS (branching + pipeline + parse tests).

- [ ] **Step 6: Mutation-verify**

In `branching`, drop the `visit_expr_try` override (so `?` is not counted) → `counts_each_branch_point_exactly` fails (6 vs 7). Restore. Change `saturating_sub(1)` to `saturating_sub(0)` → the 3-arm match over-counts and the same test fails (8 vs 7). Restore. Record both.

- [ ] **Step 7: Commit**

```bash
cargo test -p crucible 2>&1 | tail -3
git add src/lib.rs src/primitives/
git commit -m "P1 branching (cyclomatic) + the end-to-end analyze_source spine

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: P2 depth — max control-flow nesting

**Files:**
- Create: `src/primitives/depth.rs`
- Modify: `src/primitives/mod.rs`, `src/lib.rs` (fill the `depth` field)

**Interfaces:**
- Produces: `primitives::depth(block: &syn::Block) -> u32`.

- [ ] **Step 1: Write the failing test** — `src/primitives/depth.rs`:

```rust
//! P2 — depth: the deepest nesting of control constructs (`if`/`match`/`while`/
//! `for`/`loop`, and closures with a block body) reached anywhere in the fn (spec §4).
//!
//! v1 limitation, stated honestly: an `else if` chain reads as *increasing* depth
//! (each `else if` is a nested `ExprIf`). Treating an else-if as the same level is a
//! deferred sharpening — the number is defined and reproducible, just conservative on
//! flat if/else-if ladders.

use syn::visit::{self, Visit};

pub fn depth(block: &syn::Block) -> u32 {
    let mut d = Counter { cur: 0, max: 0 };
    d.visit_block(block);
    d.max
}

struct Counter {
    cur: u32,
    max: u32,
}

impl Counter {
    fn enter(&mut self) {
        self.cur += 1;
        if self.cur > self.max {
            self.max = self.cur;
        }
    }
    fn leave(&mut self) {
        self.cur -= 1;
    }
}

impl<'ast> Visit<'ast> for Counter {
    fn visit_expr_if(&mut self, n: &'ast syn::ExprIf) {
        self.enter();
        visit::visit_expr_if(self, n);
        self.leave();
    }
    fn visit_expr_while(&mut self, n: &'ast syn::ExprWhile) {
        self.enter();
        visit::visit_expr_while(self, n);
        self.leave();
    }
    fn visit_expr_for_loop(&mut self, n: &'ast syn::ExprForLoop) {
        self.enter();
        visit::visit_expr_for_loop(self, n);
        self.leave();
    }
    fn visit_expr_loop(&mut self, n: &'ast syn::ExprLoop) {
        self.enter();
        visit::visit_expr_loop(self, n);
        self.leave();
    }
    fn visit_expr_match(&mut self, n: &'ast syn::ExprMatch) {
        self.enter();
        visit::visit_expr_match(self, n);
        self.leave();
    }
    fn visit_expr_closure(&mut self, n: &'ast syn::ExprClosure) {
        let is_block = matches!(&*n.body, syn::Expr::Block(_));
        if is_block {
            self.enter();
        }
        visit::visit_expr_closure(self, n);
        if is_block {
            self.leave();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(src: &str) -> u32 {
        let f = syn::parse_str::<syn::ItemFn>(src).unwrap();
        depth(&f.block)
    }

    #[test]
    fn flat_body_is_zero() {
        assert_eq!(d("fn f() { let x = 1; g(x); }"), 0);
    }

    #[test]
    fn nested_control_gives_max_depth() {
        // if > for > match  == depth 3
        let src = r#"
            fn f(xs: &[u8]) {
                if true {
                    for _ in xs {
                        match 1 { _ => {} }
                    }
                }
            }
        "#;
        assert_eq!(d(src), 3);
    }

    #[test]
    fn siblings_do_not_stack() {
        // two separate ifs, neither nested — depth 1, not 2.
        assert_eq!(d("fn f(a: bool) { if a {} if a {} }"), 1);
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p crucible depth 2>&1 | tail -15`
Expected: FAIL to compile — module not declared.

- [ ] **Step 3: Wire it in** — add to `src/primitives/mod.rs`:

```rust
mod depth;
pub use depth::depth;
```

And in `src/lib.rs`'s `analyze_source`, add `depth` to the `FnMetrics` construction:

```rust
        .map(|f| FnMetrics {
            branching: primitives::branching(&f.block),
            depth: primitives::depth(&f.block),
            path: f.path,
            span: f.span,
            ..Default::default()
        })
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p crucible depth 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Mutation-verify**

Remove the `self.leave()` calls from `visit_expr_if` → `siblings_do_not_stack` fails (depth grows across siblings). Restore. Record the observed failure.

- [ ] **Step 6: Commit**

```bash
cargo test -p crucible 2>&1 | tail -3
git add src/primitives/ src/lib.rs
git commit -m "P2 depth (max control-flow nesting)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: P3 size — statement count (the baseline)

Size is the empirical baseline the other primitives are read against (spec §4).

**Files:**
- Create: `src/primitives/size.rs`
- Modify: `src/primitives/mod.rs`, `src/lib.rs` (fill `size`)

**Interfaces:**
- Produces: `primitives::size(block: &syn::Block) -> u32`.

- [ ] **Step 1: Write the failing test** — `src/primitives/size.rs`:

```rust
//! P3 — size: the number of statements in the fn body, nested blocks included
//! (NOT raw LOC — comments and whitespace are noise). Statements are `let`s,
//! expression-statements, and statement-macros; a nested item definition is not
//! counted as a statement of this fn (spec §4). Size is the empirically most
//! robust primitive and the BASELINE the others are read against (spec §4, §5.6).

use syn::visit::{self, Visit};

pub fn size(block: &syn::Block) -> u32 {
    let mut c = Counter { count: 0 };
    c.visit_block(block);
    c.count
}

struct Counter {
    count: u32,
}

impl<'ast> Visit<'ast> for Counter {
    fn visit_stmt(&mut self, n: &'ast syn::Stmt) {
        // Count let-bindings, expression-statements, and macro-statements. A nested
        // item def (`Stmt::Item`) is a definition, not a statement of THIS fn.
        if !matches!(n, syn::Stmt::Item(_)) {
            self.count += 1;
        }
        visit::visit_stmt(self, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(src: &str) -> u32 {
        let f = syn::parse_str::<syn::ItemFn>(src).unwrap();
        size(&f.block)
    }

    #[test]
    fn counts_all_statements_including_nested() {
        // 2 outer (`let x`, the `if` expr-stmt) + 1 inner (`let y`) + the trailing
        // `x` expr-stmt = 4.
        let src = r#"
            fn f() -> i32 {
                let x = 1;
                if x > 0 {
                    let y = x + 1;
                    g(y);
                }
                x
            }
        "#;
        // `let x`, `if {...}` stmt, `let y`, `g(y)` stmt, trailing `x` = 5.
        assert_eq!(s(src), 5);
    }

    #[test]
    fn empty_body_is_zero() {
        assert_eq!(s("fn f() {}"), 0);
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p crucible size 2>&1 | tail -15`
Expected: FAIL to compile — module not declared.

- [ ] **Step 3: Wire it in** — add to `src/primitives/mod.rs`:

```rust
mod size;
pub use size::size;
```

And add `size: primitives::size(&f.block),` to `analyze_source`'s `FnMetrics`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p crucible size 2>&1 | tail -8`
Expected: PASS.

- [ ] **Step 5: Mutation-verify**

Change the guard to `if true` (count `Stmt::Item` too), then add a nested `fn` to the fixture and assert the count is unchanged — or simpler: change `self.count += 1` to `+= 2` → `counts_all_statements...` fails (10 vs 5). Restore. Record.

- [ ] **Step 6: Commit**

```bash
cargo test -p crucible 2>&1 | tail -3
git add src/primitives/ src/lib.rs
git commit -m "P3 size (statement count) — the baseline primitive

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: P4 state — local bindings (unvalidated)

**Files:**
- Create: `src/primitives/state.rs`
- Modify: `src/primitives/mod.rs`, `src/lib.rs` (fill `state`)

**Interfaces:**
- Produces: `primitives::state(block: &syn::Block) -> u32`.

- [ ] **Step 1: Write the failing test** — `src/primitives/state.rs`:

```rust
//! P4 — state: the count of `let` bindings in the fn, a first proxy for "how much
//! you must hold in your head." HYPOTHESIS-DRIVEN / UNVALIDATED (spec §4): motivated
//! by working-memory theory, no direct empirical study. The report tags it as such.
//! v1 counts `let` statements only; `if let`/`while let` bindings and a
//! max-simultaneously-live refinement are deferred.

use syn::visit::{self, Visit};

pub fn state(block: &syn::Block) -> u32 {
    let mut c = Counter { count: 0 };
    c.visit_block(block);
    c.count
}

struct Counter {
    count: u32,
}

impl<'ast> Visit<'ast> for Counter {
    fn visit_local(&mut self, n: &'ast syn::Local) {
        self.count += 1;
        visit::visit_local(self, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(src: &str) -> u32 {
        let f = syn::parse_str::<syn::ItemFn>(src).unwrap();
        state(&f.block)
    }

    #[test]
    fn counts_let_bindings_including_nested() {
        let src = r#"
            fn f() {
                let a = 1;
                let b = 2;
                if a > 0 {
                    let c = a + b;
                    g(c);
                }
            }
        "#;
        assert_eq!(st(src), 3);
    }

    #[test]
    fn no_lets_is_zero() {
        assert_eq!(st("fn f(x: i32) -> i32 { x + 1 }"), 0);
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p crucible state 2>&1 | tail -15`
Expected: FAIL to compile.

- [ ] **Step 3: Wire it in** — `src/primitives/mod.rs`:

```rust
mod state;
pub use state::state;
```

Add `state: primitives::state(&f.block),` to `analyze_source`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p crucible state 2>&1 | tail -8`
Expected: PASS.

- [ ] **Step 5: Mutation-verify**

Change `self.count += 1` to not fire (comment the increment) → `counts_let_bindings...` fails (0 vs 3). Restore. Record.

- [ ] **Step 6: Commit**

```bash
cargo test -p crucible 2>&1 | tail -3
git add src/primitives/ src/lib.rs
git commit -m "P4 state (let-binding count) — tagged unvalidated

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: P5 density — max method-chain length (unvalidated)

**Files:**
- Create: `src/primitives/density.rs`
- Modify: `src/primitives/mod.rs`, `src/lib.rs` (fill `density`)

**Interfaces:**
- Produces: `primitives::density(block: &syn::Block) -> u32`.

- [ ] **Step 1: Write the failing test** — `src/primitives/density.rs`:

```rust
//! P5 — density: the longest method-call / `.await` / `?` chain on a single
//! receiver — the *flat but dense* case (`a.iter().map(..).filter(..)...`) that
//! nesting depth (P2) scores as shallow and is anything but. This is what `syn`
//! buys over tree-sitter (spec §4). HYPOTHESIS-DRIVEN / UNVALIDATED — but it fills a
//! real gap the validated metrics ignore. Chain length = the number of `.method()`
//! / `.await` / `?` links on one receiver chain. Deep general expression nesting is
//! a deferred refinement.

use syn::visit::{self, Visit};

pub fn density(block: &syn::Block) -> u32 {
    let mut d = Counter { max: 0 };
    d.visit_block(block);
    d.max
}

/// Links in the chain rooted at `expr`: follow the receiver through method calls,
/// awaits, and `?` operators, counting each hop.
fn chain_len(expr: &syn::Expr) -> u32 {
    match expr {
        syn::Expr::MethodCall(m) => 1 + chain_len(&m.receiver),
        syn::Expr::Await(a) => 1 + chain_len(&a.base),
        syn::Expr::Try(t) => 1 + chain_len(&t.expr),
        _ => 0,
    }
}

struct Counter {
    max: u32,
}

impl<'ast> Visit<'ast> for Counter {
    fn visit_expr(&mut self, n: &'ast syn::Expr) {
        let l = chain_len(n);
        if l > self.max {
            self.max = l;
        }
        visit::visit_expr(self, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn den(src: &str) -> u32 {
        let f = syn::parse_str::<syn::ItemFn>(src).unwrap();
        density(&f.block)
    }

    #[test]
    fn a_long_chain_is_its_link_count() {
        // .iter().map().filter().count() = 4 links
        let src = r#"fn f(xs: Vec<i32>) -> usize { xs.iter().map(|x| x + 1).filter(|x| *x > 0).count() }"#;
        assert_eq!(den(src), 4);
    }

    #[test]
    fn no_chain_is_zero() {
        assert_eq!(den("fn f() { let x = 1 + 2; }"), 0);
    }

    #[test]
    fn a_flat_dense_chain_beats_shallow_nesting() {
        // density sees the 3-link chain that P2 depth would score as 0.
        let src = r#"fn f(s: String) -> String { s.trim().to_lowercase().to_string() }"#;
        assert_eq!(den(src), 3);
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p crucible density 2>&1 | tail -15`
Expected: FAIL to compile.

- [ ] **Step 3: Wire it in** — `src/primitives/mod.rs`:

```rust
mod density;
pub use density::density;
```

Add `density: primitives::density(&f.block),` to `analyze_source`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p crucible density 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Mutation-verify**

Remove the `Expr::Await` and `Expr::Try` arms from `chain_len` (still counts MethodCall) → both chain tests still pass (no await/try), so ALSO change the `MethodCall` arm to `0 + chain_len(...)` → `a_long_chain_is_its_link_count` fails (0 vs 4). Restore. Record.

- [ ] **Step 6: Commit**

```bash
cargo test -p crucible 2>&1 | tail -3
git add src/primitives/ src/lib.rs
git commit -m "P5 density (max method-chain length) — the flat-but-dense signal, tagged unvalidated

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 7: cognitive — the derived composite (headline)

Cognitive complexity: branching *weighted by nesting* — a labeled derived composite (spec §4), surfaced as the human-facing headline because it is the only metric with human-understandability validation.

**Files:**
- Create: `src/primitives/cognitive.rs`
- Modify: `src/primitives/mod.rs`, `src/lib.rs` (fill `cognitive`)

**Interfaces:**
- Produces: `primitives::cognitive(block: &syn::Block) -> u32`.

- [ ] **Step 1: Write the failing test** — `src/primitives/cognitive.rs`:

```rust
//! Cognitive complexity — a DERIVED composite of P1 (branching) weighted by P2
//! (nesting), never an independent primitive (spec §4). Definition: each nesting
//! construct (`if`/`else if`/`while`/`for`/`loop`/`match`) adds `1 + current_nesting`
//! and increases nesting for its body; each `&&`/`||`/`?` adds a flat `1`. This is
//! SonarSource's shape (a base increment plus a nesting bonus) reduced to crucible's
//! two primitives. It is the only metric with dedicated human-understandability
//! validation, so the report SURFACES it as the headline — labeled derived.

use syn::visit::{self, Visit};

pub fn cognitive(block: &syn::Block) -> u32 {
    let mut c = Counter { depth: 0, score: 0 };
    c.visit_block(block);
    c.score
}

struct Counter {
    depth: u32,
    score: u32,
}

impl Counter {
    /// A nesting construct: charge `1 + depth`, then deepen for its body.
    fn nest<F: FnOnce(&mut Self)>(&mut self, recurse: F) {
        self.score += 1 + self.depth;
        self.depth += 1;
        recurse(self);
        self.depth -= 1;
    }
}

impl<'ast> Visit<'ast> for Counter {
    fn visit_expr_if(&mut self, n: &'ast syn::ExprIf) {
        self.nest(|s| visit::visit_expr_if(s, n));
    }
    fn visit_expr_while(&mut self, n: &'ast syn::ExprWhile) {
        self.nest(|s| visit::visit_expr_while(s, n));
    }
    fn visit_expr_for_loop(&mut self, n: &'ast syn::ExprForLoop) {
        self.nest(|s| visit::visit_expr_for_loop(s, n));
    }
    fn visit_expr_loop(&mut self, n: &'ast syn::ExprLoop) {
        self.nest(|s| visit::visit_expr_loop(s, n));
    }
    fn visit_expr_match(&mut self, n: &'ast syn::ExprMatch) {
        self.nest(|s| visit::visit_expr_match(s, n));
    }
    fn visit_expr_binary(&mut self, n: &'ast syn::ExprBinary) {
        if matches!(n.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
            self.score += 1; // flat, no nesting bonus
        }
        visit::visit_expr_binary(self, n);
    }
    fn visit_expr_try(&mut self, n: &'ast syn::ExprTry) {
        self.score += 1;
        visit::visit_expr_try(self, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cog(src: &str) -> u32 {
        let f = syn::parse_str::<syn::ItemFn>(src).unwrap();
        cognitive(&f.block)
    }

    #[test]
    fn nesting_is_penalized_more_than_flatness() {
        // Flat: two sibling ifs → (1+0) + (1+0) = 2.
        assert_eq!(cog("fn f(a: bool) { if a {} if a {} }"), 2);
        // Nested: if > if → (1+0) + (1+1) = 3. Same branch count, higher cognitive.
        assert_eq!(cog("fn f(a: bool) { if a { if a {} } }"), 3);
    }

    #[test]
    fn boolean_operators_are_flat_increments() {
        // one `if` (1+0) + one `&&` (1) = 2.
        assert_eq!(cog("fn f(a: bool, b: bool) { if a && b {} }"), 2);
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p crucible cognitive 2>&1 | tail -15`
Expected: FAIL to compile.

- [ ] **Step 3: Wire it in** — `src/primitives/mod.rs`:

```rust
mod cognitive;
pub use cognitive::cognitive;
```

Add `cognitive: primitives::cognitive(&f.block),` to `analyze_source`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p crucible cognitive 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 5: Mutation-verify**

In `nest`, change `1 + self.depth` to `1` (drop the nesting bonus) → `nesting_is_penalized_more_than_flatness` fails (the nested case reads 2, not 3). Restore. Record.

- [ ] **Step 6: Commit**

```bash
cargo test -p crucible 2>&1 | tail -3
git add src/primitives/ src/lib.rs
git commit -m "cognitive complexity — derived (branching weighted by nesting), the headline signal

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 8: rollup — distributions, relative percentiles, correlation

**Files:**
- Create: `src/rollup.rs`
- Modify: `src/lib.rs` (`pub mod rollup;`)

**Interfaces:**
- Produces:
  - `rollup::Distribution { min: u32, median: f64, p90: f64, p99: f64, max: u32, mean: f64 }`
  - `rollup::distribution(values: &[u32]) -> Distribution` — empty slice ⇒ all-zero.
  - `rollup::percentile_rank(sorted_ascending: &[u32], v: u32) -> f64` — the fraction (0.0–1.0) of values *strictly less than* `v`; relative to the given set (spec §5.4).
  - `rollup::pearson(xs: &[f64], ys: &[f64]) -> Option<f64>` — Pearson correlation; `None` when undefined (n < 2 or a zero-variance column).

- [ ] **Step 1: Write the failing test** — `src/rollup.rs`:

```rust
//! Aggregation and ranking. Enforces two honesty laws by construction:
//! §5.3 (the mean is one labeled entry in a distribution, never the headline) and
//! §5.4 (a function's signal is its percentile WITHIN the analyzed set — no absolute
//! threshold). And §5.6: `pearson` lets the report show that branching and size move
//! together, so crucible never sells one signal as two.

/// The honest shape of a primitive's spread across the analyzed set. `mean` is
/// present but is NEVER the headline (spec §5.3) — it rides inside the distribution.
#[derive(Clone, Debug, PartialEq)]
pub struct Distribution {
    pub min: u32,
    pub median: f64,
    pub p90: f64,
    pub p99: f64,
    pub max: u32,
    pub mean: f64,
}

/// Distribution of a primitive's values. Empty input ⇒ all zero (an honest "nothing
/// measured," not an error).
pub fn distribution(values: &[u32]) -> Distribution {
    if values.is_empty() {
        return Distribution { min: 0, median: 0.0, p90: 0.0, p99: 0.0, max: 0, mean: 0.0 };
    }
    let mut v = values.to_vec();
    v.sort_unstable();
    let mean = v.iter().map(|&x| x as f64).sum::<f64>() / v.len() as f64;
    Distribution {
        min: v[0],
        median: quantile(&v, 0.50),
        p90: quantile(&v, 0.90),
        p99: quantile(&v, 0.99),
        max: v[v.len() - 1],
        mean,
    }
}

/// Linear-interpolated quantile of an ascending slice (q in 0.0..=1.0).
fn quantile(sorted: &[u32], q: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0] as f64;
    }
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    let frac = pos - lo as f64;
    sorted[lo] as f64 + frac * (sorted[hi] as f64 - sorted[lo] as f64)
}

/// Fraction of `sorted_ascending` STRICTLY LESS THAN `v` — a relative rank within
/// the analyzed set (spec §5.4). Same value ranks differently in different codebases.
pub fn percentile_rank(sorted_ascending: &[u32], v: u32) -> f64 {
    if sorted_ascending.is_empty() {
        return 0.0;
    }
    let below = sorted_ascending.partition_point(|&x| x < v);
    below as f64 / sorted_ascending.len() as f64
}

/// Pearson correlation of two equal-length columns; `None` when undefined
/// (fewer than 2 points, or either column has zero variance).
pub fn pearson(xs: &[f64], ys: &[f64]) -> Option<f64> {
    let n = xs.len();
    if n < 2 || n != ys.len() {
        return None;
    }
    let mx = xs.iter().sum::<f64>() / n as f64;
    let my = ys.iter().sum::<f64>() / n as f64;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for i in 0..n {
        let dx = xs[i] - mx;
        let dy = ys[i] - my;
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    if sxx == 0.0 || syy == 0.0 {
        return None;
    }
    Some(sxy / (sxx.sqrt() * syy.sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_reports_spread_not_just_a_mean() {
        // 40 ones + one 100: the mean (~3.4) hides the monster; min/median/max don't.
        let mut v = vec![1u32; 40];
        v.push(100);
        let d = distribution(&v);
        assert_eq!(d.min, 1);
        assert_eq!(d.median, 1.0);
        assert_eq!(d.max, 100, "the outlier survives — it is the story");
        assert!(d.mean < 4.0, "and the mean, on its own, would have buried it");
    }

    #[test]
    fn percentile_rank_is_relative_to_the_set() {
        // A value of 10 is near the top of a simple set, mid-pack in a complex one.
        let simple = [1, 2, 3, 4, 10]; // sorted
        let complex = [1, 5, 10, 20, 50];
        assert_eq!(percentile_rank(&simple, 10), 4.0 / 5.0); // 4 of 5 below → 0.8
        assert_eq!(percentile_rank(&complex, 10), 2.0 / 5.0); // 2 of 5 below → 0.4
    }

    #[test]
    fn pearson_sees_a_perfect_line_and_declines_the_undefined() {
        let xs = [1.0, 2.0, 3.0, 4.0];
        let ys = [2.0, 4.0, 6.0, 8.0]; // y = 2x
        assert!((pearson(&xs, &ys).unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(pearson(&[5.0, 5.0, 5.0], &[1.0, 2.0, 3.0]), None, "zero variance ⇒ undefined");
        assert_eq!(pearson(&[1.0], &[1.0]), None, "n < 2 ⇒ undefined");
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p crucible rollup 2>&1 | tail -15`
Expected: FAIL to compile — module not declared.

- [ ] **Step 3: Declare the module** — add to `src/lib.rs`:

```rust
pub mod rollup;
```

(place it beside the other `pub mod` lines.)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p crucible rollup 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Mutation-verify**

Change `percentile_rank`'s comparison from `< v` to `<= v` → `percentile_rank_is_relative_to_the_set` fails (0.8 becomes 1.0 for the simple set, since the value itself is included). Restore. Record — this pins the "strictly less than" definition.

- [ ] **Step 6: Commit**

```bash
cargo test -p crucible 2>&1 | tail -3
git add src/rollup.rs src/lib.rs
git commit -m "rollup: distributions (never a headline mean), relative percentiles, pearson correlation

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 9: report + CLI — the honest human output, dogfooded

**Files:**
- Create: `src/report.rs`
- Modify: `src/lib.rs` (`pub mod report;`), `src/main.rs` (the real CLI)

**Interfaces:**
- Consumes: `model::FnMetrics`, `rollup::*`.
- Produces:
  - `report::render(metrics: &[model::FnMetrics], top_n: usize) -> String` — deterministic; worst-offenders per primitive, the cognitive-headline note, the branching~size correlation, and the honesty labels (P4/P5 unvalidated, no-defect disclaimer).
  - a working `crucible <path>` CLI.

- [ ] **Step 1: Write the failing test** — `src/report.rs`:

```rust
//! The human report. Deterministic ordering (spec §5, §6). It carries the honesty
//! laws in its *copy*, not just its structure: it never prints a single grade
//! (§5.5), it labels P4/P5 as unvalidated (§4), it surfaces cognitive as the
//! headline (§4), it prints the branching~size correlation (§5.6), and it states
//! that a percentile is a structural fact, not a defect risk (§5.7).

use crate::model::FnMetrics;
use crate::rollup;

const NO_DEFECT_DISCLAIMER: &str =
    "crucible measures structure, not defects — a percentile is not a bug risk.";

/// Render the full report. `top_n` bounds each worst-offenders table.
pub fn render(metrics: &[FnMetrics], top_n: usize) -> String {
    let mut out = String::new();
    out.push_str("crucible — simplicity report\n");
    out.push_str(NO_DEFECT_DISCLAIMER);
    out.push('\n');

    if metrics.is_empty() {
        out.push_str("\nno functions found.\n");
        return out;
    }

    // Cognitive is the human-facing headline (derived, labeled).
    out.push_str("\n## cognitive complexity (derived: branching x nesting; the human-facing headline)\n");
    push_offenders(&mut out, metrics, top_n, |m| m.cognitive);

    out.push_str("\n## P1 branching (cyclomatic)\n");
    push_offenders(&mut out, metrics, top_n, |m| m.branching);
    out.push_str("\n## P2 depth (max control nesting)\n");
    push_offenders(&mut out, metrics, top_n, |m| m.depth);
    out.push_str("\n## P3 size (statements — the baseline)\n");
    push_offenders(&mut out, metrics, top_n, |m| m.size);
    out.push_str("\n## P4 state (let bindings) [UNVALIDATED / intuition]\n");
    push_offenders(&mut out, metrics, top_n, |m| m.state);
    out.push_str("\n## P5 density (max method-chain) [UNVALIDATED / intuition]\n");
    push_offenders(&mut out, metrics, top_n, |m| m.density);

    // §5.6 — show that branching and size move together.
    let branching: Vec<f64> = metrics.iter().map(|m| m.branching as f64).collect();
    let size: Vec<f64> = metrics.iter().map(|m| m.size as f64).collect();
    out.push_str("\n## correlation\n");
    match rollup::pearson(&branching, &size) {
        Some(r) => out.push_str(&format!(
            "branching~size: r={r:.2} — much of 'branchy' is really 'big'; read them together.\n"
        )),
        None => out.push_str("branching~size: undefined (too few functions or no variance).\n"),
    }
    out
}

/// One primitive's worst offenders + its distribution line. Deterministic order:
/// by the metric descending, then by path ascending to break ties.
fn push_offenders(out: &mut String, metrics: &[FnMetrics], top_n: usize, key: impl Fn(&FnMetrics) -> u32) {
    let mut values: Vec<u32> = metrics.iter().map(&key).collect();
    values.sort_unstable();
    let dist = rollup::distribution(&values);
    out.push_str(&format!(
        "  distribution: min {} · median {:.0} · p90 {:.0} · p99 {:.0} · max {} · (mean {:.1})\n",
        dist.min, dist.median, dist.p90, dist.p99, dist.max, dist.mean
    ));

    let mut ranked: Vec<&FnMetrics> = metrics.iter().collect();
    ranked.sort_by(|a, b| key(b).cmp(&key(a)).then_with(|| a.path.cmp(&b.path)));
    for m in ranked.into_iter().take(top_n) {
        let pct = rollup::percentile_rank(&values, key(m));
        out.push_str(&format!(
            "  {:>5}  p{:>3.0}  {}  {}:{}:{}\n",
            key(m),
            pct * 100.0,
            m.path,
            m.span.file,
            m.span.line,
            m.span.col + 1
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Span;

    fn fnm(path: &str, cognitive: u32, branching: u32, size: u32) -> FnMetrics {
        FnMetrics {
            path: path.into(),
            span: Span { file: "x.rs".into(), line: 1, col: 0 },
            branching,
            size,
            cognitive,
            ..Default::default()
        }
    }

    #[test]
    fn surfaces_cognitive_headline_labels_unvalidated_and_disclaims_defects() {
        let r = render(&[fnm("a", 5, 3, 10), fnm("b", 1, 1, 2)], 10);
        assert!(r.contains("cognitive complexity (derived"), "cognitive is the headline");
        assert!(r.contains("headline"));
        assert!(r.contains("[UNVALIDATED / intuition]"), "P4/P5 are tagged");
        assert!(r.contains(NO_DEFECT_DISCLAIMER), "the no-defect disclaimer ships");
        // §5.5 — never a single fused 'simplicity score'.
        assert!(!r.to_lowercase().contains("simplicity score"));
        // §5.6 — the correlation line is present.
        assert!(r.contains("branching~size: r="));
    }

    #[test]
    fn worst_offender_is_first_and_ordering_is_deterministic() {
        let r = render(&[fnm("small", 1, 1, 2), fnm("BIG", 9, 5, 40)], 10);
        let cog_section = r.split("## P1 branching").next().unwrap();
        let big_at = cog_section.find("BIG").unwrap();
        let small_at = cog_section.find("small").unwrap();
        assert!(big_at < small_at, "the worst cognitive offender is listed first");
    }

    #[test]
    fn empty_input_is_honest_not_a_crash() {
        assert!(render(&[], 10).contains("no functions found"));
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p crucible report 2>&1 | tail -15`
Expected: FAIL to compile — module not declared.

- [ ] **Step 3: Declare the module and write the CLI**

Add to `src/lib.rs`:

```rust
pub mod report;
```

Replace `src/main.rs`:

```rust
//! `crucible <path>` — walk a path for .rs files, measure every function, print the
//! honest report. Alarm-don't-crash: an unparseable file is skipped and NAMED, never
//! fatal (spec §7).

use std::path::Path;
use walkdir::WalkDir;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let mut metrics = Vec::new();
    let mut skipped = Vec::new();

    for entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let rel = display_path(p);
        match std::fs::read_to_string(p) {
            Ok(src) => match crucible::analyze_source(&rel, &src) {
                Ok(mut m) => metrics.append(&mut m),
                Err(e) => skipped.push(format!("{rel}: parse error: {e}")),
            },
            Err(e) => skipped.push(format!("{rel}: unreadable: {e}")),
        }
    }

    print!("{}", crucible::report::render(&metrics, 10));

    if !skipped.is_empty() {
        // Skips are stated, never silent — an omission you cannot see reads as
        // "everything was measured."
        eprintln!("\nskipped {} file(s):", skipped.len());
        for s in &skipped {
            eprintln!("  {s}");
        }
    }
}

fn display_path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p crucible 2>&1 | tail -8`
Expected: PASS (all suites).

- [ ] **Step 5: Mutation-verify**

Remove the `NO_DEFECT_DISCLAIMER` push from `render` → `surfaces_cognitive_headline...` fails on the disclaimer assertion. Restore. Then flip the sort in `push_offenders` to ascending (`key(a).cmp(&key(b))`) → `worst_offender_is_first...` fails. Restore. Record both.

- [ ] **Step 6: The dogfood gate — run crucible on real code**

```bash
cargo build --release 2>&1 | tail -2
./target/release/crucible ~/Developer/canopy 2>/dev/null | head -40
```
Expected: a real report — cognitive headline, five primitive tables with worst offenders (each with a `file:line:col`), the branching~size correlation, the no-defect disclaimer. Confirm it names real canopy functions and produces no panic. Run it twice and `diff` the two outputs to confirm determinism:
```bash
./target/release/crucible src > /tmp/c1.txt; ./target/release/crucible src > /tmp/c2.txt; diff /tmp/c1.txt /tmp/c2.txt && echo "DETERMINISTIC"
```

- [ ] **Step 7: Commit**

```bash
cargo test -p crucible 2>&1 | tail -3
git add src/report.rs src/main.rs src/lib.rs
git commit -m "report + CLI: the honest human output — cognitive headline, unvalidated tags, no-defect disclaimer, correlation; dogfooded

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-review (done)

- **Spec coverage:** §4 five primitives → Tasks 2–6; cognitive-derived → Task 7; §5.1 no-fabrication (skip+name) → Task 9 CLI skip list; §5.2 spans → Task 1 + rendered in Task 9; §5.3 never-a-mean → rollup `Distribution` + Task 8 test; §5.4 relative rank → `percentile_rank` + Task 8 test; §5.5 no grade → Task 9 asserts no "simplicity score"; §5.6 correlation → `pearson` + Task 9 line; §5.7 no-defect → `NO_DEFECT_DISCLAIMER`; §6 units/report shape → Tasks 1, 9; §7 file structure → matches exactly; §8 tests (known-count + mutation + honesty-property + dogfood) → every task. Covered.
- **Placeholder scan:** none — every step has complete code or an exact command.
- **Type consistency:** `FnMetrics` fields (`branching/depth/size/state/density/cognitive`) are named identically in model.rs, `analyze_source`, and report.rs; each analyzer is `fn(&syn::Block) -> u32`; `percentile_rank(sorted_ascending, v)` and `distribution(&[u32])` and `pearson(&[f64], &[f64])` signatures match their call sites in report.rs.

## Notes for the implementer

- **`syn` features** are already in `Cargo.toml` (`full`, `visit`, `extra-traits`; `proc-macro2` `span-locations`). Do not remove `span-locations` — spans lose line/col without it.
- **v1 documented simplifications** (each already noted in the relevant module doc-comment, and all consistent with the spec's "name what you don't do" ethos): else-if chains read as increasing depth (P2/cognitive); P4 counts `let` statements only (not `if let`); P5 measures method-chain length (not general expression-tree depth); a `match` guard is not counted as a branch. These are deferred sharpenings, not bugs — keep the doc-comments.
- **Clippy:** run `cargo clippy --all-targets` after Task 9 and clear any warnings before the final review.
