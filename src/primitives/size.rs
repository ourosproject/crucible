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
    /// A nested item (fn/impl/mod) is its own unit — parse.rs measures it on its
    /// own row, so its statements must not be folded into this fn's count.
    fn visit_item(&mut self, _n: &'ast syn::Item) {}

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
        // `let x`, `if {...}` stmt, `let y`, `g(y)` stmt, trailing `x` = 5.
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
        assert_eq!(s(src), 5);
    }

    #[test]
    fn empty_body_is_zero() {
        assert_eq!(s("fn f() {}"), 0);
    }
}
