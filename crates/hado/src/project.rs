use std::path::{Path, PathBuf};
use std::collections::{BTreeMap, BTreeSet};
use crate::lexer::{Tok, TokKind};
use crate::parser::{Ast, Diagnostic, Related, Span};

#[derive(Clone)]
struct SourceFile {
    physical: PathBuf,
    path: String,
    directory: Vec<String>,
    offset: usize,
}

struct TokenFile {
    source: SourceFile,
    tokens: Vec<Tok>,
}

pub struct Project {
    parsed: crate::ParsedSource,
    files: Vec<SourceFile>,
}

impl Project {
    pub fn load(entry: &Path) -> Result<Self, LoadError> {
        let (token_files, directories) = read_files(entry)?;
        let files: Vec<_> = token_files.iter().map(|file| file.source.clone()).collect();
        let (libraries, duplicate_library) = library_roots(&token_files);
        let (ast, other_entry_roots, library_main) = parse_files(&token_files, &libraries)?;
        let library_error = library_main.map(|span| Diagnostic { related: None,
            path: files[0].path.clone(), line: span.line, col: span.col,
            code: Some("E206".into()),
            message: "a library directory cannot declare main".into(),
        });
        // §56: E205 と E206 は同じ名前解決の段階なので、アンカー位置のソース順で選ぶ。
        if let Some(error) = [duplicate_library, library_error].into_iter().flatten()
            .min_by_key(crate::diagnostic_order) {
            return Err(LoadError::Diagnostic(source_diagnostic(&files, error)));
        }
        let last_line = token_files.last().unwrap().tokens.last().unwrap().span.line;
        let mut parsed = crate::with_prelude(ast, &files[0].physical.to_string_lossy(), last_line)?;
        // §52: another file's `main` is an entry only in an entry inspection.
        parsed.other_entry_roots = if parsed.inspection == crate::Inspection::Entry { other_entry_roots } else { Vec::new() };
        let namespaces: Vec<_> = files.iter().map(|file| (file.offset, file.directory.clone())).collect();
        crate::resolve_namespaces(&mut parsed, &namespaces, &directories, &libraries, &files[0].path);
        Ok(Self { parsed, files })
    }

    pub fn is_library(&self) -> bool {
        self.parsed.inspection == crate::Inspection::Library
    }

    pub fn check(&self) -> Result<crate::Checked, Diagnostic> {
        crate::check_parsed(&self.parsed, &self.files[0].path)
            .map_err(|error| source_diagnostic(&self.files, error))
    }

    pub fn check_for_print(&self) -> Result<crate::Checked, Diagnostic> {
        crate::check_parsed_for_print(&self.parsed, &self.files[0].path)
            .map_err(|error| source_diagnostic(&self.files, error))
    }

    pub fn dump_mono(&self) -> Result<String, Diagnostic> {
        crate::dump_parsed(&self.parsed, &self.files[0].path)
            .map_err(|error| source_diagnostic(&self.files, error))
    }

    /// The diagnostic as the CLI prints it: the canonical form is appended
    /// when a local rewrite (§57) starts at the diagnostic's anchor.
    pub fn report(&self, error: &Diagnostic) -> String {
        match self.canonical_hint(error) {
            Some(hint) => format!("{error}; canonical form: {hint}"),
            None => error.to_string(),
        }
    }

    /// §58.4: the diagnostic as one line of JSON, for tooling. The canonical
    /// form is a field of its own instead of being appended to the message.
    pub fn report_json(&self, error: &Diagnostic) -> String {
        let related = match &error.related {
            Some(related) => format!(
                "{{\"path\": {}, \"line\": {}, \"col\": {}, \"label\": {}}}",
                json_string(&related.path),
                related.line,
                related.col,
                option_string(error.code.as_deref().and_then(crate::parser::related_label)),
            ),
            None => "null".to_string(),
        };
        format!(
            "{{\"path\": {}, \"line\": {}, \"col\": {}, \"code\": {}, \"message\": {}, \"canonical\": {}, \"related\": {}}}",
            json_string(&error.path),
            error.line,
            error.col,
            option_string(error.code.as_deref()),
            json_string(&error.message),
            option_string(self.canonical_hint(error).as_deref()),
            related,
        )
    }

    /// The canonical form to show next to a diagnostic, when one of the local
    /// rewrites (§57) starts exactly at the diagnostic's anchor. The file is
    /// read only when a diagnostic is already reported.
    pub fn canonical_hint(&self, error: &Diagnostic) -> Option<String> {
        let code = error.code.as_deref()?;
        if !crate::fixes::is_fixable(code) {
            return None;
        }
        let source = std::fs::read_to_string(self.source_file(&error.path)?).ok()?;
        crate::fixes::hint(&source, code, error.line, error.col)
    }

