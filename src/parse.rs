//! The `syn` front-end — the only module that walks `syn`'s *item* tree. It turns
//! a source string into the list of functions crucible measures, each carrying a
//! dotted path and the span of its name. Downstream analyzers see only a
//! `syn::Block`; a second-language front-end would be added here (spec §7, §9).

use crate::model::Span;

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
        syn::Item::Fn(f) => record_fn(file, prefix, &f.sig.ident, &f.block, out),
        syn::Item::Impl(imp) => {
            let ty = type_name(&imp.self_ty);
            prefix.push(ty);
            for it in &imp.items {
                if let syn::ImplItem::Fn(m) = it {
                    record_fn(file, prefix, &m.sig.ident, &m.block, out);
                }
            }
            prefix.pop();
        }
        syn::Item::Trait(tr) => {
            prefix.push(tr.ident.to_string());
            for it in &tr.items {
                if let syn::TraitItem::Fn(m) = it {
                    if let Some(block) = &m.default {
                        record_fn(file, prefix, &m.sig.ident, block, out);
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

/// Record one fn as a unit, then descend into its body for NESTED item definitions
/// (a `fn`/`impl`/`mod` declared inside the body) — each is its own unit, scoped
/// under this fn's name (`outer::helper`). The analyzers stop at the same boundary
/// (their `visit_item` is a no-op), so a nested fn's structure is measured once, on
/// its own row, and never folded into this fn's numbers.
fn record_fn(file: &str, prefix: &mut Vec<String>, ident: &syn::Ident, block: &syn::Block, out: &mut Vec<Function>) {
    let mut path = prefix.to_vec();
    path.push(ident.to_string());
    let lc = ident.span().start();
    out.push(Function {
        path: path.join("::"),
        span: Span { file: file.to_string(), line: lc.line, col: lc.column },
        block: block.clone(),
    });

    prefix.push(ident.to_string());
    for stmt in &block.stmts {
        if let syn::Stmt::Item(nested) = stmt {
            walk_item(file, nested, prefix, out);
        }
    }
    prefix.pop();
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
    fn a_nested_fn_is_its_own_unit_scoped_under_its_parent() {
        let src = r#"
            fn outer() {
                fn helper() {}
                let a = 1;
            }
        "#;
        let paths: Vec<String> = functions_in_source("lib.rs", src)
            .unwrap()
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert_eq!(
            paths,
            vec!["outer", "outer::helper"],
            "the nested helper is discovered as its own unit, not absorbed into outer"
        );
    }

    #[test]
    fn a_syntax_error_is_an_err_not_a_panic() {
        assert!(functions_in_source("bad.rs", "fn oops( {").is_err());
    }
}
