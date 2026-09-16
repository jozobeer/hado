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
    if !matches!(command, "check" | "dump-mono" | "build" | "run") {
        usage();
        return ExitCode::from(2);
    }
    let mut print = false;
    let mut fix = false;
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
            "--fix" if command == "check" => fix = true,
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
    if fix {
        return check_with_fix(input, print);
    }
    if command == "run" {
        return run_program(input, &program_args);
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

/// `run` links the program into a private directory, runs it with the given
/// arguments, and returns the program's exit code (§58). The directory is
/// removed afterwards, so nothing is written next to the source.
fn run_program(input: &Path, args: &[String]) -> ExitCode {
    let project = match Project::load(input) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let directory = match RunDirectory::create() {
        Ok(directory) => directory,
        Err(message) => {
            eprintln!("run: {message}");
            return ExitCode::from(1);
        }
    };
    let binary = directory.path().join("program");
    if let Err(message) = project.build(&binary) {
        eprintln!("{message}");
        return ExitCode::from(1);
    }
    match Command::new(&binary).args(args).status() {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from(code as u8),
            None => {
                eprintln!("run: the program was terminated by a signal");
                ExitCode::from(1)
            }
        },
        Err(error) => {
            eprintln!("run: {error}");
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
        "usage: hado check [--print-type] [--fix] <file> | dump-mono <file> | build [-o <path>] <file.hd> | run <file.hd> [-- <arg>...]"
    );
}