    /// Apply the canonical rewrite for one diagnostic to its file. Returns
    /// `true` when the file changed. The rewrite is matched by code and anchor,
    /// so a diagnostic without a local rewrite leaves the file untouched.
    pub fn apply_canonical_fix(&self, error: &Diagnostic) -> Result<bool, String> {
        let Some(code) = error.code.as_deref() else { return Ok(false) };
        if !crate::fixes::is_fixable(code) {
            return Ok(false);
        }
        let Some(file) = self.source_file(&error.path) else { return Ok(false) };
        let source = std::fs::read_to_string(file).map_err(|error| error.to_string())?;
        let Some(fix) = crate::fixes::fix_at(&source, code, error.line, error.col) else {
            return Ok(false);
        };
        let mut updated = String::with_capacity(source.len() + fix.replacement.len());
        updated.push_str(&source[..fix.start]);
        updated.push_str(&fix.replacement);
        updated.push_str(&source[fix.end..]);
        std::fs::write(file, updated).map_err(|error| error.to_string())?;
        Ok(true)
    }

    fn source_file(&self, path: &str) -> Option<&std::path::Path> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .or_else(|| self.files.first())
            .map(|file| file.physical.as_path())
    }

    pub fn compile_to_object(&self, output: &Path) -> Result<(), String> {
        if self.is_library() {
            return Err(crate::entry_required(&self.files[0].path).to_string());
        }
        let checked = self.check().map_err(|error| error.to_string())?;
        let program = crate::mono::lower_program(&self.parsed.ast, &checked)?;
        crate::emit::compile_mono_to_object(&program, &checked, output)
    }

    pub fn build(&self, output: &Path) -> Result<(), String> {
        if self.is_library() {
            return Err(self.report(&crate::entry_required(&self.files[0].path)));
        }
        // Diagnostics go through `report` so `build` and `run` show the same
        // `canonical form:` hint as `check` (§57, §58).
        let checked = self.check().map_err(|error| self.report(&error))?;
        crate::build::executable_with(output, |object| {
            let program = crate::mono::lower_program(&self.parsed.ast, &checked)?;
            crate::emit::compile_mono_to_object(&program, &checked, object)
        })
    }
}

#[derive(Debug)]
pub enum LoadError {
    Input(String),
    Diagnostic(Diagnostic),
}

impl From<Diagnostic> for LoadError {
    fn from(error: Diagnostic) -> Self { Self::Diagnostic(error) }
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(message) => formatter.write_str(message),
            Self::Diagnostic(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

fn root_directory(entry: &Path) -> &Path {
    entry.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."))
}

fn relative_source(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).map(Path::to_path_buf)
        .unwrap_or_else(|_| PathBuf::from(path.file_name().unwrap()))
}

fn namespace_parts(relative: &Path) -> Vec<String> {
    relative.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect()
}

fn valid_directory_name(name: &str) -> bool {
    if !name.starts_with(char::is_lowercase) { return false; }
    let Ok(tokens) = crate::lexer::lex_full(name) else { return false; };
    matches!(tokens.as_slice(), [Tok { kind: TokKind::Name(value), .. }, Tok { kind: TokKind::Eof, .. }] if value == name)
}

fn collect_sources(directory: &Path, root: &Path, paths: &mut Vec<PathBuf>, directories: &mut BTreeSet<Vec<String>>) -> Result<(), LoadError> {
    let mut entries = std::fs::read_dir(directory).map_err(|error| io_error(directory, error))?
        .collect::<Result<Vec<_>, _>>().map_err(|error| io_error(directory, error))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        // §50: `.` で始まる名前は読まない。名前空間の字句（§51）になり得ない
        // ので、拒否するのではなく黙って外す。git の倉庫の根でも検査でき、
        // 試し書きの隔離先になる。`__` 始まりのディレクトリも字句になり得ない
        // ので同じ（`__pycache__`）。ファイル名は自由（§50）なので、
        // `__` 始まりのファイルは読む。
        let name_bytes = entry.file_name();
        let name_bytes = name_bytes.as_encoded_bytes();
        if name_bytes.starts_with(b".") {
            continue;
        }
        let kind = entry.file_type().map_err(|error| io_error(&path, error))?;
        if kind.is_dir() {
            if name_bytes.starts_with(b"__") {
                continue;
            }
            let name = entry.file_name();
            if !name.to_str().is_some_and(valid_directory_name) {
                return Err(LoadError::Input(format!("invalid namespace directory: {}", relative_source(&path, root).display())));
            }
            directories.insert(namespace_parts(&relative_source(&path, root)));
            collect_sources(&path, root, paths, directories)?;
        } else if path.is_file()
            && matches!(path.extension().and_then(|extension| extension.to_str()), Some("hd" | "hdi")) {
            paths.push(path);
        }
    }
    Ok(())
}

