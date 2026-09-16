use hado::project::Project;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        usage();
        return ExitCode::from(2);
    };
    let mut print = false;
    let mut fix = false;
    let mut path: Option<&str> = None;
    for arg in &args[1..] {
        match arg.as_str() {
            "--print-type" if command == "check" => print = true,
            "--fix" if command == "check" => fix = true,
            value if !value.starts_with("--") && path.is_none() => path = Some(value),
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    if !matches!(command, "check" | "dump-mono" | "build") {
        usage();
        return ExitCode::from(2);
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
    let project = match Project::load(input) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    if command == "build" {
        return match project.build(&input.with_extension("")) {
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

fn usage() {
    eprintln!(
        "usage: hado check [--print-type] [--fix] <file> | dump-mono <file> | build <file.hd>"
    );
}
