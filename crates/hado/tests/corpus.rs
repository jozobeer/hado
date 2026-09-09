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

/// `*/main.hd` files under `dir`, sorted. Anything else is ignored.
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
        let path = entry.path().join("main.hd");
        if path.is_file() {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Value of the first header line starting with `marker`
/// (e.g. `// expect:` or `// error:`), if it is non-empty.
fn header_value(text: &str, marker: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix(marker)?;
        let value = rest.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn expected_output(text: &str) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    let headers: Vec<_> = text.lines().filter_map(|line| {
        line.strip_prefix("// expect:").map(|value| (0, value))
            .or_else(|| line.strip_prefix("// expect-error:").map(|value| (1, value)))
    }).collect();
    let [(status, value)] = headers.as_slice() else {
        return Err("expected exactly one output header".into());
    };
    let value = value.trim();
    let bytes = if value.is_empty() { Vec::new() } else { format!("{value}\n").into_bytes() };
    Ok(if *status == 0 { (0, bytes, Vec::new()) } else { (1, Vec::new(), bytes) })
}

fn try_accept(rel: &str, text: &str, compile: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    let (status, stdout, stderr) = expected_output(text)?;
    let stem = rel.replace('/', "_");
    let work = std::env::temp_dir().join(format!("hado-corpus-{}-{stem}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let obj = work.join(format!("{stem}.o"));
    let exe = work.join(format!("{stem}.exe"));
    let result = (|| {
        compile(&obj)?;
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
    let code = header_value(text, "// error:").ok_or("missing error code")?;
    let checked = match hado::project::Project::load(&repo_root().join(rel)) {
        Ok(project) => project.check(),
        Err(hado::project::LoadError::Diagnostic(error)) => Err(error),
        Err(error) => return Err(error.to_string()),
    };
    match checked {
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
            try_accept(rel, &text, |object| {
                hado::project::Project::load(&root.join(rel)).map_err(|error| error.to_string())?.compile_to_object(object)
            })
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
        let failure = format!("def main = raise {{ message = \"{number}\" }}");
        let empty_failure = "def main = raise { message = \"\" }";
        for (case, source, header, accepted) in [
            ("stdout", success.clone(), format!("// expect: {number}"), true),
            ("stderr", failure.clone(), format!("// expect-error: {number}"), true),
            ("wrong-stream", failure, format!("// expect: {number}"), false),
            ("wrong-status", success.clone(), format!("// expect-error: {number}"), false),
            ("wrong-value", success.clone(), format!("// expect: {}", number + 1), false),
            ("empty", "def main = ()".into(), "// expect:".into(), true),
            ("empty-error", empty_failure.into(), "// expect-error:".into(), false),
            ("empty-error-wrong-status", empty_failure.into(), "// expect:".into(), false),
            ("missing", success.clone(), String::new(), false),
            ("both", success, format!("// expect: {number}\n// expect-error: {number}"), false),
        ] {
            let text = format!("{header}\n{source}");
            let result = try_accept(&format!("generated-{index}-{case}"), &text, |object| {
                hado::compile_source_to_object(&text, "generated.hd", object)
            });
            assert_eq!(result.is_ok(), accepted, "{case}: {result:?}");
        }
    }
}
