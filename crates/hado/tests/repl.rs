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
    // 入口で断る入力（名前空間でない名前・`library` の無いディレクトリ）では、
    // セッションは標準入力を読む前に終わる。書き込みの失敗はその合図なので、
    // ここで panic にしない。
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(script.as_bytes());
    }
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

/// 束縛の仮引数は、本体の自由な名前だけにする。字句に同じ綴りがあっても、
/// ラムダの仮引数や field・module member は値の参照ではない。字句で拾うと
/// 合成した `hado_step` が E505 になり、その入力が捨てられる。
#[test]
fn repl_does_not_pass_a_binding_that_only_shares_a_token_spelling() {
    let out = stdout("1 as x ->\nfn(x => x + 1) 2\n");
    assert_eq!(out, "3\n");
    let out = stdout("1 as map ->\nList.map [1, 2] fn(v => v + 1)\n");
    assert_eq!(out, "[2, 3]\n");
    let out = stdout("\"hello\" as message ->\n{ message = \"err\" }\n");
    assert_eq!(out, "{ message = \"err\" }\n");
    // 本当に使う束縛は、同名の仮引数や field があっても残る。
    let out = stdout("1 as x ->\nfn(a => a + x) 2\n");
    assert_eq!(out, "3\n");
    let out = stdout("1 as x ->\n2 as x ->\nx\n");
    assert_eq!(out, "2\n");
}

