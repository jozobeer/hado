pub mod build;
pub mod emit;
mod file_scopes;
pub mod fixes;
mod functor_instances;
pub mod lexer;
pub mod lsp;
mod namespaces;
pub mod parser;
pub mod project;
pub mod types;

use std::path::{Path, PathBuf};

/// Check, specialize, and compile source to a native object file.
pub fn compile_to_object(source: &str, obj_path: &Path) -> Result<(), String> {
    compile_source_to_object(source, "main", obj_path)
}

pub fn compile_source_to_object(
    source: &str,
    source_path: &str,
    obj_path: &Path,
) -> Result<(), String> {
    let checked = check_source(source, source_path).map_err(|e| e.to_string())?;
    let ParsedSource { ast, .. } =
        parse_with_prelude(source, source_path).map_err(|e| e.to_string())?;
    let program = mono::lower_program(&ast, &checked)?;
    emit::compile_mono_to_object(&program, &checked, obj_path)
}

/// The static libraries every guest links against, in link order: the C
/// runtime built by `build.rs` from `runtime/` and the Float formatter.
/// They are embedded rather than referenced by their `OUT_DIR` path so an
/// installed `hado` keeps linking after its build directory is gone.
const RUNTIME_ARCHIVES: [(&str, &[u8]); 2] = [
    (
        "libhado_runtime.a",
        include_bytes!(concat!(env!("OUT_DIR"), "/libhado_runtime.a")),
    ),
    (
        "libhado_float_format.a",
        include_bytes!(concat!(env!("OUT_DIR"), "/libhado_float_format.a")),
    ),
];

/// Write the embedded archives next to `obj_path`, prefixed with its stem so
/// links of different objects in one directory do not share files.
fn write_runtime_archives(obj_path: &Path) -> Result<Vec<PathBuf>, String> {
    let dir = obj_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stem = obj_path.file_stem().unwrap_or_default().to_string_lossy();
    RUNTIME_ARCHIVES
        .iter()
        .map(|(name, bytes)| {
            let path = dir.join(format!("{stem}.{name}"));
            std::fs::write(&path, bytes).map_err(|e| format!("write {}: {e}", path.display()))?;
            Ok(path)
        })
        .collect()
}

/// Link a guest object with the runtime static library and `-lgc`
/// into an executable using the host `cc`.
pub fn link_executable(obj_path: &Path, exe_path: &Path) -> Result<(), String> {
    let archives = write_runtime_archives(obj_path)?;
    let linked = run_cc(obj_path, &archives, exe_path);
    for path in &archives {
        let _ = std::fs::remove_file(path);
    }
    linked
}

