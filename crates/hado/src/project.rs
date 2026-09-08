use std::path::{Path, PathBuf};
use crate::lexer::{Tok, TokKind};
use crate::parser::{Ast, Diagnostic};

#[derive(Clone)]
struct SourceFile {
    path: String,
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
    pub fn load(entry: &Path) -> Result<Self, Diagnostic> {
        let token_files = read_files(entry)?;
        let files: Vec<_> = token_files.iter().map(|file| file.source.clone()).collect();
        let (ast, other_entry_roots) = parse_files(&token_files)?;
        let last_line = token_files.last().unwrap().tokens.last().unwrap().span.line;
        let mut parsed = crate::with_prelude(ast, &files[0].path, last_line)?;
        parsed.other_entry_roots = other_entry_roots;
        Ok(Self { parsed, files })
    }

    pub fn check(&self) -> Result<crate::Checked, Diagnostic> {
        crate::check_parsed(&self.parsed, &self.files[0].path)
            .map_err(|error| source_diagnostic(&self.files, error))
    }

    pub fn dump_mono(&self) -> Result<String, Diagnostic> {
        crate::dump_parsed(&self.parsed, &self.files[0].path)
            .map_err(|error| source_diagnostic(&self.files, error))
    }

    pub fn compile_to_object(&self, output: &Path) -> Result<(), String> {
        let checked = self.check().map_err(|error| error.to_string())?;
        let program = crate::mono::lower_program(&self.parsed.ast, &checked)?;
        crate::emit::compile_mono_to_object(&program, &checked, output)
    }

    pub fn build(&self, output: &Path) -> Result<(), String> {
        crate::build::executable_with(output, |object| self.compile_to_object(object))
    }
}

fn sibling_paths(entry: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let parent = entry.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut siblings = Vec::new();
    collect_sources(parent, &mut siblings)?;
    siblings.retain(|path| !(path.parent() == Some(parent) && path.file_name() == entry.file_name()));
    siblings.sort_by(|a, b| a.as_os_str().as_encoded_bytes().cmp(b.as_os_str().as_encoded_bytes()));
    let mut paths = vec![entry.to_path_buf()];
    paths.extend(siblings);
    Ok(paths)
}

fn collect_sources(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), Diagnostic> {
    for entry in std::fs::read_dir(directory).map_err(|error| io_error(directory, error))? {
        let entry = entry.map_err(|error| io_error(directory, error))?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|error| io_error(&path, error))?;
        if kind.is_dir() {
            collect_sources(&path, paths)?;
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

fn read_files(entry: &Path) -> Result<Vec<TokenFile>, Diagnostic> {
    let mut files = Vec::new();
    let mut offset = 0;
    for path in sibling_paths(entry)? {
        let file = read_file(&path, offset)?;
        offset = file.tokens.last().unwrap().span.line + 1;
        files.push(file);
    }
    Ok(files)
}

fn read_file(path: &Path, offset: usize) -> Result<TokenFile, Diagnostic> {
    let text = std::fs::read_to_string(path).map_err(|error| io_error(path, error))?;
    let mut tokens = crate::lexer::lex_full(&text).map_err(|error| Diagnostic {
        path: path.to_string_lossy().into_owned(), line: error.line, col: error.col,
        code: error.code.map(str::to_owned), message: error.message,
    })?;
    for token in &mut tokens { token.span.line += offset; }
    Ok(TokenFile { source: SourceFile { path: path.to_string_lossy().into_owned(), offset }, tokens })
}

fn parse_files(files: &[TokenFile]) -> Result<(Ast, Vec<String>), Diagnostic> {
    let root = Path::new(&files[0].source.path).parent().unwrap_or(Path::new(""));
    let mut ast = parse_file(&files[0].tokens, Path::new(&files[0].source.path), true)?;
    let mut roots = crate::file_scopes::isolate(&mut ast, 0);
    for (index, file) in files.iter().enumerate().skip(1) {
        let path = Path::new(&file.source.path);
        let parent = path.parent().unwrap_or(Path::new(""));
        let root_file = parent == root || (root.as_os_str().is_empty() && parent == Path::new("."));
        let mut parsed = parse_file(&file.tokens, path, root_file)
            .map_err(|mut error| { error.line -= file.source.offset; error })?;
        roots.extend(crate::file_scopes::isolate(&mut parsed, index));
        ast.append(parsed);
    }
    Ok((ast, roots))
}

fn parse_file(tokens: &[Tok], path: &Path, root_file: bool) -> Result<Ast, Diagnostic> {
    let role_error = role_error(tokens, path, root_file);
    let parsed = crate::parser::parse_tokens(tokens, &path.to_string_lossy());
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
        let invalid = (interface && token.kind == TokKind::KwImplement)
            || ((interface || !root_file) && outer && token.kind == TokKind::Def);
        if invalid {
            return Some(Diagnostic { path: path.to_string_lossy().into_owned(), line: token.span.line,
                col: token.span.col, code: Some("E100".into()),
                message: "declaration is not allowed in this file".into() });
        }
        if matches!(token.kind, TokKind::KwModule | TokKind::KwImplement) { outer = false; }
    }
    None
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
