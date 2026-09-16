use std::path::{Path, PathBuf};
use std::collections::BTreeSet;
use crate::lexer::{Tok, TokKind};
use crate::parser::{Ast, Diagnostic, Span};

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
        let library_error = library_main.map(|span| Diagnostic {
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
        crate::build::executable_with(output, |object| self.compile_to_object(object))
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
        let kind = entry.file_type().map_err(|error| io_error(&path, error))?;
        if kind.is_dir() {
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
    Diagnostic { path: path.to_string_lossy().into_owned(), line: 1, col: 1,
        code: None, message: error.to_string() }
}

fn read_files(entry: &Path) -> Result<(Vec<TokenFile>, BTreeSet<Vec<String>>), LoadError> {
    let root = root_directory(entry);
    let mut paths = Vec::new();
    let mut directories = BTreeSet::from([Vec::new()]);
    collect_sources(root, root, &mut paths, &mut directories)?;
    let selected = relative_source(entry, root);
    paths.retain(|path| relative_source(path, root) != selected);
    paths.sort_by(|a, b| relative_source(a, root).as_os_str().as_encoded_bytes()
        .cmp(relative_source(b, root).as_os_str().as_encoded_bytes()));
    paths.insert(0, entry.to_path_buf());
    let mut files = Vec::new();
    let mut offset = 0;
    for path in paths {
        let file = read_file(&path, root, offset)?;
        offset = file.tokens.last().unwrap().span.line + 1;
        files.push(file);
    }
    Ok((files, directories))
}

fn read_file(path: &Path, root: &Path, offset: usize) -> Result<TokenFile, Diagnostic> {
    let relative = relative_source(path, root);
    let display = relative.to_string_lossy().into_owned();
    let text = std::fs::read_to_string(path).map_err(|error| io_error(&relative, error))?;
    let mut tokens = crate::lexer::lex_full(&text).map_err(|error| Diagnostic {
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
            return Some(Diagnostic { path: path.to_string_lossy().into_owned(), line: token.span.line,
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
    let mut duplicate: Option<Diagnostic> = None;
    for file in files {
        let mut declared = false;
        for token in &file.tokens {
            if token.kind != TokKind::KwLibrary { continue; }
            if declared || roots.contains(&file.source.directory) {
                let error = Diagnostic { path: file.source.path.clone(), line: token.span.line,
                    col: token.span.col, code: Some("E205".into()),
                    message: "duplicate library declaration".into() };
                if duplicate.as_ref().map_or(true, |current| crate::diagnostic_order(&error) < crate::diagnostic_order(current)) {
                    duplicate = Some(error);
                }
                continue;
            }
            declared = true;
            roots.insert(file.source.directory.clone());
        }
    }
    (roots, duplicate)
}

fn source_diagnostic(files: &[SourceFile], mut error: Diagnostic) -> Diagnostic {
    if let Some(file) = files.iter().rev().find(|file| file.offset < error.line) {
        error.path = file.path.clone();
        error.line -= file.offset;
    }
    for index in 0..files.len() {
        error.message = error.message.replace(&format!("file{index}$"), "");
    }
    error
}
