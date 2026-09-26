use hado::parser::Syntax;
use hado::project::Project;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// §58.4: `--format json` は診断を出すすべての経路に効くので、ここに置く。
static FORMAT_JSON: AtomicBool = AtomicBool::new(false);

/// The one place the CLI writes a diagnostic: the human line, or §58.4 JSON.
fn print_diagnostic(project: &Project, error: &hado::parser::Diagnostic) {
    if FORMAT_JSON.load(Ordering::Relaxed) {
        eprintln!("{}", project.report_json(error));
    } else {
        eprintln!("{}", project.report(error));
    }
}

/// `Project::load` の失敗を印字する形。診断は §58.4 の JSON にも従う。読み込めて
/// いないので書き換えは引けず、終端はアンカーと同じ位置である。位置を持たない
/// 入力の誤り（§51 の名前空間ディレクトリなど）は入口の先頭に置く。
fn load_error_report(error: &hado::project::LoadError, input: &Path) -> String {
    if FORMAT_JSON.load(Ordering::Relaxed) {
        return match error {
            hado::project::LoadError::Diagnostic(diagnostic) => {
                hado::project::diagnostic_json(diagnostic, (diagnostic.line, diagnostic.col), None)
            }
            hado::project::LoadError::Input(message) => {
                let diagnostic = hado::project::input_diagnostic(input, message);
                hado::project::diagnostic_json(&diagnostic, (1, 1), None)
            }
        };
    }
    error.to_string()
}

/// コマンドラインの解釈結果。`main` はここから実際の処理を選ぶ。
struct Cli<'a> {
    command: &'a str,
    print: bool,
    type_name: Option<&'a str>,
    fix: bool,
    check_only: bool,
    /// §58.4 / plan.md L1: 同じ段階の診断を全件出す（既定は先頭の 1 件）。
    show_all: bool,
    output: Option<&'a str>,
    path: Option<&'a str>,
    second_path: Option<&'a str>,
    syntax: Syntax,
    program_args: Vec<String>,
}

/// 使い方を出して終了コード 2 を返す。引数の誤りはすべてここへ集める。
fn usage_error() -> ExitCode {
    usage();
    ExitCode::from(2)
}

fn parse_args(args: &[String]) -> Result<Cli<'_>, ExitCode> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage_error());
    };
    if !matches!(
        command,
        "check"
            | "dump-mono"
            | "build"
            | "run"
            | "fmt"
            | "test"
            | "lsp"
            | "print-postfix"
            | "probe-postfix"
            | "probe-postfix-roundtrip"
            | "probe-postfix-compare"
    ) {
        return Err(usage_error());
    }
    // §58: `lsp` speaks JSON-RPC on stdin and stdout and takes no file.
    if command == "lsp" {
        if args.len() != 1 {
            return Err(usage_error());
        }
        return Ok(Cli {
            command,
            print: false,
            type_name: None,
            fix: false,
            check_only: false,
            show_all: false,
            output: None,
            path: None,
            second_path: None,
            syntax: Syntax::Infix,
            program_args: Vec::new(),
        });
    }
    let mut cli = Cli {
        command,
        print: false,
        type_name: None,
        fix: false,
        check_only: false,
        show_all: false,
        output: None,
        path: None,
        second_path: None,
        syntax: Syntax::Infix,
        program_args: Vec::new(),
    };
    parse_flags(&mut cli, &args[1..])?;
    if cli.path.is_none() {
        return Err(usage_error());
    }
    Ok(cli)
}

/// 引数を 1 つ読んだ結果。`Stop` は `run` の `--` の後をすべて取り込んだ印。
enum Step {
    Next,
    Stop,
}

fn parse_flags<'a>(cli: &mut Cli<'a>, rest: &'a [String]) -> Result<(), ExitCode> {
    let mut index = 0;
    while index < rest.len() {
        match next_token(cli, rest, &mut index)? {
            Step::Next => index += 1,
            Step::Stop => break,
        }
    }
    Ok(())
}

