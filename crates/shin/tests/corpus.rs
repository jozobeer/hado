//! M1 corpus enumeration test (see `docs/plan.0.1.md` M1, steps 2-3).
//!
//! Lists every `*.shin` file under `tests/accept` and `tests/reject`,
//! prints the count and the paths, then attempts the accept/reject
//! control flow for each file:
//!
//! - accept: `compile_to_object` -> `link_executable` -> run, compare
//!   stdout with the `-- expect:` header;
//! - reject: expect compilation to fail with the `-- error:` code.
//!
//! Failures are printed only; nothing is asserted. At this stage the
//! compiler only handles `def main = 1 + 2`, so most corpus files are
//! expected to fail (see `docs/plan.0.1.md:156`).

use std::path::{Path, PathBuf};

/// Repository root: two levels above `CARGO_MANIFEST_DIR` (`crates/shin`).
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/shin is two levels below the repo root")
        .to_path_buf()
}

/// `*.shin` files directly under `dir`, sorted. Anything else is ignored.
fn list_shin(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            println!("corpus_unreadable_dir: {} ({e})", dir.display());
            return out;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("shin") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Value of the first header line starting with `marker`
/// (e.g. `-- expect:` or `-- error:`), if it is non-empty.
fn header_value(text: &str, marker: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix(marker)?;
        let value = rest.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn try_accept(rel: &str, text: &str) {
    let body: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with("--"))
        .collect();
    let source = body.join("\n");
    let work = std::env::temp_dir().join(format!("shin-corpus-{}", std::process::id()));
    if std::fs::create_dir_all(&work).is_err() {
        println!("corpus_no_workdir: {rel}");
        return;
    }
    let stem: String = rel.replace('/', "_");
    let obj = work.join(format!("{stem}.o"));
    let exe = work.join(format!("{stem}.exe"));
    match shin::compile_to_object(&source, &obj) {
        Ok(()) => match shin::link_executable(&obj, &exe) {
            Ok(()) => match std::process::Command::new(&exe).output() {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let actual = stdout.trim_end_matches(['\n', '\r']);
                    match header_value(text, "-- expect:") {
                        Some(want) if actual == want => {}
                        Some(want) => {
                            println!("corpus_mismatch: {rel} expect={want:?} got={actual:?}");
                        }
                        None => println!("corpus_unexpected_success: {rel}"),
                    }
                }
                Err(e) => println!("corpus_run_failed: {rel} ({e})"),
            },
            Err(e) => println!("corpus_link_failed: {rel} ({e})"),
        },
        Err(e) => println!("corpus_compile_failed: {rel} ({e})"),
    }
}

fn try_reject(rel: &str, text: &str) {
    match header_value(text, "-- error:") {
        Some(code) => println!("corpus_reject_expect: {rel} {code}"),
        None => println!("corpus_reject_no_code: {rel}"),
    }
}

#[test]
fn corpus() {
    let root = repo_root();
    let mut rels: Vec<String> = Vec::new();
    for dir in ["tests/accept", "tests/reject"] {
        for path in list_shin(&root.join(dir)) {
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            rels.push(rel);
        }
    }
    rels.sort();
    println!("corpus_enumerated: {}", rels.len());
    for rel in &rels {
        println!("corpus_file: {rel}");
    }
    for rel in &rels {
        let text = std::fs::read_to_string(root.join(rel)).unwrap_or_default();
        if rel.starts_with("tests/accept/") {
            try_accept(rel, &text);
        } else {
            try_reject(rel, &text);
        }
    }
}