/// 束縛の名前の直前が ASCII 以外の区切り（コピーした NBSP や全角空白）だと
/// 字句が読めず、入力は未完のまま EOF で評価される。束縛として剥がさず、
/// `hado check` と同じ E100 を出す（剥がすと本体の `1` だけが通って黙って束縛する）。
#[test]
fn repl_reports_e100_when_a_non_ascii_separator_precedes_the_binder_name() {
    for separator in ['\u{a0}', '\u{3000}'] {
        let (code, out, err) = repl(&format!("1 as{separator}x ->\n"), None);
        let err = String::from_utf8(err).unwrap();
        assert_eq!(code, Some(0), "{err}");
        assert_eq!(String::from_utf8(out).unwrap(), "");
        assert!(err.contains("error[E100]"), "{err}");
    }
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

/// §52: `library` を宣言しないディレクトリはライブラリではない。`stdlib/` の
/// ように名詞が prelude と重なるディレクトリを根の宣言として読むと、入力ごとに
/// 大量の E205 が出てしまうので、入口で断る。
#[test]
fn repl_rejects_a_directory_that_declares_no_library() {
    let dir = scratch("no-library");
    let plain = dir.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    std::fs::write(plain.join("plain.hd"), "def value => 1\n").unwrap();
    let (code, out, err) = repl("1\n", Some(&plain));
    assert_eq!(code, Some(1));
    assert!(out.is_empty());
    let err = String::from_utf8(err).unwrap();
    assert!(err.contains("§52"), "{err}");
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

/// 完了条件: 前の入力の効果が再実行されないこと。`log` の出力だけでは
/// 「出力を /dev/null へ向けてやり直す」実装を通してしまうので、ファイルへの
/// 書き込みの回数で数える。1 入力につき 1 文字増える入力を 2 回、束縛と長さの
/// 読み出しを挟んで 8 入力流す。期待は実装から独立に決まる。
#[test]
fn repl_counts_file_writes_once_per_input() {
    let dir = scratch("count");
    let path = dir.join("count.txt");
    std::fs::write(&path, "").unwrap();
    let path = path.to_str().unwrap();
    let script = format!(
        "read_file \"{path}\" as old -> write_file \"{path}\" (Bytes.of_string (String.concat (Bytes.to_string old) \"x\"))\n\
         1 as y ->\n\
         read_file \"{path}\" as now -> Bytes.length now\n\
         y + 1\n\
         read_file \"{path}\" as now -> Bytes.length now\n\
         read_file \"{path}\" as old -> write_file \"{path}\" (Bytes.of_string (String.concat (Bytes.to_string old) \"x\"))\n\
         y + 2\n\
         read_file \"{path}\" as now -> Bytes.length now\n"
    );
    let (code, out, err) = repl(&script, None);
    assert_eq!(code, Some(0), "{}", String::from_utf8_lossy(&err));
    assert_eq!(String::from_utf8(out).unwrap(), "1\n2\n1\n3\n2\n");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "xx");
    let _ = std::fs::remove_dir_all(dir);
}

/// 欠陥 1: 束縛した値は、そのあとの入力の割り当てで回収されてはならない。
/// Boehm GC は Rust のヒープを走査しないので、束縛を GC の根に登録する。
/// 登録しないと、割り当ての多い入力を挟んだあとの `xs` の読み出しで
/// セッションがシグナルで落ちる。
#[test]
fn repl_keeps_a_bound_value_alive_across_allocating_inputs() {
    let out = stdout(
        "List.map [1, 2, 3, 4, 5, 6, 7, 8] fn(v => String.concat \"value-\" (Int.to_string v)) as xs ->\n\
         [0, 1, 2, 3, 4, 5, 6, 7, 8, 9] as a ->\n\
         List.fold a [] fn(acc, i => List.append (List.map a fn(j => i * 10 + j)) acc) as b ->\n\
         List.fold b 0 fn(s, i => s + i + List.length (List.map b fn(j => String.concat \"zzzzzzzzzzzzzzzz\" (Int.to_string j))))\n\
         List.fold b 0 fn(s, i => s + i + List.length (List.map b fn(j => String.concat \"yyyyyyyyyyyyyyyy\" (Int.to_string j))))\n\
         xs\n",
    );
    assert_eq!(
        out,
        "14950\n14950\n[\"value-1\", \"value-2\", \"value-3\", \"value-4\", \"value-5\", \"value-6\", \"value-7\", \"value-8\"]\n"
    );
}

/// 欠陥 6: 最上位に届いた `raise` は、その入力だけを捨ててセッションを続ける。
/// §54 の `main` の規則はプログラムを終えるが、セッションには次の入力がある。
#[test]
fn repl_continues_after_a_raise() {
    let (code, out, err) = repl("Bytes.get (Bytes.of_string \"\") 5\n7\n", None);
    assert_eq!(code, Some(0));
    assert_eq!(String::from_utf8(out).unwrap(), "7\n");
    let err = String::from_utf8(err).unwrap();
    assert!(err.contains("IndexOutOfRange"), "{err}");
}

/// 欠陥 2: ライブラリの実装は、1 つの入力がその一部しか使わなくても未使用では
/// ない。§52 の利用側の検査を 1 歩ごとに当てると、A だけ使う入力も B だけ使う
/// 入力も `1 + 1` も E513 で断られる。
#[test]
fn repl_uses_library_implementations_one_input_at_a_time() {
    let dir = scratch("two");
    let library = dir.join("two");
    std::fs::create_dir_all(library.join("a")).unwrap();
    std::fs::create_dir_all(library.join("b")).unwrap();
    std::fs::write(library.join("two.hd"), "library\n").unwrap();
    std::fs::write(
        library.join("a/a.hdi"),
        "module A\ndef one (n: Int): Int\n/module\n",
    )
    .unwrap();
    std::fs::write(
        library.join("a/a.hd"),
        "implement A as A\ndef one n => n + 1\n/implement\n",
    )
    .unwrap();
    std::fs::write(
        library.join("b/b.hdi"),
        "module B\ndef two (n: Int): Int\n/module\n",
    )
    .unwrap();
    std::fs::write(
        library.join("b/b.hd"),
        "implement B as B\ndef two n => n + 2\n/implement\n",
    )
    .unwrap();
    let (code, out, err) = repl("two::a::A.one 1\ntwo::b::B.two 1\n1 + 1\n", Some(&library));
    assert_eq!(code, Some(0), "{}", String::from_utf8_lossy(&err));
    assert_eq!(String::from_utf8(out).unwrap(), "2\n3\n2\n");
    let _ = std::fs::remove_dir_all(dir);
}

/// §53.1: スタックの枯渇は実行時の診断で、シグナル死ではない。ゲストの `main`
/// が入れる番人を JIT の経路でも入れる（欠陥 5）。
#[test]
fn repl_reports_stack_exhaustion_without_a_signal() {
    let dir = scratch("deep");
    let library = dir.join("deep");
    std::fs::create_dir_all(library.join("d")).unwrap();
    std::fs::write(library.join("deep.hd"), "library\n").unwrap();
    std::fs::write(
        library.join("d/d.hdi"),
        "module D\ndef down (n: Int): Int\n/module\n",
    )
    .unwrap();
    std::fs::write(
        library.join("d/d.hd"),
        "implement D as D\ndef rec down n => if n == 0 => 0 | 1 + down (n - 1)\n/implement\n",
    )
    .unwrap();
    let (code, out, err) = repl("deep::d::D.down 1000000\n7\n", Some(&library));
    assert_eq!(code, Some(1));
    assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
    let err = String::from_utf8(err).unwrap();
    assert!(err.contains("hado: stack overflow"), "{err}");
    let _ = std::fs::remove_dir_all(dir);
}
