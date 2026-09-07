fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let runtime = format!("{manifest}/../../runtime/shin_runtime.c");
    println!("cargo::rerun-if-changed={runtime}");
    println!("cargo::rerun-if-changed={manifest}/../../runtime/shin_runtime.h");
    println!("cargo::rerun-if-changed={manifest}/runtime_print.c");
    println!("cargo::rerun-if-changed={manifest}/float_format.rs");
    let output = std::env::var("OUT_DIR").unwrap();
    let status = std::process::Command::new(std::env::var("RUSTC").unwrap())
        .args(["--crate-type", "staticlib", "--edition=2021", "--crate-name", "shin_float_format"])
        .args(["-C", "panic=abort", "-C", "opt-level=2", "-C", "relocation-model=pic"])
        .arg("--target").arg(std::env::var("TARGET").unwrap())
        .arg(format!("{manifest}/float_format.rs"))
        .arg("-o").arg(format!("{output}/libshin_float_format.a"))
        .status().expect("compile Float formatter");
    assert!(status.success(), "Float formatter compilation failed");
    cc::Build::new()
        .file(&runtime)
        .file(format!("{manifest}/runtime_print.c"))
        .cargo_metadata(false)
        .compile("shin_runtime");
}
