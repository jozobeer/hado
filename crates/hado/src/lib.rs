mod file_scopes;
mod functor_instances;
mod namespaces;
pub mod build;
pub mod project;
pub mod fixes;
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

/// Absolute path of the static runtime library (`libhado_runtime.a`)
/// built by `build.rs` from `runtime/`.
pub fn runtime_lib_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("OUT_DIR")).join("libhado_runtime.a")
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
        .arg(std::path::PathBuf::from(env!("OUT_DIR")).join("libhado_float_format.a"))
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
mod contract_types;
mod name_resolution;
mod named_patterns;

#[derive(Clone)]
pub struct Checked {
    pub instantiated_modules: Vec<parser::ModuleImpl>,
    pub abstract_representations: std::collections::HashMap<String, types::TypeRealization>,
    pub module_constructors: std::collections::HashMap<String, (u32, types::Scheme)>,
    pub module_named_constructors: std::collections::HashMap<String, Vec<String>>,
    pub named_constructors: std::collections::HashMap<String, Vec<String>>,
    pub module_records: std::collections::HashMap<String, (Vec<String>, types::Scheme)>,
    pub module_definitions: std::collections::HashMap<String, types::Scheme>,
    pub modules: std::collections::HashMap<String, std::collections::HashMap<String, types::Scheme>>,
    pub constructor_tags: std::collections::HashMap<String, u32>,
    pub constructors: std::collections::HashMap<String, types::Scheme>,
    pub records: std::collections::HashMap<String, (Vec<String>, types::Scheme)>,
    pub composite_types: std::collections::HashMap<(usize, usize), types::Ty>,
    pub value_types: std::collections::HashMap<(usize, usize), types::Ty>,
    pub main: types::Ty,
    pub effects: types::Row,
    pub definitions: std::collections::HashMap<String, types::Scheme>,
    pub default_operations: std::collections::HashMap<String, String>,
    pub operations: std::collections::HashMap<String, parser::OpKind>,
}

