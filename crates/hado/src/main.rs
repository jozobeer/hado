use hado::project::Project;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicU64, Ordering};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        usage();
        return ExitCode::from(2);
    };
    if !matches!(command, "check" | "dump-mono" | "build" | "run" | "fmt" | "test") {
        usage();
        return ExitCode::from(2);
    }
    let mut print = false;
    let mut fix = false;
    let mut check_only = false;
    let mut output: Option<&str> = None;
    let mut path: Option<&str> = None;
    let mut program_args: Vec<String> = Vec::new();
    let rest = &args[1..];
    let mut index = 0;
    while index < rest.len() {
        let arg = rest[index].as_str();
        match arg {
            // `run` の `--` の後だけが子プロセスの引数である。
            "--" if command == "run" => {
                program_args.extend(rest[index + 1..].iter().cloned());
                break;
            }
            "--print-type" if command == "check" => print = true,
            "--fix" if command == "check" || command == "build" => fix = true,
            "--check" if command == "fmt" => check_only = true,
            "-o" | "--output" if command == "build" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    usage();
                    return ExitCode::from(2);
                };
                output = Some(value.as_str());
            }
            value if !value.starts_with('-') && path.is_none() => path = Some(value),
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
        index += 1;
    }
    let Some(path) = path else {
        usage();
        return ExitCode::from(2);
    };
    let input = Path::new(path);
    if input.extension().and_then(|extension| extension.to_str()) != Some("hd") {
        eprintln!("input must have the .hd extension");
        return ExitCode::from(1);
    }
    if command == "fmt" {
        return match format_sources(input, check_only) {
            Ok(changed) if check_only && changed => ExitCode::from(1),
            Ok(_) => ExitCode::SUCCESS,
            Err(code) => code,
        };
    }
    if fix && command == "check" {
        return check_with_fix(input, print);
    }
    if command == "run" {
        return run_program(input, &program_args);
    }
    if command == "test" {
        return run_tests(input);
    }
    if fix && command == "build" {
        // §58.1: `build --fix` writes the canonical form before reading the
        // project, so the build sees the formatted sources.
        if let Err(code) = format_sources(input, false) {
            return code;
        }
    }
    let project = match Project::load(input) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    if command == "build" {
        let target = match output {
            Some(output) => PathBuf::from(output),
            None => input.with_extension(""),
        };
        return match project.build(&target) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(1)
            }
        };
    }
    if command == "dump-mono" {
        return match project.dump_mono() {
            Ok(output) => {
                print!("{output}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{}", project.report(&error));
                ExitCode::from(1)
            }
        };
    }
    match if print { project.check_for_print() } else { project.check() } {
        Ok(checked) => {
            if print && !project.is_library() {
                println!("{}{}", checked.main, checked.effects.suffix());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", project.report(&error));
            ExitCode::from(1)
        }
    }
}

/// `check --fix` applies the canonical rewrites at the reported anchor and
/// re-checks, until a check passes or a diagnostic has no local rewrite.
/// The default `check` / `build` never write to a source file.
fn check_with_fix(input: &Path, print: bool) -> ExitCode {
    const LIMIT: usize = 16;
    for _ in 0..LIMIT {
        let project = match Project::load(input) {
            Ok(project) => project,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(1);
            }
        };
        match project.check() {
            Ok(checked) => {
                if print && !project.is_library() {
                    println!("{}{}", checked.main, checked.effects.suffix());
                }
                return ExitCode::SUCCESS;
            }
            Err(error) => match project.apply_canonical_fix(&error) {
                Ok(true) => continue,
                Ok(false) => {
                    eprintln!("{}", project.report(&error));
                    return ExitCode::from(1);
                }
                Err(message) => {
                    eprintln!("{}", project.report(&error));
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
            eprintln!("{error}");
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
fn execute(input: &Path, args: &[String]) -> Outcome {
    let project = match Project::load(input) {
        Ok(project) => project,
        Err(error) => return Outcome::Failed(error.to_string()),
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
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    if tests.is_empty() {
        eprintln!("test: no *_test.hd in the input root");
        return ExitCode::from(1);
    }
    let mut passed = 0;
    for (display, path) in &tests {
        match execute(path, &[]) {
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
    if passed == tests.len() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

/// `run` reports the program's exit code as its own (§58).
fn run_program(input: &Path, args: &[String]) -> ExitCode {
    match execute(input, args) {
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

fn usage() {
    eprintln!(
        "usage: hado check [--print-type] [--fix] <file> | dump-mono <file> | build [-o <path>] [--fix] <file.hd> | run <file.hd> [-- <arg>...] | fmt [--check] <file.hd> | test <file.hd>"
    );
}
