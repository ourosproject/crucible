//! `crucible <path>` — walk a path for .rs files, measure every function, print the
//! honest report. Alarm-don't-crash: an unparseable file is skipped and NAMED, never
//! fatal (spec §7).

use std::path::Path;
use walkdir::WalkDir;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let mut metrics = Vec::new();
    let mut skipped = Vec::new();

    for entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let rel = display_path(p);
        match std::fs::read_to_string(p) {
            Ok(src) => match crucible::analyze_source(&rel, &src) {
                Ok(mut m) => metrics.append(&mut m),
                Err(e) => skipped.push(format!("{rel}: parse error: {e}")),
            },
            Err(e) => skipped.push(format!("{rel}: unreadable: {e}")),
        }
    }

    print!("{}", crucible::report::render(&metrics, 10));

    if !skipped.is_empty() {
        // Skips are stated, never silent — an omission you cannot see reads as
        // "everything was measured."
        eprintln!("\nskipped {} file(s):", skipped.len());
        for s in &skipped {
            eprintln!("  {s}");
        }
    }
}

fn display_path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
