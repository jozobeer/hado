//! §51 / §58.5: a load refusal must still reach the editor. `collect_paths`
//! rejects a sibling directory that is not a `name` token; `hado check` prints
//! that message, so `publishDiagnostics` must not replace it with an empty list.
//! The refusal belongs to the analysis root, so it must reach the documents
//! whose analysis uses that root — and only those.

use hado::lsp::{frame_message, read_message};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

fn workspace(files: &[(&str, &str)]) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "hado-lsp-load-errors-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir
}

fn uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// Opens every document in one server, with the workspace root as `rootUri`,
/// and returns the `publishDiagnostics` parameters per document name.
fn publishes_for(root: &Path, documents: &[(&str, &str)]) -> BTreeMap<String, Value> {
    let mut server = Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = server.stdin.take().unwrap();
    let mut stdout = BufReader::new(server.stdout.take().unwrap());
    let send = |stdin: &mut std::process::ChildStdin, message: Value| {
        stdin
            .write_all(&frame_message(&message.to_string()))
            .unwrap();
        stdin.flush().unwrap();
    };
    let wanted: BTreeMap<String, String> = documents
        .iter()
        .map(|(name, _)| (uri(&root.join(name)), (*name).to_string()))
        .collect();
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
               "params": {"rootUri": uri(root), "capabilities": {}}}),
    );
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    for (name, text) in documents {
        send(
            &mut stdin,
            json!({"jsonrpc": "2.0", "method": "textDocument/didOpen",
                   "params": {"textDocument": {"uri": uri(&root.join(name)),
                                               "languageId": "hado",
                                               "version": 1, "text": text}}}),
        );
    }
    let mut published = BTreeMap::new();
    while published.len() < wanted.len() {
        let Some(message) = read_message(&mut stdout).unwrap() else {
            break;
        };
        let value: Value = serde_json::from_str(&message).unwrap();
        if value["method"] != "textDocument/publishDiagnostics" {
            continue;
        }
        let published_uri = value["params"]["uri"].as_str().unwrap_or_default();
        if let Some(name) = wanted.get(published_uri) {
            published.insert(name.clone(), value["params"].clone());
        }
    }
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null}),
    );
    send(&mut stdin, json!({"jsonrpc": "2.0", "method": "exit"}));
    assert!(server.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(root);
    published
}

