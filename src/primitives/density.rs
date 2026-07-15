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
    /// A nested item (fn/impl/mod) is its own unit — parse.rs measures it on its
    /// own row, so its chains must not be folded into this fn's count.
    fn visit_item(&mut self, _n: &'ast syn::Item) {}

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
