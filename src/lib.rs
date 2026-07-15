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
