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

fn try_accept(rel: &str, text: &str) -> Result<(), String> {
    let work = std::env::temp_dir().join(format!("shin-corpus-{}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let stem = rel.replace('/', "_");
    let obj = work.join(format!("{stem}.o"));
    let exe = work.join(format!("{stem}.exe"));
    let result = (|| {
        shin::compile_source_to_object(text, rel, &obj)?;
        shin::link_executable(&obj, &exe)?;
        let out = std::process::Command::new(&exe).output().map_err(|e| e.to_string())?;
        let expected = header_value(text, "-- expect:").ok_or("missing expected stdout")? + "\n";
        if !out.status.success() || !out.stderr.is_empty() || out.stdout != expected.as_bytes() {
            return Err(format!("status={:?} stdout={:?} stderr={:?}", out.status, out.stdout, out.stderr));
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(obj);
    let _ = std::fs::remove_file(exe);
    let _ = std::fs::remove_dir(work);
    result
}

fn try_reject(rel: &str, text: &str) -> Result<(), String> {
    let code = header_value(text, "-- error:").ok_or("missing error code")?;
    match shin::check_source(text, rel) {
        Err(error) if error.code.as_deref() == Some(code.as_str()) => Ok(()),
        Err(error) => Err(format!("expected {code}, got {error}")),
        Ok(_) => Err(format!("expected {code}, but checking succeeded")),
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
    assert!(!rels.is_empty(), "empty corpus");
    let mut failures = Vec::new();
    for rel in &rels {
        let text = std::fs::read_to_string(root.join(rel)).unwrap_or_default();
        let result = if rel.starts_with("tests/accept/") {
            try_accept(rel, &text)
        } else {
            try_reject(rel, &text)
        };
        if let Err(error) = result { failures.push(format!("{rel}: {error}")); }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
