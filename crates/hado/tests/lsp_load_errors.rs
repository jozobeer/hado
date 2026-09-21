//! §51 / §58.5: a load refusal must still reach the editor. `collect_paths`
//! rejects a sibling directory that is not a `name` token; `hado check` prints
//! that message, so `publishDiagnostics` must not replace it with an empty list.

use hado::lsp::{frame_message, read_message};
use serde_json::{json, Value};
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

fn publish_diagnostics_for(root: &Path, file: &str, text: &str) -> Value {
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
    let file_uri = uri(&root.join(file));
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
               "params": {"rootUri": uri(root), "capabilities": {}}}),
    );
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen",
               "params": {"textDocument": {"uri": file_uri, "languageId": "hado",
                                           "version": 1, "text": text}}}),
    );
    let mut published = None;
    while let Some(message) = read_message(&mut stdout).unwrap() {
        let value: Value = serde_json::from_str(&message).unwrap();
        if value["method"] == "textDocument/publishDiagnostics"
            && value["params"]["uri"] == file_uri
        {
            published = Some(value["params"].clone());
            break;
        }
    }
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null}),
    );
    send(&mut stdin, json!({"jsonrpc": "2.0", "method": "exit"}));
    assert!(server.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(root);
    published.expect("didOpen publishes diagnostics for the open document")
}

#[test]
fn invalid_namespace_directory_is_published_instead_of_an_empty_list() {
    // `Lib` is not a `name` token (§51), so load refuses before the checker
    // runs. The file itself is well-typed; an empty publish would look like a
    // successful check.
    let text = "def main = 1\n";
    let root = workspace(&[("main.hd", text)]);
    std::fs::create_dir(root.join("Lib")).unwrap();
    refusal_published(&root, "main.hd", text);
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
    refusal_published(&root, "api/store.hdi", contract);
}

/// The `publishDiagnostics` parameters of `file`, asserting that the tree's
/// refusal reached the editor instead of an empty list.
fn refusal_published(root: &Path, file: &str, text: &str) {
    let params = publish_diagnostics_for(root, file, text);
    let diagnostics = params["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");
    assert!(
        !diagnostics.is_empty(),
        "load refusal must not publish an empty list: {params}"
    );
    let message = diagnostics[0]["message"].as_str().unwrap_or("");
    assert!(
        message.contains("invalid namespace directory"),
        "expected the CLI refusal on {file}, got {params}"
    );
}
