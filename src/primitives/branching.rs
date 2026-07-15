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
    /// A nested item (fn/impl/mod) is its own unit — parse.rs measures it on its
    /// own row, so its body must not be folded into this fn's count.
    fn visit_item(&mut self, _n: &'ast syn::Item) {}

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