fn next_token<'a>(
    cli: &mut Cli<'a>,
    rest: &'a [String],
    index: &mut usize,
) -> Result<Step, ExitCode> {
    let arg = rest[*index].as_str();
    // `run` の `--` の後だけが子プロセスの引数である。
    if arg == "--" && cli.command == "run" {
        cli.program_args.extend(rest[*index + 1..].iter().cloned());
        return Ok(Step::Stop);
    }
    if take_option(cli, arg, rest, index)? {
        return Ok(Step::Next);
    }
    if let Some(path) = positional_path(cli, arg) {
        if cli.path.is_none() {
            cli.path = Some(path);
        } else {
            cli.second_path = Some(path);
        }
        return Ok(Step::Next);
    }
    Err(usage_error())
}

fn take_option<'a>(
    cli: &mut Cli<'a>,
    arg: &'a str,
    rest: &'a [String],
    index: &mut usize,
) -> Result<bool, ExitCode> {
    if cli.command == "check" && check_option(cli, arg)? {
        return Ok(true);
    }
    match arg {
        "--fix" if cli.command == "check" || cli.command == "build" => cli.fix = true,
        // §58.4: 診断を 1 行 1 つの JSON で出す。診断を出す命令だけが受ける。
        "--format" if prints_diagnostics(cli.command) => {
            enable_json(rest, index)?;
        }
        "--check" if cli.command == "fmt" => cli.check_only = true,
        // postfix-binding-probe: 読み込みを新構文へ切り替える（既定は今の構文）。
        "--syntax" => cli.syntax = named_syntax(value_after(rest, index)?)?,
        "-o" | "--output" if cli.command == "build" => {
            cli.output = Some(value_after(rest, index)?);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `check` だけが受ける印。当たったかを返す。
fn check_option<'a>(cli: &mut Cli<'a>, arg: &'a str) -> Result<bool, ExitCode> {
    match arg {
        "--print-type" => cli.print = true,
        // §58.3: `--print-type=<名前>` はその定義の型を出す。
        value if value.starts_with("--print-type=") => {
            cli.print = true;
            cli.type_name = Some(named_type(value)?);
        }
        // §58.4 / plan.md L1: 同じ段階の診断を全件、アンカー順に出す。
        "--show-all" => cli.show_all = true,
        _ => return Ok(false),
    }
    Ok(true)
}

fn positional_path<'a>(cli: &Cli<'a>, arg: &'a str) -> Option<&'a str> {
    if arg.starts_with('-') || (cli.path.is_some() && cli.second_path.is_some()) {
        return None;
    }
    Some(arg)
}

fn prints_diagnostics(command: &str) -> bool {
    matches!(command, "check" | "build" | "run" | "test")
}

fn named_syntax(value: &str) -> Result<Syntax, ExitCode> {
    match value {
        "infix" => Ok(Syntax::Infix),
        "postfix" => Ok(Syntax::Postfix),
        _ => Err(usage_error()),
    }
}

fn named_type(value: &str) -> Result<&str, ExitCode> {
    let name = &value["--print-type=".len()..];
    if name.is_empty() {
        return Err(usage_error());
    }
    Ok(name)
}

fn enable_json(rest: &[String], index: &mut usize) -> Result<(), ExitCode> {
    if value_after(rest, index)? != "json" {
        return Err(usage_error());
    }
    FORMAT_JSON.store(true, Ordering::Relaxed);
    Ok(())
}

fn value_after<'a>(rest: &'a [String], index: &mut usize) -> Result<&'a str, ExitCode> {
    *index += 1;
    rest.get(*index).map(String::as_str).ok_or_else(usage_error)
}

