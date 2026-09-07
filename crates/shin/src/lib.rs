pub mod build;
pub mod emit;
pub mod lexer;
pub mod parser;
pub mod types;

use std::path::Path;

/// Check, specialize, and compile source to a native object file.
pub fn compile_to_object(source: &str, obj_path: &Path) -> Result<(), String> {
    compile_source_to_object(source, "main", obj_path)
}

pub fn compile_source_to_object(source: &str, source_path: &str, obj_path: &Path) -> Result<(), String> {
    let checked = check_source(source, source_path).map_err(|e| e.to_string())?;
    let ParsedSource { ast, .. } = parse_with_prelude(source, source_path).map_err(|e| e.to_string())?;
    let program = mono::lower_program(&ast, &checked)?;
    emit::compile_mono_to_object(&program, &checked, obj_path)
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
        .arg(std::path::PathBuf::from(env!("OUT_DIR")).join("libshin_float_format.a"))
        .arg("-Wl,--gc-sections")
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

pub mod infer;
mod dependencies;
mod name_resolution;

#[derive(Clone)]
pub struct Checked {
    pub abstract_representations: std::collections::HashMap<String, types::Ty>,
    pub module_constructors: std::collections::HashMap<String, (u32, types::Scheme)>,
    pub module_records: std::collections::HashMap<String, (Vec<String>, types::Scheme)>,
    pub module_definitions: std::collections::HashMap<String, types::Scheme>,
    pub modules: std::collections::HashMap<String, std::collections::HashMap<String, types::Scheme>>,
    pub constructor_tags: std::collections::HashMap<String, u32>,
    pub constructors: std::collections::HashMap<String, types::Scheme>,
    pub records: std::collections::HashMap<String, (Vec<String>, types::Scheme)>,
    pub composite_types: std::collections::HashMap<(usize, usize), types::Ty>,
    pub value_types: std::collections::HashMap<(usize, usize), types::Ty>,
    pub main: types::Ty,
    pub definitions: std::collections::HashMap<String, types::Scheme>,
}

impl Checked {
    pub(crate) fn for_codegen(&self) -> Self {
        let mut checked = self.clone();
        checked.records.extend(self.module_records.clone());
        for (name, (tag, scheme)) in &self.module_constructors {
            checked.constructors.insert(name.clone(), scheme.clone());
            checked.constructor_tags.insert(name.clone(), *tag);
        }
        let names = &self.abstract_representations;
        for scheme in checked.definitions.values_mut().chain(checked.module_definitions.values_mut()).chain(checked.constructors.values_mut()) {
            *scheme = scheme.reveal(names);
        }
        for (_, scheme) in checked.records.values_mut() { *scheme = scheme.reveal(names); }
        for ty in checked.value_types.values_mut().chain(checked.composite_types.values_mut()) {
            *ty = ty.reveal(names);
        }
        checked.main = checked.main.reveal(names);
        checked
    }
}

struct ParsedSource {
    ast: parser::Ast,
    trusted_start: Option<usize>,
    prelude_items: usize,
}

fn parse_with_prelude(source: &str, path: &str) -> Result<ParsedSource, parser::Diagnostic> {
    let mut ast = parser::parse_source(source, path)?;
    let mut prelude = Vec::new();
    for (name, source) in [
        ("stdlib/division_error.shin", include_str!("../../../stdlib/division_error.shin")),
        ("stdlib/list.shin", include_str!("../../../stdlib/list.shin")),
        ("stdlib/option.shin", include_str!("../../../stdlib/option.shin")),
        ("stdlib/result.shin", include_str!("../../../stdlib/result.shin")),
    ] {
        prelude.extend(parser::parse_source(source, name)?.items.into_iter().filter(|item| matches!(item, parser::Item::TypeDef(_))));
    }
    let mut trusted_start = None;
    if !std::path::Path::new(path).components().any(|part| part.as_os_str() == "stdlib") {
        let mut line = source.lines().count() + 2;
        trusted_start = Some(line + 1);
        for module in [
            include_str!("../../../stdlib/list.shin"),
            include_str!("../../../stdlib/option.shin"),
            include_str!("../../../stdlib/result.shin"),include_str!("../../../stdlib/string.shin"), include_str!("../../../stdlib/int.shin"), include_str!("../../../stdlib/float.shin")] {
            let padded = "\n".repeat(line) + module;
            let parsed = parser::parse_source(&padded, path)?;
            ast.branch_anchors.extend(parsed.branch_anchors);
            ast.branches_after_fallback.extend(parsed.branches_after_fallback);
            prelude.extend(parsed.items.into_iter().filter(|item| !matches!(item, parser::Item::TypeDef(_))));
            line += module.lines().count() + 1;
        }
    }
    let prelude_items = prelude.len() + if trusted_start.is_none() { ast.items.len() } else { 0 };
    prelude.append(&mut ast.items);
    ast.items = prelude;
    Ok(ParsedSource { ast, trusted_start, prelude_items })
}

pub fn check_source(source: &str, path: &str) -> Result<Checked, parser::Diagnostic> {
    let ParsedSource { ast, trusted_start, prelude_items } = parse_with_prelude(source, path)?;
    names::resolve_with_prelude(&ast, path, trusted_start)?;
    type_cycles::check(&ast, path)?;
    let (checked, mut diagnostics) = infer::infer_with_diagnostics(&ast, path, trusted_start)?;
    if let Err(error) = check::check_ast(&ast, path) {
        diagnostics.push(error);
    }
    diagnostics.extend(dependencies::diagnostics(&ast, &checked, prelude_items, path));
    match diagnostics.into_iter().map(|error| check::arm_diagnostic(&ast, error)).min_by_key(diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(checked),
    }
}

pub(crate) fn diagnostic_order(error: &parser::Diagnostic) -> (u8, usize, usize) {
    let stage = match error.code.as_deref().and_then(|code| code.as_bytes().get(1)) {
        Some(b'1') => 1,
        Some(b'2') => 2,
        Some(b'3' | b'6') => 3,
        Some(b'4') => 4,
        Some(b'5') => 5,
        _ => 6,
    };
    (stage, error.line, error.col)
}

mod type_cycles;

pub mod names;

pub mod check;

mod normalize;

mod coverage;

pub mod mono;

pub fn dump_mono(source: &str, path: &str) -> Result<String, parser::Diagnostic> {
    let checked = check_source(source, path)?;
    let ParsedSource { ast, .. } = parse_with_prelude(source, path)?;
    let main = ast.main().ok_or_else(|| parser::Diagnostic {
        path: path.into(), line: ast.span.line, col: ast.span.col, code: None,
        message: "main must be a value definition for lowering".into(),
    })?;
    let program = mono::lower_program(&ast, &checked).map_err(|message| parser::Diagnostic {
        path: path.into(), line: main.span.line, col: main.span.col, code: None, message,
    })?;
    let mut output = format!("fn main() -> {}\n{:?}\n", checked.main, program.entry);
    let printer_id = mono::FunctionId(program.functions.iter().map(|function| function.id.0).max().map_or(1, |id| id + 2));
    let printer = mono::derive_printers(printer_id, program.entry.ty(), &checked);
    for function in program.functions { output.push_str(&format!("{function:?}\n")); }
    match printer {
        Ok(functions) => for function in functions { output.push_str(&format!("{function:?}\n")); },
        Err(message) => output.push_str(&format!("; main printer unavailable: {message}\n")),
    }
    Ok(output)
}

pub mod runtime_abi;

pub mod repr;
