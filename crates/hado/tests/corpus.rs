use std::path::{Path, PathBuf};

use hado::parser::parse_source;

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
    let headers: Vec<_> = text
        .lines()
        .filter_map(|line| {
            line.strip_prefix("// expect:")
                .map(|value| (0, value))
                .or_else(|| {
                    line.strip_prefix("// expect-error:")
                        .map(|value| (1, value))
                })
        })
        .collect();
    let [(status, value)] = headers.as_slice() else {
        return Err("expected exactly one output header".into());
    };
    let value = value.trim();
    let bytes = if value.is_empty() {
        Vec::new()
    } else {
        format!("{value}\n").into_bytes()
    };
    Ok(if *status == 0 {
        (0, bytes, Vec::new())
    } else {
        (1, Vec::new(), bytes)
    })
}

fn launch(exe: &Path, work: &Path) -> std::io::Result<std::process::Output> {
    std::process::Command::new(exe)
        .current_dir(work)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?
        .wait_with_output()
}

#[cfg(test)]
mod launch_tests {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("hado-corpus-launch-{}-{nonce}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn observe_launch(root: &Path) {
        let work = root.join("work");
        std::fs::create_dir(&work).unwrap();
        let contents = root.file_name().unwrap().as_encoded_bytes();
        std::fs::write(work.join("fixture"), contents).unwrap();
        let object = work.join("main.o");
        hado::compile_source_to_object(
            "type Observation = { stdin: Int; same: Bool }\n\
             def main =\n\
               Bytes.length read_stdin as input ->\n\
               read_file \"fixture\" as contents ->\n\
               write_file \"written\" contents ->\n\
               { stdin = input; same = read_file \"written\" == contents }\n",
            "main.hd",
            &object,
        )
        .unwrap();
        let exe = work.join("main");
        hado::link_executable(&object, &exe).unwrap();

        let output = launch(&exe, &work).unwrap();

        println!(
            "launch: status={:?} stdout={:?} stderr={:?}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(output.stdout, b"{ stdin = 0; same = true }\n");
        assert_eq!(std::fs::read(work.join("written")).unwrap(), contents);
        assert!(!root.join("ambient/written").exists());
    }

    #[test]
    fn isolates_work_and_stdin() {
        const CHILD_ROOT: &str = "HADO_CORPUS_LAUNCH_TEST_ROOT";
        if let Some(root) = std::env::var_os(CHILD_ROOT) {
            observe_launch(Path::new(&root));
            return;
        }
        let scratch = Scratch::new();
        std::fs::create_dir(scratch.0.join("ambient")).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "launch_tests::isolates_work_and_stdin",
                "--nocapture",
            ])
            .env(CHILD_ROOT, &scratch.0)
            .current_dir(scratch.0.join("ambient"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"stdin-must-disturb\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        let path = scratch.0.clone();
        drop(scratch);

        assert!(
            !path.exists(),
            "scratch was not removed: {}",
            path.display()
        );
        println!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(
            output.status.success(),
            "child: {:?}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        // libtest はフィルタが 0 件でも終了 0 を返す。子が launch を実際に通ったことを
        // 印字で確かめないと、テスト名がずれた瞬間に固定が測られなくなる。
        let printed = String::from_utf8_lossy(&output.stdout);
        assert!(
            printed.contains("launch: status=Some(0)"),
            "child did not run launch:\n{printed}"
        );
        assert!(
            printed.contains("1 passed"),
            "child ran no test:\n{printed}"
        );
    }
}

fn try_accept(
    rel: &str,
    text: &str,
    compile: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let (status, stdout, stderr) = expected_output(text)?;
    let stem = rel.replace('/', "_");
    let work = std::env::temp_dir().join(format!("hado-corpus-{}-{stem}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let obj = work.join(format!("{stem}.o"));
    let exe = work.join(format!("{stem}.exe"));
    let result = (|| {
        compile(&obj)?;
        hado::link_executable(&obj, &exe)?;
        let out = launch(&exe, &work).map_err(|e| e.to_string())?;
        if out.status.code() != Some(status) || out.stderr != stderr || out.stdout != stdout {
            return Err(format!(
                "status={:?} stdout={:?} stderr={:?}",
                out.status, out.stdout, out.stderr
            ));
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
                hado::project::Project::load(&root.join(rel))
                    .map_err(|error| error.to_string())?
                    .compile_to_object(object)
            })
        } else {
            try_reject(rel, &text)
        };
        if let Err(error) = result {
            failures.push(format!("{rel}: {error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every `.hd`/`.hdi` file under `directory`, recursively, sorted by path bytes.
fn source_files(directory: &Path) -> Vec<PathBuf> {
    fn walk(directory: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("hd" | "hdi")
            ) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(directory, &mut out);
    out.sort();
    out
}

/// Representative file of a `tests/library/` case (docs/corpus.md): the file
/// that declares `library`, or the byte-first path when there is none.
fn library_representative(case_directory: &Path) -> Option<PathBuf> {
    let files = source_files(case_directory);
    for file in &files {
        if file.extension().and_then(|extension| extension.to_str()) != Some("hd") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        if let Ok(ast) = parse_source(&text, &file.to_string_lossy()) {
            if ast.library.is_some() {
                return Some(file.clone());
            }
        }
    }
    files.into_iter().next()
}

#[test]
fn library_corpus() {
    let root = repo_root();
    let mut cases: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root.join("tests/library")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                cases.push(path);
            }
        }
    }
    cases.sort();
    assert!(!cases.is_empty(), "tests/library has no cases");
    let mut failures = Vec::new();
    for case in &cases {
        let rel = case.strip_prefix(&root).unwrap_or(case).display();
        let Some(representative) = library_representative(case) else {
            failures.push(format!("{rel}: no source files"));
            continue;
        };
        match hado::project::Project::load(&representative) {
            Ok(project) => {
                if let Err(error) = project.check() {
                    failures.push(format!("{rel}: {error}"));
                }
            }
            Err(hado::project::LoadError::Diagnostic(error)) => {
                failures.push(format!("{rel}: {error}"))
            }
            Err(error) => failures.push(format!("{rel}: {error}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn generated_output_headers_enforce_stream_status_and_empty_output() {
    for index in 0..8 {
        let number = index * 17 + 3;
        // §54: an Int main is the exit code and prints nothing, so the
        // generated success case prints a String to keep measuring the same
        // thing -- a value on stdout with status 0. `compile_source_to_object`
        // does not link the prelude, so `Int.to_string` is not available here.
        let success = format!("def main = \"{number}\"");
        let failure = format!("def main = raise {{ message = \"{number}\" }}");
        let empty_failure = "def main = raise { message = \"\" }";
        for (case, source, header, accepted) in [
            (
                "stdout",
                success.clone(),
                format!("// expect: {number}"),
                true,
            ),
            (
                "stderr",
                failure.clone(),
                format!("// expect-error: {number}"),
                true,
            ),
            (
                "wrong-stream",
                failure,
                format!("// expect: {number}"),
                false,
            ),
            (
                "wrong-status",
                success.clone(),
                format!("// expect-error: {number}"),
                false,
            ),
            (
                "wrong-value",
                success.clone(),
                format!("// expect: {}", number + 1),
                false,
            ),
            ("empty", "def main = ()".into(), "// expect:".into(), true),
            (
                "empty-error",
                empty_failure.into(),
                "// expect-error:".into(),
                false,
            ),
            (
                "empty-error-wrong-status",
                empty_failure.into(),
                "// expect:".into(),
                false,
            ),
            ("missing", success.clone(), String::new(), false),
            (
                "both",
                success,
                format!("// expect: {number}\n// expect-error: {number}"),
                false,
            ),
        ] {
            let text = format!("{header}\n{source}");
            let result = try_accept(&format!("generated-{index}-{case}"), &text, |object| {
                hado::compile_source_to_object(&text, "generated.hd", object)
            });
            assert_eq!(result.is_ok(), accepted, "{case}: {result:?}");
        }
    }
}
