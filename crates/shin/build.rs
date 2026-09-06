fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let runtime = format!("{manifest}/../../runtime/shin_runtime.c");
    println!("cargo::rerun-if-changed={runtime}");
    println!("cargo::rerun-if-changed={manifest}/../../runtime/shin_runtime.h");
    cc::Build::new()
        .file(&runtime)
        .cargo_metadata(false)
        .compile("shin_runtime");
}