fn run_cc(obj_path: &Path, archives: &[PathBuf], exe_path: &Path) -> Result<(), String> {
    let status = std::process::Command::new("cc")
        .arg(obj_path)
        .args(archives)
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

mod contract_types;
mod dependencies;
pub mod infer;
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
    pub modules:
        std::collections::HashMap<String, std::collections::HashMap<String, types::Scheme>>,
    pub contracts:
        std::collections::HashMap<String, std::collections::HashMap<String, types::Scheme>>,
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
        checked
            .named_constructors
            .extend(self.module_named_constructors.clone());
        for (name, (tag, scheme)) in &self.module_constructors {
            checked.constructors.insert(name.clone(), scheme.clone());
            checked.constructor_tags.insert(name.clone(), *tag);
        }
        let names = &self.abstract_representations;
        for scheme in checked
            .definitions
            .values_mut()
            .chain(checked.module_definitions.values_mut())
            .chain(checked.constructors.values_mut())
        {
            *scheme = scheme.reveal(names);
        }
        for (_, scheme) in checked.records.values_mut() {
            *scheme = scheme.reveal(names);
        }
        for ty in checked
            .value_types
            .values_mut()
            .chain(checked.composite_types.values_mut())
        {
            *ty = ty.reveal(names);
        }
        checked.main = checked.main.reveal(names);
        checked
    }

    /// §58.3 / plan.md L2: whether a real type is available for `name`. Q2 の決着に
    /// より、字句・構文・名前解決・型の致命で止まったときは `Checked` の中身が
    /// `Ty::Error`（推論が失敗した印）になる。そのときは型を出さない。
    pub fn has_type(&self, name: Option<&str>) -> bool {
        match name {
            Some(name) => self
                .scheme_of(name)
                .is_some_and(|scheme| !scheme.has_error()),
            None => !self.main.has_error(),
        }
    }

    /// §58.3: the canonical print of the type of the definition named `name`,
    /// as the source spells it. `None` means no such definition.
    pub fn type_of(&self, name: &str) -> Option<String> {
        // §54: `main`'s effects are closed by its body and kept beside its type
        // rather than in its scheme, so the named print uses the same pair as
        // `--print-type` and the two ways to ask agree.
        if name == "main" && self.definitions.contains_key("main") {
            return Some(format!("{}{}", self.main, self.effects.suffix()));
        }
        self.scheme_of(name).map(types::Scheme::canonical)
    }

    /// The scheme behind a name: the entry file's own outer `def`s first (§50),
    /// then the implementations' members (`lib::Util.helper`, `List.map`), then
    /// the members a contract declares under its own name (`Store.find`).
    /// `has_type` and `type_of` share this lookup so that a name either has a
    /// type or is reported missing, never both.
    fn scheme_of(&self, name: &str) -> Option<&types::Scheme> {
        if let Some(scheme) = self.definitions.get(&crate::file_scopes::entry_name(name)) {
            return Some(scheme);
        }
        // §47, §51: a member `A.name` has the type its contract declares, which
        // is the one the source can use at that name. An implementation's own
        // type may be more polymorphic (E602 requires it to be), and its extra
        // definitions are not visible outside at all (E204). A contract's own
        // name reaches the declaration whether or not it has an implementation
        // (§58.5: hover on a `.hdi`).
        let (module, member) = name.rsplit_once('.')?;
        self.modules
            .get(module)
            .and_then(|members| members.get(member))
            .or_else(|| self.contracts.get(module)?.get(member))
    }
}

/// Whether the program passed to `Project::load` (or the single-file API) has an
/// entry `main`, or is checked as a library (A1, §10, §50, §52).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Inspection {
    Entry,
    Library,
}

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
    resolve_namespaces(
        &mut parsed,
        &[(0, Vec::new())],
        &std::collections::BTreeSet::from([Vec::new()]),
        &std::collections::BTreeSet::new(),
        path,
    );
    Ok(parsed)
}

fn resolve_namespaces(
    parsed: &mut ParsedSource,
    files: &[(usize, Vec<String>)],
    directories: &std::collections::BTreeSet<Vec<String>>,
    libraries: &std::collections::BTreeSet<Vec<String>>,
    path: &str,
) {
    let resolved = namespaces::resolve(
        &mut parsed.ast,
        files,
        directories,
        libraries,
        parsed.prelude_items,
        path,
    );
    parsed.namespace_diagnostics = resolved.diagnostics;
    parsed.namespace_anchors = resolved.error_anchors;
}

