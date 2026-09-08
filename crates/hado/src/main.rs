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
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}:1:1: error: {error}");
            return ExitCode::from(1);
        }
    };
    if build {
        let input = std::path::Path::new(path);
        if input.extension().and_then(|s| s.to_str()) != Some("hd") {
            eprintln!("build input must have the .hd extension");
            return ExitCode::from(1);
        }
        return match hado::build::executable(&source, path, &input.with_extension("")) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        };
    }
    if dump {
        return match hado::dump_mono(&source, path) {
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
    match hado::check_source(&source, path) {
        Ok(checked) => {
            if print {
                println!("{}", checked.main);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
