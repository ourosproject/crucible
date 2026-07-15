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
    /// A nested item (fn/impl/mod) is its own unit — parse.rs measures it on its
    /// own row, so its bindings must not be folded into this fn's count.
    fn visit_item(&mut self, _n: &'ast syn::Item) {}

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