fn io_error(path: &Path, error: std::io::Error) -> Diagnostic {
    Diagnostic { related: None, path: path.to_string_lossy().into_owned(), line: 1, col: 1,
        code: None, message: error.to_string() }
}

/// §50: the sources of the project whose entry is `entry`, paired with the
/// path diagnostics print for them. The entry comes first, then the other
/// files in byte order of their relative paths. Reading is left to the caller,
/// so a file that does not lex cannot hide the rest from `hado fmt` (§58.1).
pub fn source_paths(entry: &Path) -> Result<Vec<(String, PathBuf)>, LoadError> {
    let (root, paths, _) = collect_paths(entry)?;
    Ok(paths
        .into_iter()
        .map(|path| (relative_source(&path, &root).to_string_lossy().into_owned(), path))
        .collect())
}

/// §58.2: the tests of the project: the root directory's `*_test.hd`, in the
/// order the checker reads them. A test needs `main`, and only a root file can
/// hold one (§50, §52), so the subdirectories contribute none.
pub fn test_paths(entry: &Path) -> Result<Vec<(String, PathBuf)>, LoadError> {
    let (root, paths, _) = collect_paths(entry)?;
    Ok(paths
        .into_iter()
        .filter(|path| path.parent() == Some(root.as_path()))
        .filter(|path| {
            path.file_name().is_some_and(|name| name.as_encoded_bytes().ends_with(b"_test.hd"))
        })
        .map(|path| (relative_source(&path, &root).to_string_lossy().into_owned(), path))
        .collect())
}

/// The paths of the project in reading order, with the root and the namespace
/// directories `read_files` needs.
fn collect_paths(
    entry: &Path,
) -> Result<(PathBuf, Vec<PathBuf>, BTreeSet<Vec<String>>), LoadError> {
    let root = root_directory(entry).to_path_buf();
    let mut paths = Vec::new();
    let mut directories = BTreeSet::from([Vec::new()]);
    collect_sources(&root, &root, &mut paths, &mut directories)?;
    let selected = relative_source(entry, &root);
    paths.retain(|path| relative_source(path, &root) != selected);
    paths.sort_by(|a, b| relative_source(a, &root).as_os_str().as_encoded_bytes()
        .cmp(relative_source(b, &root).as_os_str().as_encoded_bytes()));
    paths.insert(0, entry.to_path_buf());
    Ok((root, paths, directories))
}

fn read_files(entry: &Path) -> Result<(Vec<TokenFile>, BTreeSet<Vec<String>>), LoadError> {
    let (root, paths, directories) = collect_paths(entry)?;
    let mut files = Vec::new();
    let mut offset = 0;
    for path in paths {
        let file = read_file(&path, &root, offset)?;
        offset = file.tokens.last().unwrap().span.line + 1;
        files.push(file);
    }
    Ok((files, directories))
}

fn read_file(path: &Path, root: &Path, offset: usize) -> Result<TokenFile, Diagnostic> {
    let relative = relative_source(path, root);
    let display = relative.to_string_lossy().into_owned();
    let text = std::fs::read_to_string(path).map_err(|error| io_error(&relative, error))?;
    let mut tokens = crate::lexer::lex_full(&text).map_err(|error| Diagnostic { related: None,
        path: display.clone(), line: error.line, col: error.col,
        code: error.code.map(str::to_owned), message: error.message,
    })?;
    for token in &mut tokens { token.span.line += offset; }
    let directory = namespace_parts(relative.parent().unwrap_or(Path::new("")));
    Ok(TokenFile { source: SourceFile { physical: path.to_path_buf(), path: display, directory, offset }, tokens })
}

fn parse_files(files: &[TokenFile], libraries: &BTreeSet<Vec<String>>) -> Result<(Ast, Vec<String>, Option<Span>), Diagnostic> {
    let mut ast = parse_file(&files[0], libraries)?;
    let mut library_main = library_main_span(&ast, &files[0].source.directory, libraries);
    let mut roots = crate::file_scopes::isolate(&mut ast, 0);
    for (index, file) in files.iter().enumerate().skip(1) {
        let mut parsed = parse_file(file, libraries)
            .map_err(|mut error| { error.line -= file.source.offset; error })?;
        if library_main.is_none() {
            library_main = library_main_span(&parsed, &file.source.directory, libraries);
        }
        roots.extend(crate::file_scopes::isolate(&mut parsed, index));
        ast.append(parsed);
    }
    Ok((ast, roots, library_main))
}