fn run(cli: &Cli<'_>) -> ExitCode {
    // §58: the LSP reads and writes the protocol streams, not a source file.
    if cli.command == "lsp" {
        return ExitCode::from(hado::lsp::serve() as u8);
    }
    match cli.command {
        "print-postfix" => return print_postfix_command(cli),
        "probe-postfix" => return probe_postfix_command(cli),
        "probe-postfix-roundtrip" => return probe_roundtrip_command(cli),
        "probe-postfix-compare" => return probe_compare_command(cli),
        _ => {}
    }
    let input = match input_path(cli) {
        Ok(input) => input,
        Err(code) => return code,
    };
    if cli.command == "fmt" {
        return format_command(cli, input);
    }
    if cli.fix && cli.command == "check" {
        return check_with_fix(input, cli.type_name);
    }
    if cli.command == "run" {
        return run_program(input, &cli.program_args, cli.syntax);
    }
    if cli.command == "test" {
        return run_tests(input);
    }
    run_project_command(cli, input)
}

fn input_path<'a>(cli: &Cli<'a>) -> Result<&'a Path, ExitCode> {
    let Some(path) = cli.path else {
        return Err(usage_error());
    };
    let input = Path::new(path);
    if input.extension().and_then(|extension| extension.to_str()) != Some("hd") {
        eprintln!("input must have the .hd extension");
        return Err(ExitCode::from(1));
    }
    Ok(input)
}

fn format_command(cli: &Cli<'_>, input: &Path) -> ExitCode {
    match format_sources(input, cli.check_only) {
        Ok(changed) if cli.check_only && changed => ExitCode::from(1),
        Ok(_) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run_project_command(cli: &Cli<'_>, input: &Path) -> ExitCode {
    if cli.fix && cli.command == "build" {
        // §58.1: `build --fix` writes the canonical form before reading the
        // project, so the build sees the formatted sources.
        if let Err(code) = format_sources(input, false) {
            return code;
        }
    }
    let project = match Project::load_with_syntax(input, cli.syntax) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{}", load_error_report(&error, input));
            return ExitCode::from(1);
        }
    };
    match cli.command {
        "build" => build_command(cli, input, &project),
        "dump-mono" => dump_mono(&project),
        _ => check_command(cli, input, &project),
    }
}

fn build_command(cli: &Cli<'_>, input: &Path, project: &Project) -> ExitCode {
    let target = match cli.output {
        Some(output) => PathBuf::from(output),
        None => input.with_extension(""),
    };
    match project.build(&target) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn dump_mono(project: &Project) -> ExitCode {
    match project.dump_mono() {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_diagnostic(project, &error);
            ExitCode::from(1)
        }
    }
}

fn check_command(cli: &Cli<'_>, input: &Path, project: &Project) -> ExitCode {
    // `--print-type` keeps E706 so the printed main type keeps its effect row
    // (`docs/spec.md` §58.3); the plain check rejects a user effect on `main`.
    let (errors, checked) = project.check_all(!cli.print);
    // §58.4 / L1: `--show-all` は同じ段階の全件、既定は先頭の 1 件だけ。
    let shown = if cli.show_all {
        errors.as_slice()
    } else {
        &errors[..errors.len().min(1)]
    };
    for error in shown {
        print_diagnostic(project, error);
    }
    if cli.print {
        if let Err(code) = print_requested_type(cli, project, input, checked.as_ref()) {
            return code;
        }
    }
    if errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// §58.3 / L2: E4xx・E5xx・E6xx で止まっていても型を出す。型が無いのは字句・
/// 構文・名前解決・型の致命で、推論が `Error` を残したときだけ（Q2 の決着）。
fn print_requested_type(
    cli: &Cli<'_>,
    project: &Project,
    input: &Path,
    checked: Option<&hado::Checked>,
) -> Result<(), ExitCode> {
    let Some(checked) = checked else {
        return Ok(());
    };
    if checked.has_type(cli.type_name) {
        return print_type(checked, project, cli.type_name, input);
    }
    let Some(name) = cli.type_name else {
        return Ok(());
    };
    // §58.3: a name that is not there is `no definition named`; a name whose
    // inference failed (Q2) is there but has no type to print.
    let message = if checked.type_of(name).is_some() {
        "no type for"
    } else {
        "no definition named"
    };
    eprintln!("check: {message} {name} in {}", input.display());
    Err(ExitCode::from(1))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Ok(cli) => run(&cli),
        Err(code) => code,
    }
}

