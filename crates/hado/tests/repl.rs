use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hado-repl-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `hado repl` を子プロセスで回す。JIT したコードの印字は C の標準出力へ
/// 出るので、セッションの中ではなくプロセスの外から受け取る。
fn repl(script: &str, library: Option<&Path>) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hado"));
    command.arg("repl");
    if let Some(library) = library {
        command.arg(library);
    }
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code(), out.stdout, out.stderr)
}

fn stdout(script: &str) -> String {
    let (code, out, err) = repl(script, None);
    assert_eq!(
        code,
        Some(0),
        "{script:?}: {}",
        String::from_utf8_lossy(&err)
    );
    String::from_utf8(out).unwrap()
}

/// §11: 1 入力が 1 継続の歩である。値は §54 の `main` の規則で印字し、
/// `e as x ->` は束縛して次の入力へ持ち越す。
#[test]
fn repl_prints_each_input_and_keeps_bindings() {
    let out = stdout("40 + 2\n\"hi\"\ntrue\n1.5\n'c'\n()\n1 as x ->\nx + 1\n");
    assert_eq!(out, "42\nhi\ntrue\n1.5\nc\n2\n");
}

/// 完了条件: 前の入力の効果が再実行されないこと。ここまでの入力を毎回まるごと
/// 走らせる実装なら `a` が 2 回出る。
#[test]
fn repl_does_not_repeat_an_earlier_inputs_effect() {
    let out = stdout("log \"a\"\nlog \"b\"\n");
    assert_eq!(out, "a\nb\n");
    let out = stdout("log \"a\"\n1 as x ->\nx\nlog \"b\"\n");
    assert_eq!(out, "a\n1\nb\n");
    // `e ->`（束縛しない継続）でも効果は 1 回だけである。
    let out = stdout("log \"a\" ->\nlog \"b\"\n");
    assert_eq!(out, "a\nb\n");
}

/// 入力の完結: 括弧が釣り合うまで送らない。
#[test]
fn repl_waits_for_a_balanced_input() {
    let out = stdout("[1, 2\n, 3]\n7\n");
    assert_eq!(out, "[1, 2, 3]\n7\n");
}

/// 診断は標準エラーへ出し、セッションは続く。
#[test]
fn repl_continues_after_a_diagnostic() {
    let (code, out, err) = repl("1 + true\n7\n", None);
    assert_eq!(code, Some(0));
    assert_eq!(String::from_utf8(out).unwrap(), "7\n");
    let err = String::from_utf8(err).unwrap();
    assert!(err.contains("error[E305]"), "{err}");
}

/// 関数値は束縛でき、次の入力から呼べる。印字はできないので出さない。
/// 前の入力の束縛を捕まえた閉包も、そのまま次の入力で呼べる。
#[test]
fn repl_binds_a_function_value_without_printing_it() {
    let out = stdout("fn(a: Int => a + 1) as f ->\nf 4\n");
    assert_eq!(out, "5\n");
    let out = stdout("1 as x ->\n(fn(a: Int => a + x)) as f ->\nf 4\n");
    assert_eq!(out, "5\n");
}

/// §52: `hado repl <dir>` はそのライブラリを読み込んだセッションを始める。
#[test]
fn repl_loads_a_library_from_its_directory() {
    let dir = scratch("library");
    let library = dir.join("counter");
    std::fs::create_dir_all(library.join("box")).unwrap();
    std::fs::write(library.join("counter.hd"), "library\n").unwrap();
    std::fs::write(
        library.join("box/box.hdi"),
        "module Box\ndef value (n: Int): Int\n/module\n",
    )
    .unwrap();
    std::fs::write(
        library.join("box/box.hd"),
        "implement Box as Box\ndef value n => n + 41\n/implement\n",
    )
    .unwrap();
    let (code, out, err) = repl("counter::box::Box.value 1\n", Some(&library));
    assert_eq!(code, Some(0), "{}", String::from_utf8_lossy(&err));
    assert_eq!(String::from_utf8(out).unwrap(), "42\n");
    let _ = std::fs::remove_dir_all(dir);
}

/// §51: 名前空間にならない名前のディレクトリは受け取らない。
#[test]
fn repl_rejects_a_directory_name_that_is_not_a_namespace() {
    let dir = scratch("bad-name");
    let library = dir.join("Bad");
    std::fs::create_dir_all(&library).unwrap();
    let (code, out, err) = repl("1\n", Some(&library));
    assert_eq!(code, Some(1));
    assert!(out.is_empty());
    let err = String::from_utf8(err).unwrap();
    assert!(err.contains("namespace"), "{err}");
    let _ = std::fs::remove_dir_all(dir);
}

/// 段階 1 は式だけを受ける。定義の入力は断るが、セッションは続く。
#[test]
fn repl_defers_definition_input_to_a_later_stage() {
    let (code, out, err) = repl("def x = 1\n2\n", None);
    assert_eq!(code, Some(0));
    assert_eq!(String::from_utf8(out).unwrap(), "2\n");
    let err = String::from_utf8(err).unwrap();
    assert!(err.contains("定義の入力"), "{err}");
}