fn with_prelude(
    mut ast: parser::Ast,
    path: &str,
    source_lines: usize,
) -> Result<ParsedSource, parser::Diagnostic> {
    let inspection = if has_main_definition(&ast) {
        Inspection::Entry
    } else {
        Inspection::Library
    };
    let mut prelude = Vec::new();
    for (name, source) in [
        ("stdlib/list.hd", include_str!("../../../stdlib/list.hd")),
        (
            "stdlib/option.hd",
            include_str!("../../../stdlib/option.hd"),
        ),
        ("stdlib/error.hd", include_str!("../../../stdlib/error.hd")),
    ] {
        prelude.extend(
            parser::parse_source(source, name)?
                .items
                .into_iter()
                .filter(|item| matches!(item, parser::Item::TypeDef(_))),
        );
    }
    let mut trusted_start = None;
    if !std::path::Path::new(path)
        .components()
        .any(|part| part.as_os_str() == "stdlib")
    {
        let mut line = source_lines + 2;
        trusted_start = Some(line + 1);
        for module in [
            include_str!("../../../stdlib/error.hd"),
            include_str!("../../../stdlib/env.hd"),
            include_str!("../../../stdlib/io.hd"),
            include_str!("../../../stdlib/list.hd"),
            include_str!("../../../stdlib/option.hd"),
            include_str!("../../../stdlib/array.hd"),
            include_str!("../../../stdlib/string.hd"),
            include_str!("../../../stdlib/bytes.hd"),
            include_str!("../../../stdlib/int.hd"),
            include_str!("../../../stdlib/float.hd"),
        ] {
            let padded = "\n".repeat(line) + module;
            let parsed = parser::parse_source(&padded, path)?;
            ast.branch_anchors.extend(parsed.branch_anchors);
            ast.branch_spans.extend(parsed.branch_spans);
            ast.equality_anchors.extend(parsed.equality_anchors);
            ast.qualified_type_anchors
                .extend(parsed.qualified_type_anchors);
            ast.name_sites.extend(parsed.name_sites);
            ast.written_list_patterns
                .extend(parsed.written_list_patterns);
            ast.long_condition_functions
                .extend(parsed.long_condition_functions);
            ast.branches_after_fallback
                .extend(parsed.branches_after_fallback);
            prelude.extend(
                parsed
                    .items
                    .into_iter()
                    .filter(|item| !matches!(item, parser::Item::TypeDef(_))),
            );
            line += module.lines().count() + 1;
        }
    }
    let prelude_items = prelude.len()
        + if trusted_start.is_none() {
            ast.items.len()
        } else {
            0
        };
    prelude.append(&mut ast.items);
    ast.items = prelude;
    Ok(ParsedSource {
        ast,
        inspection,
        trusted_start,
        prelude_items,
        other_entry_roots: Vec::new(),
        namespace_diagnostics: Vec::new(),
        namespace_anchors: Vec::new(),
    })
}

pub fn check_source(source: &str, path: &str) -> Result<Checked, parser::Diagnostic> {
    check_parsed(&parse_with_prelude(source, path)?, path)
}

fn check_parsed(parsed: &ParsedSource, path: &str) -> Result<Checked, parser::Diagnostic> {
    check_parsed_filtered(parsed, path, true)
}

pub(crate) fn check_parsed_for_print(
    parsed: &ParsedSource,
    path: &str,
) -> Result<Checked, parser::Diagnostic> {
    check_parsed_filtered(parsed, path, false)
}

/// §56・§58.4（L1）: 最初に当たる段階の診断を**全件**、アンカー順に返す。`checked` は
/// 「型が得られたか」で、L2 により**型推論が途中で失敗したときだけ** `None` になる
/// （E3xx の致命。E602 は推論が最後まで走るので型がある）。CLI の `--show-all` と LSP の
/// diagnostics が同じ列を使う。
pub(crate) fn check_parsed_all(
    parsed: &ParsedSource,
    path: &str,
    reject_user_main_effects: bool,
) -> (Vec<parser::Diagnostic>, Option<Checked>) {
    check_parsed_filtered_all(parsed, path, reject_user_main_effects)
}

/// 診断を 1 件に絞る前の列と、型が得られたかどうか。`Err` は「その段階より前で
/// 解析そのものが止まった」ことを表し、そのときは診断も型も無い。
fn check_parsed_filtered_all(
    parsed: &ParsedSource,
    path: &str,
    reject_user_main_effects: bool,
) -> (Vec<parser::Diagnostic>, Option<Checked>) {
    match check_parsed_stage(parsed, path, reject_user_main_effects) {
        Ok((diagnostics, checked)) => (diagnostics, checked),
        Err(error) => (vec![error], None),
    }
}