/// `check --fix` applies the canonical rewrites at the reported anchor and
/// re-checks, until a check passes or a diagnostic has no local rewrite.
/// The default `check` / `build` never write to a source file.
fn check_with_fix(input: &Path, type_name: Option<&str>) -> ExitCode {
    const LIMIT: usize = 16;
    for _ in 0..LIMIT {
        let project = match Project::load(input) {
            Ok(project) => project,
            Err(error) => {
                eprintln!("{}", load_error_report(&error, input));
                return ExitCode::from(1);
            }
        };
        match project.check() {
            Ok(checked) => {
                if let Some(name) = type_name {
                    if let Err(code) = print_type(&checked, &project, Some(name), input) {
                        return code;
                    }
                }
                return ExitCode::SUCCESS;
            }
            Err(error) => match project.apply_canonical_fix(&error) {
                Ok(true) => continue,
                Ok(false) => {
                    print_diagnostic(&project, &error);
                    return ExitCode::from(1);
                }
                Err(message) => {
                    print_diagnostic(&project, &error);
                    eprintln!("fix: {message}");
                    return ExitCode::from(1);
                }
            },
        }
    }
    eprintln!("fix: stopped after {LIMIT} rewrites; re-run check");
    ExitCode::from(1)
}

/// `hado fmt` and `build --fix` write every local rewrite (§57) the project's
/// sources offer, in bulk and until none applies (§58.1). `check_only` reports
/// instead of writing. The result says whether any file would change; an error
/// is the exit code to return.
fn format_sources(input: &Path, check_only: bool) -> Result<bool, ExitCode> {
    let files = match hado::project::source_paths(input) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("{}", load_error_report(&error, input));
            return Err(ExitCode::from(1));
        }
    };
    let mut changed = false;
    for (display, physical) in files {
        let source = match std::fs::read_to_string(&physical) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("fmt: {display}: {error}");
                return Err(ExitCode::from(1));
            }
        };
        match hado::fixes::format(&source) {
            Ok(None) => {}
            Ok(Some((updated, count))) => {
                changed = true;
                if check_only {
                    println!("fmt: {display} is not canonical ({count} rewrites)");
                } else {
                    if let Err(error) = std::fs::write(&physical, updated) {
                        eprintln!("fmt: {display}: {error}");
                        return Err(ExitCode::from(1));
                    }
                    println!("fmt: {display} ({count} rewrites)");
                }
            }
            Err(limit) => {
                eprintln!("fmt: {display}: the rewrites did not settle after {limit} passes");
                return Err(ExitCode::from(1));
            }
        }
    }
    Ok(changed)
}

/// The outcome of one `run`-style execution. `run` and `test` differ only in
/// how they report it.
enum Outcome {
    /// The program exited with this code.
    Exited(i32),
    /// The program was terminated by a signal and has no exit code.
    Signalled,
    /// The program could not be loaded, built or started. The message is
    /// already a report for stderr.
    Failed(String),
}

/// Load, link into a private directory, and run the program with `args`
/// (§58). The directory is removed before returning, so nothing is written
/// next to the source.
fn execute(input: &Path, args: &[String], syntax: Syntax) -> Outcome {
    let project = match Project::load_with_syntax(input, syntax) {
        Ok(project) => project,
        Err(error) => return Outcome::Failed(load_error_report(&error, input)),
    };
    let directory = match RunDirectory::create() {
        Ok(directory) => directory,
        Err(message) => return Outcome::Failed(format!("run: {message}")),
    };
    let binary = directory.path().join("program");
    if let Err(message) = project.build(&binary) {
        return Outcome::Failed(message);
    }
    match Command::new(&binary).args(args).status() {
        Ok(status) => match status.code() {
            Some(code) => Outcome::Exited(code),
            None => Outcome::Signalled,
        },
        Err(error) => Outcome::Failed(format!("run: {error}")),
    }
}

