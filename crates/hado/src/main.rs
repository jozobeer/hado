use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (path, print, dump, build) = match args.as_slice() {
        [command, path] if command == "check" => (path, false, false, false),
        [command, flag, path] if command == "check" && flag == "--print-type" => {
            (path, true, false, false)
        }
        [command, path] if command == "dump-mono" => (path, false, true, false),
        [command, path] if command == "build" => (path, false, false, true),
        _ => {
            eprintln!("usage: hado check [--print-type] <file> | dump-mono <file> | build <file.hd>");
            return ExitCode::from(2);
        }
    };
    let input = std::path::Path::new(path);
    if input.extension().and_then(|extension| extension.to_str()) != Some("hd") {
        eprintln!("input must have the .hd extension");
        return ExitCode::from(1);
    }
    let project = match hado::project::Project::load(input) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    if build {
        return match project.build(&input.with_extension("")) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        };
    }
    if dump {
        return match project.dump_mono() {
            Ok(output) => {
                print!("{output}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        };
    }
    match if print { project.check_for_print() } else { project.check() } {
        Ok(checked) => {
            if print {
                println!("{}{}", checked.main, checked.effects.suffix());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
