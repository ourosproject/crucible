//! crucible — a deterministic simplicity engine. Put your code in, find out what
//! it's really made of. Zero model, zero network; every number is a pure function
//! of the source and traces to a span.

pub mod model;
pub mod parse;
pub mod primitives;
pub mod report;
pub mod rollup;

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
            depth: primitives::depth(&f.block),
            size: primitives::size(&f.block),
            state: primitives::state(&f.block),
            density: primitives::density(&f.block),
            cognitive: primitives::cognitive(&f.block),
            path: f.path,
            span: f.span,
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

    #[test]
    fn a_nested_fn_is_measured_on_its_own_row_not_folded_into_its_parent() {
        // outer's OWN structure is trivial; all the complexity lives in `helper`.
        let src = r#"
            fn outer() {
                fn helper(x: bool) { if x { for _ in 0..10 { let z = 1; } } }
                let a = 1;
                helper(true);
            }
        "#;
        let m = analyze_source("x.rs", src).unwrap();
        let by = |p: &str| m.iter().find(|f| f.path == p).unwrap();

        let outer = by("outer");
        assert_eq!(
            (outer.branching, outer.depth, outer.size, outer.state, outer.cognitive),
            (1, 0, 2, 1, 0),
            "outer's numbers must reflect ONLY outer (the `if`/`for`/`let z` belong to helper)"
        );

        let helper = by("outer::helper");
        assert_eq!(
            (helper.branching, helper.depth, helper.state, helper.cognitive),
            (3, 2, 1, 3),
            "helper carries its own structure, once"
        );
    }
}