/// The messages published for one document.
fn messages(params: &Value) -> Vec<String> {
    params["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .filter_map(|diagnostic| diagnostic["message"].as_str().map(str::to_owned))
        .collect()
}

fn is_refusal(message: &str) -> bool {
    message.contains("invalid namespace directory")
}

#[test]
fn invalid_namespace_directory_is_published_instead_of_an_empty_list() {
    // `Lib` is not a `name` token (§51), so load refuses before the checker
    // runs. The file itself is well-typed; an empty publish would look like a
    // successful check.
    let text = "def main = 1\n";
    let root = workspace(&[("main.hd", text)]);
    std::fs::create_dir(root.join("Lib")).unwrap();
    let published = publishes_for(&root, &[("main.hd", text)]);
    let messages = messages(&published["main.hd"]);
    assert!(
        messages.iter().any(|message| is_refusal(message)),
        "expected the CLI refusal on the entry, got {published:?}"
    );
}

#[test]
fn a_contract_opened_alone_sees_the_refusal_of_its_candidates() {
    // §58.5: a `.hdi` opened on its own takes the `.hd` files under the root as
    // its candidates, and is not an entry itself. The refusal belongs to the
    // candidates' root, so it must reach the contract too; otherwise the
    // contract looks clean while every candidate fails to load.
    let contract = "module M\ndef f (x: Int): Int\n";
    let root = workspace(&[("api/store.hdi", contract), ("main.hd", "def main = 1\n")]);
    std::fs::create_dir(root.join("Lib")).unwrap();
    let published = publishes_for(&root, &[("api/store.hdi", contract)]);
    let messages = messages(&published["api/store.hdi"]);
    assert!(
        messages.iter().any(|message| is_refusal(message)),
        "expected the refusal on the contract, got {published:?}"
    );
}

#[test]
fn a_contract_sees_a_refusal_of_a_candidate_in_a_subdirectory() {
    // The contract's candidates are the `.hd` files under the workspace root,
    // so a candidate in a subdirectory is one of them even though the failing
    // directory is not an ancestor of the contract.
    let contract = "module M\ndef f (x: Int): Int\n";
    let root = workspace(&[("api/store.hdi", contract), ("a/x.hd", "def x = 1\n")]);
    std::fs::create_dir_all(root.join("a/Lib")).unwrap();
    let published = publishes_for(&root, &[("api/store.hdi", contract)]);
    let messages = messages(&published["api/store.hdi"]);
    assert!(
        messages.iter().any(|message| message.contains("Lib")),
        "expected the subdirectory candidate's refusal, got {published:?}"
    );
}

#[test]
fn distinct_refusals_are_not_deduplicated() {
    // Two candidates rooted at different directories refuse with different
    // messages. §58.5 merges candidate results with only duplicates removed, so
    // both refusals must survive even though both sit at line 1, column 1.
    let contract = "module M\ndef f (x: Int): Int\n";
    let root = workspace(&[
        ("api/store.hdi", contract),
        ("a/x.hd", "def x = 1\n"),
        ("b/y.hd", "def y = 1\n"),
    ]);
    std::fs::create_dir_all(root.join("a/LibA")).unwrap();
    std::fs::create_dir_all(root.join("b/LibB")).unwrap();
    let published = publishes_for(&root, &[("api/store.hdi", contract)]);
    let mut messages = messages(&published["api/store.hdi"]);
    messages.sort();
    assert_eq!(
        messages,
        vec![
            "invalid namespace directory: LibA".to_string(),
            "invalid namespace directory: LibB".to_string(),
        ],
        "expected both distinct refusals, got {published:?}"
    );
}

#[test]
fn a_sibling_entry_is_not_blamed_for_another_entrys_refusal() {
    // `sub/x.hd` is its own entry and its root (`sub`) is clean, so
    // `hado check sub/x.hd` succeeds. It must not inherit the refusal of the
    // workspace root entry.
    let main = "def main = 1\n";
    let sibling = "def x = 1\n";
    let root = workspace(&[("main.hd", main), ("sub/x.hd", sibling)]);
    std::fs::create_dir(root.join("Lib")).unwrap();
    let published = publishes_for(&root, &[("main.hd", main), ("sub/x.hd", sibling)]);
    assert!(
        messages(&published["main.hd"])
            .iter()
            .any(|message| is_refusal(message)),
        "the refused entry must publish the refusal: {published:?}"
    );
    assert!(
        !messages(&published["sub/x.hd"])
            .iter()
            .any(|message| is_refusal(message)),
        "a clean sibling entry must not inherit the refusal: {published:?}"
    );
}

#[test]
fn an_excluded_document_is_not_blamed_for_the_roots_refusal() {
    // §50 excludes `.` and `__` directories from traversal, so `.scratch/` is
    // not read by the workspace root entry. Its own root is clean.
    let main = "def main = 1\n";
    let scratch = "def helper = 1\n";
    let root = workspace(&[("main.hd", main), (".scratch/helper.hd", scratch)]);
    std::fs::create_dir(root.join("Lib")).unwrap();
    let published = publishes_for(&root, &[("main.hd", main), (".scratch/helper.hd", scratch)]);
    assert!(
        messages(&published["main.hd"])
            .iter()
            .any(|message| is_refusal(message)),
        "the refused entry must publish the refusal: {published:?}"
    );
    assert!(
        !messages(&published[".scratch/helper.hd"])
            .iter()
            .any(|message| is_refusal(message)),
        "an excluded document must not inherit the refusal: {published:?}"
    );
}