impl Checked {
    pub(crate) fn for_codegen(&self) -> Self {
        let mut checked = self.clone();
        checked.records.extend(self.module_records.clone());
        checked.named_constructors.extend(self.module_named_constructors.clone());
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

/// Whether the program passed to `Project::load` (or the single-file API) has an
/// entry `main`, or is checked as a library (A1, §10, §50, §52).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Inspection { Entry, Library }

struct ParsedSource {
    ast: parser::Ast,
    inspection: Inspection,
    trusted_start: Option<usize>,
    prelude_items: usize,
    other_entry_roots: Vec<String>,
    namespace_diagnostics: Vec<parser::Diagnostic>,
    namespace_anchors: Vec<(parser::Span, parser::Span)>,
}

fn parse_with_prelude(source: &str, path: &str) -> Result<ParsedSource, parser::Diagnostic> {
    let ast = parser::parse_source(source, path)?;
    let mut parsed = with_prelude(ast, path, source.lines().count())?;
    resolve_namespaces(&mut parsed, &[(0, Vec::new())], &std::collections::BTreeSet::from([Vec::new()]), &std::collections::BTreeSet::new(), path);
    Ok(parsed)
}

fn resolve_namespaces(parsed: &mut ParsedSource, files: &[(usize, Vec<String>)], directories: &std::collections::BTreeSet<Vec<String>>, libraries: &std::collections::BTreeSet<Vec<String>>, path: &str) {
    let resolved = namespaces::resolve(&mut parsed.ast, files, directories, libraries, parsed.prelude_items, path);
    parsed.namespace_diagnostics = resolved.diagnostics;
    parsed.namespace_anchors = resolved.error_anchors;
}

fn with_prelude(mut ast: parser::Ast, path: &str, source_lines: usize) -> Result<ParsedSource, parser::Diagnostic> {
    let inspection = if has_main_definition(&ast) { Inspection::Entry } else { Inspection::Library };
    let mut prelude = Vec::new();
    for (name, source) in [
        ("stdlib/list.hd", include_str!("../../../stdlib/list.hd")),
        ("stdlib/option.hd", include_str!("../../../stdlib/option.hd")),
        ("stdlib/error.hd", include_str!("../../../stdlib/error.hd")),
    ] {
        prelude.extend(parser::parse_source(source, name)?.items.into_iter().filter(|item| matches!(item, parser::Item::TypeDef(_))));
    }
    let mut trusted_start = None;
    if !std::path::Path::new(path).components().any(|part| part.as_os_str() == "stdlib") {
        let mut line = source_lines + 2;
        trusted_start = Some(line + 1);
        for module in [
            include_str!("../../../stdlib/error.hd"),
            include_str!("../../../stdlib/env.hd"),
            include_str!("../../../stdlib/io.hd"),
            include_str!("../../../stdlib/list.hd"),
            include_str!("../../../stdlib/option.hd"),
            include_str!("../../../stdlib/array.hd"),
            include_str!("../../../stdlib/string.hd"), include_str!("../../../stdlib/bytes.hd"), include_str!("../../../stdlib/int.hd"), include_str!("../../../stdlib/float.hd")] {
            let padded = "\n".repeat(line) + module;
            let parsed = parser::parse_source(&padded, path)?;
            ast.branch_anchors.extend(parsed.branch_anchors);
            ast.equality_anchors.extend(parsed.equality_anchors);
            ast.qualified_type_anchors.extend(parsed.qualified_type_anchors);
            ast.name_sites.extend(parsed.name_sites);
            ast.written_list_patterns.extend(parsed.written_list_patterns);
            ast.long_condition_functions.extend(parsed.long_condition_functions);
            ast.branches_after_fallback.extend(parsed.branches_after_fallback);
            prelude.extend(parsed.items.into_iter().filter(|item| !matches!(item, parser::Item::TypeDef(_))));
            line += module.lines().count() + 1;
        }
    }
    let prelude_items = prelude.len() + if trusted_start.is_none() { ast.items.len() } else { 0 };
    prelude.append(&mut ast.items);
    ast.items = prelude;
    Ok(ParsedSource { ast, inspection, trusted_start, prelude_items, other_entry_roots: Vec::new(), namespace_diagnostics: Vec::new(), namespace_anchors: Vec::new() })
}

pub fn check_source(source: &str, path: &str) -> Result<Checked, parser::Diagnostic> {
    check_parsed(&parse_with_prelude(source, path)?, path)
}

fn check_parsed(parsed: &ParsedSource, path: &str) -> Result<Checked, parser::Diagnostic> {
    check_parsed_filtered(parsed, path, true)
}

pub(crate) fn check_parsed_for_print(parsed: &ParsedSource, path: &str) -> Result<Checked, parser::Diagnostic> {
    check_parsed_filtered(parsed, path, false)
}

fn check_parsed_filtered(parsed: &ParsedSource, path: &str, reject_user_main_effects: bool) -> Result<Checked, parser::Diagnostic> {
    let ParsedSource { ast, inspection, trusted_start, prelude_items, other_entry_roots, namespace_diagnostics, namespace_anchors } = parsed;
    let adjust = |error| namespace_diagnostic(check::arm_diagnostic(ast, error), namespace_anchors);
    let name_error = names::resolve_with_prelude(ast, path, *trusted_start, *inspection).err();
    let (checked, mut diagnostics) = match infer::infer_with_diagnostics(ast, path, *trusted_start, *inspection, *prelude_items) {
        Ok(result) => result,
        Err(error) => return Err(name_error.into_iter().chain(std::iter::once(error)).chain(namespace_diagnostics.iter().cloned()).map(adjust).min_by_key(diagnostic_order).unwrap()),
    };
    diagnostics.extend(name_error);
    diagnostics.extend(namespace_diagnostics.iter().cloned());
    if let Err(error) = check::check_ast(ast, path) {
        diagnostics.push(error);
    }
    diagnostics.extend(dependencies::diagnostics(ast, &checked, *prelude_items, other_entry_roots, *inspection, path));
    // Why not keep E706 for --print-type: r24/r25 print `String {Ask}` on main.
    if !reject_user_main_effects {
        diagnostics.retain(|error| error.code.as_deref() != Some("E706"));
    }
    match diagnostics.into_iter().map(adjust).min_by_key(diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(checked),
    }
}

fn namespace_diagnostic(mut error: parser::Diagnostic, anchors: &[(parser::Span, parser::Span)]) -> parser::Diagnostic {
    if error.code.as_deref() == Some("E204") {
        if let Some((_, target)) = anchors.iter().find(|(source, _)| source.line == error.line && source.col == error.col) {
            error.line = target.line;
            error.col = target.col;
        }
    }
    error
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
mod type_realizations;
mod ambiguous_declarations;

pub mod names;

pub mod check;

mod normalize;

mod coverage;

pub mod mono;

pub fn dump_mono(source: &str, path: &str) -> Result<String, parser::Diagnostic> {
    dump_parsed(&parse_with_prelude(source, path)?, path)
}

pub(crate) fn entry_required(path: &str) -> parser::Diagnostic {
    parser::Diagnostic { path: path.into(), line: 1, col: 1, code: Some("E206".into()),
        message: "build requires an entry file with main; check treats this directory as a library".into() }
}

/// The span of the `def main` of a file, whatever its form (§52). `Ast::main` is
/// only the value definition used for lowering; an argument-taking or pattern
/// `main` is still a declaration the entry check must anchor to.
pub(crate) fn main_definition_span(ast: &parser::Ast) -> Option<parser::Span> {
    ast.items.iter().find_map(|item| match item {
        parser::Item::Def(def) if def.name == "main" => Some(def.span),
        _ => None,
    })
}

/// Whether an entry `main` exists, whatever its form. The inspection is decided
/// by this: only a file without any `def main` is checked as a library (A1, §50).
pub(crate) fn has_main_definition(ast: &parser::Ast) -> bool {
    main_definition_span(ast).is_some()
}

fn dump_parsed(parsed: &ParsedSource, path: &str) -> Result<String, parser::Diagnostic> {
    if parsed.inspection == Inspection::Library {
        return Err(entry_required(path));
    }
    let checked = check_parsed(parsed, path)?;
    let ast = &parsed.ast;
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