/// 1 件に絞る前の段階の結果。`Err` は字句・構文・名前解決など、`Checked` を作る前に
/// 止まった致命。
fn check_parsed_stage(
    parsed: &ParsedSource,
    path: &str,
    reject_user_main_effects: bool,
) -> Result<(Vec<parser::Diagnostic>, Option<Checked>), parser::Diagnostic> {
    let ParsedSource {
        ast,
        inspection,
        trusted_start,
        prelude_items,
        other_entry_roots,
        namespace_diagnostics,
        namespace_anchors,
    } = parsed;
    let adjust = |error| namespace_diagnostic(check::arm_diagnostic(ast, error), namespace_anchors);
    let name_error = names::resolve_with_prelude(ast, path, *trusted_start, *inspection).err();
    let (checked, mut diagnostics) =
        match infer::infer_with_diagnostics(ast, path, *trusted_start, *inspection, *prelude_items)
        {
            Ok(result) => result,
            // §58.3 / L2: 名前解決の致命と型推論の致命では型が無い。E3xx はここへ来る。
            Err(error) => {
                let errors: Vec<_> = name_error
                    .into_iter()
                    .chain(std::iter::once(error))
                    .chain(namespace_diagnostics.iter().cloned())
                    .map(adjust)
                    .collect();
                // `errors` は必ず 1 件以上ある（`infer` の `Err` を積んでいる）。
                return Ok((first_stage(errors).unwrap_or_default(), None));
            }
        };
    diagnostics.extend(name_error);
    diagnostics.extend(namespace_diagnostics.iter().cloned());
    // §58.4 / L1: この段階の診断は全件を積む。1 件に絞るのは `first_stage` の仕事で、
    // `--show-all` と既定の差はそこだけで決まる。
    diagnostics.extend(check::check_ast_all(ast, path));
    diagnostics.extend(dependencies::diagnostics(
        ast,
        &checked,
        *prelude_items,
        other_entry_roots,
        *inspection,
        path,
    ));
    // Why not keep E706 for --print-type: r24/r25 print `String {Ask}` on main.
    if !reject_user_main_effects {
        diagnostics.retain(|error| error.code.as_deref() != Some("E706"));
    }
    let errors: Vec<_> = diagnostics.into_iter().map(adjust).collect();
    // 推論は最後まで走ったので型がある。§58.3 / L2: E4xx・E5xx・E6xx では型を返す
    // （Q2 の決着）。診断が空なら `check` は通ったということでもある。
    Ok((first_stage(errors).unwrap_or_default(), Some(checked)))
}

/// §56: 最初に当たる段階の診断だけを、アンカー順（§50）で返す。E1xx と E3xx の致命は
/// 1 件だけ（plan.md L1）。
pub(crate) fn first_stage(mut errors: Vec<parser::Diagnostic>) -> Option<Vec<parser::Diagnostic>> {
    let stage = errors.iter().map(diagnostic_order).map(|o| o.0).min()?;
    errors.retain(|error| diagnostic_order(error).0 == stage);
    errors.sort_by_key(diagnostic_order);
    // 名前解決と推論は同じ欠陥を別々に報告する（未束縛の名前は E204 を 2 回積む）。
    // 同じコードが同じアンカーを指すものは 1 件に畳む。畳まないと `--show-all` が
    // 同じ位置の同じ欠陥を 2 行で出し、「同じ段階の全件」ではなくなる（J2）。
    errors.dedup_by(|later, earlier| {
        later.code == earlier.code && later.line == earlier.line && later.col == earlier.col
    });

    // 字句・構文の致命（E1xx）と型の致命（E3xx）は 1 件だけ残す。どちらも
    // 「そこから先を読めない」種類なので、同じ段階の残りは同じ原因の繰り返しになる。
    if matches!(stage, 1) || errors.iter().all(is_fatal_type) {
        errors.truncate(1);
    }
    Some(errors)
}

