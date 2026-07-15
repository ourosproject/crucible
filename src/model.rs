//! Pure measurement-record types. No logic beyond construction.

/// Where a measured unit lives. `line` is 1-based; `col` is 0-based (proc-macro2
/// convention) — the reporter adds 1 to `col` so a human reads both 1-based.
/// A number crucible reports without a span is a bug (spec §5.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

/// One function's five primitives plus the derived cognitive composite.
/// Populated field-by-field as the analyzers land (Tasks 2–7); `Default` lets a
/// task fill only its own field. `path` and `span` come from `parse`, never default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FnMetrics {
    pub path: String,
    pub span: Span,
    pub branching: u32, // P1
    pub depth: u32,     // P2
    pub size: u32,      // P3
    pub state: u32,     // P4 — unvalidated
    pub density: u32,   // P5 — unvalidated
    pub cognitive: u32, // derived (P1 weighted by P2)
}
