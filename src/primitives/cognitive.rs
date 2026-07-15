//! Cognitive complexity — a DERIVED composite of P1 (branching) weighted by P2
//! (nesting), never an independent primitive (spec §4). Definition: each nesting
//! construct (`if`/`else if`/`while`/`for`/`loop`/`match`) adds `1 + current_nesting`
//! and increases nesting for its body; each `&&`/`||`/`?` adds a flat `1`. This
//! follows the SHAPE of SonarSource cognitive complexity (a base increment plus a
//! nesting bonus) reduced to crucible's two primitives — it is not a faithful
//! reimplementation. The metric FAMILY it belongs to has human-understandability
//! validation, so the report surfaces it as the headline — labeled derived.
//!
//! Two v1 simplifications, stated for the same honesty reason `depth.rs` states its
//! own (this is the headline number, so its caveats must be visible): an `else if`
//! ladder reads as *increasing* nesting (each `else if` is a nested `ExprIf`, so it
//! takes a nesting bonus rather than being flattened as SonarSource does), and a run
//! of like boolean operators charges `+1` per operator rather than `+1` per sequence.
//! Both inflate relative to canonical cognitive complexity; both are deferred
//! sharpenings, defined and reproducible.

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
    /// A nested item (fn/impl/mod) is its own unit — parse.rs measures it on its
    /// own row, so its body must not be folded into this fn's count.
    fn visit_item(&mut self, _n: &'ast syn::Item) {}

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
