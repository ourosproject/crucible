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
    /// A nested item (fn/impl/mod) is its own unit — parse.rs measures it on its
    /// own row, so its body must not be folded into this fn's count.
    fn visit_item(&mut self, _n: &'ast syn::Item) {}

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