/// E3xx: 型推論が途中で止まる致命（L2 で型を返さない段階）。
fn is_fatal_type(error: &parser::Diagnostic) -> bool {
    matches!(
        error.code.as_deref().and_then(|c| c.as_bytes().get(1)),
        Some(b'3')
    )
}

fn check_parsed_filtered(
    parsed: &ParsedSource,
    path: &str,
    reject_user_main_effects: bool,
) -> Result<Checked, parser::Diagnostic> {
    match check_parsed_stage(parsed, path, reject_user_main_effects) {
        Ok((errors, checked)) => match (errors.into_iter().next(), checked) {
            (Some(error), _) => Err(error),
            (None, Some(checked)) => Ok(checked),
            (None, None) => Err(parser::Diagnostic {
                related: None,
                path: path.into(),
                line: 1,
                col: 1,
                end_line: 1,
                end_col: 1,
                code: None,
                message: "no diagnostic".into(),
            }),
        },
        Err(error) => Err(error),
    }
}

fn namespace_diagnostic(
    mut error: parser::Diagnostic,
    anchors: &[(parser::Span, parser::Span)],
) -> parser::Diagnostic {
    if error.code.as_deref() == Some("E204") {
        if let Some((_, target)) = anchors
            .iter()
            .find(|(source, _)| source.line == error.line && source.col == error.col)
        {
            error.line = target.line;
            error.col = target.col;
        }
    }
    error
}

pub(crate) fn diagnostic_order(error: &parser::Diagnostic) -> (u8, usize, usize) {
    let stage = match error
        .code
        .as_deref()
        .and_then(|code| code.as_bytes().get(1))
    {
        Some(b'1') => 1,
        Some(b'2') => 2,
        Some(b'3' | b'6') => 3,
        Some(b'4') => 4,
        Some(b'5') => 5,
        _ => 6,
    };
    (stage, error.line, error.col)
}

mod ambiguous_declarations;
mod type_cycles;
mod type_realizations;

pub mod names;

pub mod check;

mod normalize;

mod coverage;

pub mod mono;

pub fn dump_mono(source: &str, path: &str) -> Result<String, parser::Diagnostic> {
    dump_parsed(&parse_with_prelude(source, path)?, path)
}

pub(crate) fn entry_required(path: &str) -> parser::Diagnostic {
    parser::Diagnostic {
        related: None,
        path: path.into(),
        line: 1,
        col: 1,
        end_line: 1,
        end_col: 1,
        code: Some("E206".into()),
        message: "build requires an entry file with main; check treats this directory as a library"
            .into(),
    }
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
        related: None,
        path: path.into(),
        line: ast.span.line,
        col: ast.span.col,
        end_line: ast.span.line,
        end_col: ast.span.col,
        code: None,
        message: "main must be a value definition for lowering".into(),
    })?;
    let program = mono::lower_program(ast, &checked).map_err(|message| parser::Diagnostic {
        related: None,
        path: path.into(),
        line: main.span.line,
        col: main.span.col,
        end_line: main.span.line,
        end_col: main.span.col,
        code: None,
        message,
    })?;
    let mut output = format!("fn main() -> {}\n{:?}\n", checked.main, program.entry);
    let printer_id = mono::FunctionId(
        program
            .functions
            .iter()
            .map(|function| function.id.0)
            .max()
            .map_or(1, |id| id + 2),
    );
    let printer = mono::derive_printers(printer_id, program.entry.ty(), &checked);
    for function in program.functions {
        output.push_str(&format!("{function:?}\n"));
    }
    match printer {
        Ok(functions) => {
            for function in functions {
                output.push_str(&format!("{function:?}\n"));
            }
        }
        Err(message) => output.push_str(&format!("; main printer unavailable: {message}\n")),
    }
    Ok(output)
}

pub mod runtime_abi;

pub mod repr;
