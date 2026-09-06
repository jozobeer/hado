pub mod emit;
pub mod lexer;
pub mod parser;
pub mod types;

use std::path::Path;

/// Compile a source to an object file.
///
/// Glue: parse the full grammar, lower only the M0 subset
/// (`def main = <Int arithmetic>`) into the M0 `Program`, check it, and
/// pass it to the existing emitter. Anything beyond the subset fails with
/// an `M3+` error after a successful parse.
pub fn compile_to_object(source: &str, obj_path: &Path) -> Result<(), String> {
    let ast = parser::parse_source(source, "main").map_err(|d| d.to_string())?;
    let prog = parser::lower_to_m0(&ast).ok_or_else(|| {
        "M3+ 以降で対応: only `def main = <Int arithmetic>` can be compiled yet".to_string()
    })?;
    let ty = types::check(&prog)?;
    if ty != types::Ty::Int {
        return Err(format!("main must be Int, found {ty:?}"));
    }
    emit::compile_to_object(&prog, obj_path)
}

/// Absolute path of the static runtime library (`libshin_runtime.a`)
/// built by `build.rs` from `runtime/`.
pub fn runtime_lib_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("OUT_DIR")).join("libshin_runtime.a")
}

/// Link a guest object with the runtime static library and `-lgc`
/// into an executable using the host `cc`.
pub fn link_executable(
    obj_path: &std::path::Path,
    exe_path: &std::path::Path,
) -> Result<(), String> {
    let status = std::process::Command::new("cc")
        .arg(obj_path)
        .arg(runtime_lib_path())
        .arg("-lgc")
        .arg("-o")
        .arg(exe_path)
        .status()
        .map_err(|e| format!("spawn cc: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cc link failed: {status:?}"))
    }
}
