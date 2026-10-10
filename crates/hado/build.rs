fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let runtime = format!("{manifest}/../../runtime/hado_runtime.c");
    println!("cargo::rerun-if-changed={runtime}");
    println!("cargo::rerun-if-changed={manifest}/../../runtime/hado_runtime.h");
    println!("cargo::rerun-if-changed={manifest}/runtime_print.c");
    println!("cargo::rerun-if-changed={manifest}/float_format.rs");
    let output = std::env::var("OUT_DIR").unwrap();
    let status = std::process::Command::new(std::env::var("RUSTC").unwrap())
        .args([
            "--crate-type",
            "staticlib",
            "--edition=2021",
            "--crate-name",
            "hado_float_format",
        ])
        .args([
            "-C",
            "panic=abort",
            "-C",
            "opt-level=2",
            "-C",
            "relocation-model=pic",
        ])
        .arg("--target")
        .arg(std::env::var("TARGET").unwrap())
        .arg(format!("{manifest}/float_format.rs"))
        .arg("-o")
        .arg(format!("{output}/libhado_float_format.a"))
        .status()
        .expect("compile Float formatter");
    assert!(status.success(), "Float formatter compilation failed");
    cc::Build::new()
        .file(&runtime)
        .file(format!("{manifest}/runtime_print.c"))
        .cargo_metadata(false)
        .compile("hado_runtime");
    // REPL の JIT は、ゲストと同じ C ランタイムを実行中のプロセスへ渡す。
    // ゲスト向けの `libhado_runtime.a` を `hado` 自身へ静的リンクすると、
    // Float 整形の `libhado_float_format.a`（`panic=abort` の Rust staticlib）
    // が `__rustc::rust_begin_unwind` を std と二重に定義してしまう。そこで
    // ランタイムだけの共有ライブラリを作り、REPL が `dlopen` して
    // `JITBuilder::symbol` に住所を渡す。`hado` 本体は libgc に依存しない。
    let shared = format!("{output}/libhado_repl_runtime.so");
    let status = std::process::Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".into()))
        .args(["-shared", "-fPIC", "-ffunction-sections", "-fdata-sections"])
        .arg(&runtime)
        .arg(format!("{manifest}/runtime_print.c"))
        .arg(format!("{output}/libhado_float_format.a"))
        // ゲスト向けの `main` は `.so` に残る（`--gc-sections` は入口として
        // 残す）。その `hado_entry` の参照と、panic=abort では呼ばれない
        // `rust_eh_personality` の参照が未定義のまま残り、`dlopen(RTLD_NOW)`
        // が失敗する。落ちきらない参照は 0 に束ねる。どちらも JIT からは
        // 呼ばない（JIT は `RuntimeFunction` の記号しか引かない）。
        .args([
            "-Wl,--gc-sections",
            "-Wl,--defsym=rust_eh_personality=0",
            "-Wl,--defsym=hado_entry=0",
            "-lgc",
            "-o",
        ])
        .arg(&shared)
        .status()
        .expect("link the REPL runtime");
    assert!(status.success(), "REPL runtime link failed");
}
