pub mod emit;
pub mod lexer;
pub mod parser;
pub mod types;

pub use emit::compile_to_object;

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
