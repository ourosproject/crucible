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
