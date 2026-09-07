fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let runtime = format!("{manifest}/../../runtime/shin_runtime.c");
    println!("cargo::rerun-if-changed={runtime}");
    println!("cargo::rerun-if-changed={manifest}/../../runtime/shin_runtime.h");
    println!("cargo::rerun-if-changed={manifest}/runtime_print.c");
    cc::Build::new()
        .file(&runtime)
        .file(format!("{manifest}/runtime_print.c"))
        .cargo_metadata(false)
        .compile("shin_runtime");
}