fn parse_file(file: &TokenFile, libraries: &BTreeSet<Vec<String>>) -> Result<Ast, Diagnostic> {
    let path = Path::new(&file.source.path);
    // §52: an outer `def` is allowed in the root directory and in a library
    // root directory (A3), not in a plain subdirectory (§50).
    let root_file = file.source.directory.is_empty() || libraries.contains(&file.source.directory);
    let role_error = role_error(&file.tokens, path, root_file);
    let parsed = crate::parser::parse_tokens(&file.tokens, &file.source.path);
    match (parsed, role_error) {
        (Ok(ast), None) => Ok(ast),
        (Ok(_), Some(error)) | (Err(error), None) => Err(error),
        (Err(syntax), Some(role)) => Err([syntax, role].into_iter().min_by_key(crate::diagnostic_order).unwrap()),
    }
}

fn role_error(tokens: &[Tok], path: &Path, root_file: bool) -> Option<Diagnostic> {
    let interface = path.extension().is_some_and(|extension| extension == "hdi");
    let mut outer = true;
    for token in tokens {
        let invalid = (interface && matches!(token.kind, TokKind::KwImplement | TokKind::KwLibrary))
            || ((interface || !root_file) && outer && token.kind == TokKind::Def);
        if invalid {
            return Some(Diagnostic { related: None, path: path.to_string_lossy().into_owned(), line: token.span.line,
                col: token.span.col, code: Some("E100".into()),
                message: "declaration is not allowed in this file".into() });
        }
        if matches!(token.kind, TokKind::KwModule | TokKind::KwImplement | TokKind::KwEffect | TokKind::KwHandler) { outer = false; }
    }
    None
}

/// A `main` in a directory that declares `library` is E206 (§52), whichever
/// file of the directory it sits in. The prohibition does not depend on the
/// shape of the definition, so any `def main` counts.
fn library_main_span(ast: &Ast, directory: &[String], libraries: &BTreeSet<Vec<String>>) -> Option<Span> {
    libraries.contains(directory).then(|| crate::main_definition_span(ast)).flatten()
}

/// Directories that declare `library`, in file order (entry first, then relative path bytes).
/// The second declaration of a directory, and the second in a file, are E205 (§52);
/// the diagnostic is returned rather than raised so the caller can order it with E206.
fn library_roots(files: &[TokenFile]) -> (BTreeSet<Vec<String>>, Option<Diagnostic>) {
    let mut roots = BTreeSet::new();
    // §56: the first `library` of a directory is the related position.
    let mut firsts: BTreeMap<Vec<String>, (String, Span)> = BTreeMap::new();
    let mut duplicate: Option<Diagnostic> = None;
    for file in files {
        let mut declared = false;
        for token in &file.tokens {
            if token.kind != TokKind::KwLibrary { continue; }
            if declared || roots.contains(&file.source.directory) {
                let related = firsts.get(&file.source.directory)
                    .map(|(path, first)| Related { path: path.clone(), line: first.line, col: first.col });
                let error = Diagnostic { related, path: file.source.path.clone(), line: token.span.line,
                    col: token.span.col, code: Some("E205".into()),
                    message: "duplicate library declaration".into() };
                if duplicate.as_ref().map_or(true, |current| crate::diagnostic_order(&error) < crate::diagnostic_order(current)) {
                    duplicate = Some(error);
                }
                continue;
            }
            declared = true;
            roots.insert(file.source.directory.clone());
            firsts.entry(file.source.directory.clone())
                .or_insert_with(|| (file.source.path.clone(), token.span));
        }
    }
    (roots, duplicate)
}

/// §56: a position is written with the path of the file that holds it and with
/// the line counted inside that file. Every file was lexed with the previous
/// files' line counts added, so the checked line picks the file (§50).
fn localize(files: &[SourceFile], path: &mut String, line: &mut usize) {
    if let Some(file) = files.iter().rev().find(|file| file.offset < *line) {
        *path = file.path.clone();
        *line -= file.offset;
    }
}

fn source_diagnostic(files: &[SourceFile], mut error: Diagnostic) -> Diagnostic {
    if let Some(related) = &mut error.related {
        localize(files, &mut related.path, &mut related.line);
    }
    localize(files, &mut error.path, &mut error.line);
    for index in 0..files.len() {
        error.message = error.message.replace(&format!("file{index}$"), "");
    }
    error
}

/// A JSON string literal, or `null` when there is nothing to write (§58.4).
fn option_string(value: Option<&str>) -> String {
    match value {
        Some(text) => json_string(text),
        None => "null".to_string(),
    }
}

/// A JSON string literal with the escapes §58.4 needs.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push('"');
    out
}