/// `hado test <file.hd>` runs the root's `*_test.hd` in turn (§58.2). A test
/// passes when the program exits 0; a `raise` that reaches `main`, a stack
/// overflow and a signal all fail. One failure does not stop the run.
fn run_tests(input: &Path) -> ExitCode {
    let tests = match hado::project::test_paths(input) {
        Ok(tests) => tests,
        Err(error) => {
            eprintln!("{}", load_error_report(&error, input));
            return ExitCode::from(1);
        }
    };
    if tests.is_empty() {
        eprintln!("test: no *_test.hd in the input root");
        return ExitCode::from(1);
    }
    let mut passed = 0;
    for (display, path) in &tests {
        match execute(path, &[], Syntax::Infix) {
            Outcome::Exited(0) => {
                println!("test: {display} ok");
                passed += 1;
            }
            Outcome::Exited(code) => println!("test: {display} failed (exit {code})"),
            Outcome::Signalled => println!("test: {display} failed (signal)"),
            Outcome::Failed(message) => {
                eprintln!("{message}");
                println!("test: {display} failed (build)");
            }
        }
    }
    println!("test: {passed}/{} passed", tests.len());
    if passed == tests.len() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// `run` reports the program's exit code as its own (§58).
fn run_program(input: &Path, args: &[String], syntax: Syntax) -> ExitCode {
    match execute(input, args, syntax) {
        Outcome::Exited(code) => ExitCode::from(code as u8),
        Outcome::Signalled => {
            eprintln!("run: the program was terminated by a signal");
            ExitCode::from(1)
        }
        Outcome::Failed(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

/// A private directory for the executable that `run` links. It is removed on
/// drop, so a faulty program cannot leave a binary behind.
struct RunDirectory(PathBuf);

impl RunDirectory {
    fn create() -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir();
        loop {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!("hado-run-{}-{serial}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for RunDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// §58: `--print-type` prints `main`'s type and effects. §58.3: with a name,
/// that definition's type is printed instead, and a name that is not there is
/// an error rather than silence.
fn print_type(
    checked: &hado::Checked,
    project: &Project,
    type_name: Option<&str>,
    input: &Path,
) -> Result<(), ExitCode> {
    match type_name {
        Some(name) => match checked.type_of(name) {
            Some(ty) => {
                println!("{ty}");
                Ok(())
            }
            None => {
                eprintln!("check: no definition named {name} in {}", input.display());
                Err(ExitCode::from(1))
            }
        },
        // ライブラリには `main` が無い（§52）。
        None if project.is_library() => Ok(()),
        None => {
            println!("{}{}", checked.main, checked.effects.suffix());
            Ok(())
        }
    }
}

// --- postfix-binding-probe の実測用の命令 ---

/// 入力根（ファイルかディレクトリ）の `.hd` を、相対パスを保って集める。
fn probe_paths(cli: &Cli<'_>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for candidate in [cli.path, cli.second_path].into_iter().flatten() {
        let path = PathBuf::from(candidate);
        if path.is_dir() {
            files.extend(hado::project::hd_files_under(&path));
        } else {
            files.push(path);
        }
    }
    files.sort();
    files.dedup();
    files
}

/// `print-postfix <入力根> <出力ディレクトリ>`。旧構文で読める `.hd` を後置
/// 束縛の綴りへ変換し、入力根からの相対パスに書く。先頭に続くコメント行は
/// そのまま写し、本文のコメントは落として行数を数える。
fn print_postfix_command(cli: &Cli<'_>) -> ExitCode {
    let (Some(root_arg), Some(out_arg)) = (cli.path, cli.second_path) else {
        return usage_error();
    };
    let root = PathBuf::from(root_arg);
    let out = PathBuf::from(out_arg);
    let files = if root.is_dir() {
        postfix_input_files(&root)
    } else {
        vec![root.clone()]
    };
    if files.is_empty() {
        eprintln!("print-postfix: no .hd under {}", root.display());
        return ExitCode::from(1);
    }
    let mut skipped = 0usize;
    let mut dropped = 0usize;
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("print-postfix: {}: {error}", file.display());
                return ExitCode::from(1);
            }
        };
        let display = file.to_string_lossy().into_owned();
        let ast = match hado::parser::parse_source(&text, &display) {
            Ok(ast) => ast,
            Err(error) => {
                eprintln!("skip: {display}: {error}");
                skipped += 1;
                continue;
            }
        };
        let (prefix, prefix_lines) = leading_comment_block(&text);
        let mut output = String::new();
        output.push_str(prefix);
        output.push_str(&hado::parser::postfix_program(&ast));
        dropped += text
            .lines()
            .enumerate()
            .filter(|(index, line)| *index >= prefix_lines && is_comment_line(line))
            .count();
        let relative = if root.is_file() {
            PathBuf::from(file.file_name().unwrap_or_default())
        } else {
            file.strip_prefix(&root).unwrap_or(file).to_path_buf()
        };
        let target = out.join(relative);
        if let Some(parent) = target.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                eprintln!("print-postfix: {}: {error}", parent.display());
                return ExitCode::from(1);
            }
        }
        if let Err(error) = std::fs::write(&target, output) {
            eprintln!("print-postfix: {}: {error}", target.display());
            return ExitCode::from(1);
        }
    }
    let interfaces = files
        .iter()
        .filter(|file| file.extension().is_some_and(|extension| extension == "hdi"))
        .count();
    println!(
        "print-postfix: files={} hd={} hdi={} skipped={} dropped-comments={}",
        files.len(),
        files.len() - interfaces,
        interfaces,
        skipped,
        dropped
    );
    ExitCode::SUCCESS
}

/// `.hd` と `.hdi` の両方を集める。`.hdi` は新構文でも読めるよう変換する。
fn postfix_input_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_postfix_files(root, &mut files);
    files.sort();
    files
}

