//! §58.5: a `.hdi` opened on its own has a hover. The server is driven as a
//! subprocess over stdin/stdout, with the workspace root as `rootUri`, so the
//! contract's entry candidates are the `.hd` files under the root and the
//! member's type comes from the contract's own declaration.

use hado::lsp::{frame_message, read_message};
use serde_json::{json, Value};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

fn workspace(files: &[(&str, &str)]) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "hado-lsp-hover-contract-{}-{}",
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

/// Opens only the contract and asks for the hover at (`line`, `character`),
/// 0-based in UTF-16 as the default encoding.
fn hover_on_contract(root: &Path, contract: &str, text: &str, line: u64, character: u64) -> Value {
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
    let contract_uri = uri(&root.join(contract));
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
               "params": {"textDocument": {"uri": contract_uri, "languageId": "hado",
                                           "version": 1, "text": text}}}),
    );
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 2, "method": "textDocument/hover",
               "params": {"textDocument": {"uri": contract_uri},
                          "position": {"line": line, "character": character}}}),
    );
    let mut hover = None;
    while let Some(message) = read_message(&mut stdout).unwrap() {
        let value: Value = serde_json::from_str(&message).unwrap();
        if value["id"] == json!(2) {
            hover = Some(value["result"].clone());
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
    hover.expect("the hover request has a response")
}

#[test]
fn hover_on_a_contract_opened_alone_prints_the_declared_type() {
    let contract = "type T = | A | B\nmodule M\ndef good (x: Int): T\n";
    let root = workspace(&[
        ("store.hdi", contract),
        ("main.hd", "def use (x: T) => x\n"),
    ]);
    // `good` starts at column 4 of the third line.
    let hover = hover_on_contract(&root, "store.hdi", contract, 2, 4);
    assert_eq!(
        hover,
        json!({"contents": {"kind": "plaintext", "value": "Fn(Int => T {A})"}})
    );
}

#[test]
fn hover_on_a_nested_contract_uses_the_entry_under_the_root() {
    let contract = "type T = | A | B\nmodule M\ndef good (x: Int): T\n";
    let root = workspace(&[
        ("api/store.hdi", contract),
        ("main.hd", "def use (x: api::T) => x\n"),
    ]);
    let hover = hover_on_contract(&root, "api/store.hdi", contract, 2, 4);
    assert_eq!(
        hover,
        json!({"contents": {"kind": "plaintext", "value": "Fn(Int => api::T {A})"}})
    );
}

#[test]
fn hover_off_a_member_is_null() {
    let contract = "type T = | A | B\nmodule M\ndef good (x: Int): T\n";
    let root = workspace(&[
        ("store.hdi", contract),
        ("main.hd", "def use (x: T) => x\n"),
    ]);
    let hover = hover_on_contract(&root, "store.hdi", contract, 0, 0);
    assert_eq!(hover, Value::Null);
}
