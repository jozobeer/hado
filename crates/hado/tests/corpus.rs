use std::path::{Path, PathBuf};

/// Repository root: two levels above `CARGO_MANIFEST_DIR` (`crates/hado`).
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/hado is two levels below the repo root")
        .to_path_buf()
}

/// `*.hd` files directly under `dir`, sorted. Anything else is ignored.
fn list_hado(dir: &Path) -> Vec<PathBuf> {
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
        if path.extension().and_then(|e| e.to_str()) == Some("hd") {
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

fn expected_output(text: &str) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    let headers: Vec<_> = text.lines().filter_map(|line| {
        line.strip_prefix("-- expect:").map(|value| (0, value))
            .or_else(|| line.strip_prefix("-- expect-error:").map(|value| (1, value)))
    }).collect();
    let [(status, value)] = headers.as_slice() else {
        return Err("expected exactly one output header".into());
    };
    let value = value.trim();
    let bytes = if value.is_empty() { Vec::new() } else { format!("{value}\n").into_bytes() };
    Ok(if *status == 0 { (0, bytes, Vec::new()) } else { (1, Vec::new(), bytes) })
}

fn try_accept(rel: &str, text: &str) -> Result<(), String> {
    let (status, stdout, stderr) = expected_output(text)?;
    let stem = rel.replace('/', "_");
    let work = std::env::temp_dir().join(format!("hado-corpus-{}-{stem}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let obj = work.join(format!("{stem}.o"));
    let exe = work.join(format!("{stem}.exe"));
    let result = (|| {
        hado::compile_source_to_object(text, rel, &obj)?;
        hado::link_executable(&obj, &exe)?;
        let out = std::process::Command::new(&exe).output().map_err(|e| e.to_string())?;
        if out.status.code() != Some(status) || out.stderr != stderr || out.stdout != stdout {
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
    match hado::check_source(text, rel) {
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
        for path in list_hado(&root.join(dir)) {
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

#[test]
fn generated_output_headers_enforce_stream_status_and_empty_output() {
    for index in 0..8 {
        let number = index * 17 + 3;
        let success = format!("def main = {number}");
        let failure = format!("def identity (x: Result<Unit, Int>): Result<Unit, Int> => x\ndef main = identity (Error({number}))");
        let empty_failure = "def identity (x: Result<Int, Unit>) => x\ndef main = identity (Error(()))";
        for (case, source, header, accepted) in [
            ("stdout", success.clone(), format!("-- expect: {number}"), true),
            ("stderr", failure.clone(), format!("-- expect-error: {number}"), true),
            ("wrong-stream", failure, format!("-- expect: {number}"), false),
            ("wrong-status", success.clone(), format!("-- expect-error: {number}"), false),
            ("wrong-value", success.clone(), format!("-- expect: {}", number + 1), false),
            ("empty", "def main = ()".into(), "-- expect:".into(), true),
            ("empty-error", empty_failure.into(), "-- expect-error:".into(), true),
            ("empty-error-wrong-status", empty_failure.into(), "-- expect:".into(), false),
            ("missing", success.clone(), String::new(), false),
            ("both", success, format!("-- expect: {number}\n-- expect-error: {number}"), false),
        ] {
            let result = try_accept(&format!("generated-{index}-{case}"), &format!("{header}\n{source}"));
            assert_eq!(result.is_ok(), accepted, "{case}: {result:?}");
        }
    }
}