fn collect_postfix_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        match std::fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    collect_postfix_files(&entry.path(), out);
                }
            }
            Err(error) => eprintln!("print-postfix: {}: {error}", path.display()),
        }
    } else if matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("hd" | "hdi")
    ) {
        out.push(path.to_path_buf());
    }
}

/// `probe-postfix <入力根>`。後置構文で読んで、集まった診断（P001 を含む）を
/// 1 行ずつ出す。診断が無ければ `ok`。
fn probe_postfix_command(cli: &Cli<'_>) -> ExitCode {
    let files = probe_paths(cli);
    if files.is_empty() {
        eprintln!("probe-postfix: no input");
        return ExitCode::from(1);
    }
    let mut failed = 0usize;
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) => {
                println!("fail: {}: {error}", file.display());
                failed += 1;
                continue;
            }
        };
        let display = file.to_string_lossy().into_owned();
        let (_, diagnostics) = hado::parser::parse_postfix_probe(&text, &display);
        if diagnostics.is_empty() {
            println!("ok: {display}");
        } else {
            for diagnostic in &diagnostics {
                println!(
                    "{}: {display}: {}",
                    diagnostic.code.clone().unwrap_or_default(),
                    diagnostic.message
                );
            }
            failed += 1;
        }
    }
    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// J1（往復で同じ構文木）と J2（冪等）を、入力根の全 `.hd` で測る。
/// 旧パーサで読めないファイルは `skip` として数える。
fn probe_roundtrip_command(cli: &Cli<'_>) -> ExitCode {
    let files = probe_paths(cli);
    if files.is_empty() {
        eprintln!("probe-postfix-roundtrip: no input");
        return ExitCode::from(1);
    }
    let (mut j1_ok, mut j1_fail, mut j2_ok, mut j2_fail, mut skip) = (0, 0, 0, 0, 0);
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) => {
                println!("j1-fail: {}: {error}", file.display());
                j1_fail += 1;
                continue;
            }
        };
        let display = file.to_string_lossy().into_owned();
        let old = match hado::parser::parse_source(&text, &display) {
            Ok(ast) => ast,
            Err(_) => {
                println!("skip: {display}");
                skip += 1;
                continue;
            }
        };
        let printed = hado::parser::postfix_program(&old);
        match hado::parser::parse_source_syntax(&printed, &display, Syntax::Postfix) {
            Err(error) => {
                println!("j1-fail: {display}: parse_new: {error}");
                println!("j2-fail: {display}: parse_new failed");
                j1_fail += 1;
                j2_fail += 1;
            }
            Ok(new) => {
                if same_ast(&old, &new) {
                    j1_ok += 1;
                } else {
                    println!("j1-fail: {display}: ast mismatch");
                    j1_fail += 1;
                }
                if hado::parser::postfix_program(&new) == printed {
                    j2_ok += 1;
                } else {
                    println!("j2-fail: {display}: not idempotent");
                    j2_fail += 1;
                }
            }
        }
    }
    println!(
        "roundtrip: j1-ok={j1_ok} j1-fail={j1_fail} j2-ok={j2_ok} j2-fail={j2_fail} skip={skip}"
    );
    if j1_fail + j2_fail > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// `probe-postfix-compare <旧構文> <新構文>`。両者が同じ構文木になるか。
fn probe_compare_command(cli: &Cli<'_>) -> ExitCode {
    let (Some(old_arg), Some(new_arg)) = (cli.path, cli.second_path) else {
        return usage_error();
    };
    let old_text = match std::fs::read_to_string(old_arg) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("probe-postfix-compare: {old_arg}: {error}");
            return ExitCode::from(1);
        }
    };
    let new_text = match std::fs::read_to_string(new_arg) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("probe-postfix-compare: {new_arg}: {error}");
            return ExitCode::from(1);
        }
    };
    let old = match hado::parser::parse_source(&old_text, old_arg) {
        Ok(ast) => ast,
        Err(error) => {
            eprintln!("probe-postfix-compare: old: {error}");
            return ExitCode::from(1);
        }
    };
    let new = match hado::parser::parse_source_syntax(&new_text, new_arg, Syntax::Postfix) {
        Ok(ast) => ast,
        Err(error) => {
            eprintln!("probe-postfix-compare: new: {error}");
            return ExitCode::from(1);
        }
    };
    if same_ast(&old, &new) {
        println!("compare: ok");
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "compare: mismatch\nold: {}\nnew: {}",
            hado::parser::canonical_program(&old),
            hado::parser::canonical_program(&new)
        );
        ExitCode::from(1)
    }
}

/// 構文木の一致。`Node` の `PartialEq` は span を見ないので、項目の正準形と
/// `library` の有無で比べる。
fn same_ast(left: &hado::parser::Ast, right: &hado::parser::Ast) -> bool {
    left.library.is_some() == right.library.is_some()
        && hado::parser::canonical_program(left) == hado::parser::canonical_program(right)
}

/// ファイル先頭に続く、空行かコメント行だけの塊。写す綴りと、その行数。
fn leading_comment_block(text: &str) -> (&str, usize) {
    let mut end = 0usize;
    let mut lines = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            end += line.len();
            lines += 1;
        } else {
            break;
        }
    }
    (&text[..end], lines)
}

/// `//` のコメントを含む行か。文字列と文字リテラルの中は数えない。
fn is_comment_line(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let mut index = 0usize;
    let (mut string, mut character, mut escape) = (false, false, false);
    while index < chars.len() {
        let current = chars[index];
        if escape {
            escape = false;
        } else if string {
            match current {
                '\\' => escape = true,
                '"' => string = false,
                _ => {}
            }
        } else if character {
            match current {
                '\\' => escape = true,
                '\'' => character = false,
                _ => {}
            }
        } else {
            match current {
                '"' => string = true,
                '\'' => character = true,
                '/' if chars.get(index + 1) == Some(&'/') => return true,
                _ => {}
            }
        }
        index += 1;
    }
    false
}

fn usage() {
    eprintln!(
        "usage: hado check [--print-type[=<name>]] [--fix] [--syntax postfix] <file> | dump-mono [--syntax postfix] <file> | build [-o <path>] [--fix] <file.hd> | run [--syntax postfix] <file.hd> [-- <arg>...] | fmt [--check] <file.hd> | test <file.hd> | print-postfix <root> <out> | probe-postfix[-roundtrip|-compare] <args> | lsp"
    );
}
