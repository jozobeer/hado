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
        let ast = parse_files(&token_files)?;
        let last_line = token_files.last().unwrap().tokens.last().unwrap().span.line;
        let parsed = crate::with_prelude(ast, &files[0].path, last_line)?;
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
    for sibling in std::fs::read_dir(parent).map_err(|error| io_error(parent, error))? {
        let path = sibling.map_err(|error| io_error(parent, error))?.path();
        if path.file_name() != entry.file_name() && path.is_file()
            && matches!(path.extension().and_then(|extension| extension.to_str()), Some("hd" | "hdi")) {
            siblings.push(path);
        }
    }
    siblings.sort_by(|a, b| a.as_os_str().as_encoded_bytes().cmp(b.as_os_str().as_encoded_bytes()));
    let mut paths = vec![entry.to_path_buf()];
    paths.extend(siblings);
    Ok(paths)
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

fn parse_files(files: &[TokenFile]) -> Result<Ast, Diagnostic> {
    let mut ast = parse_file(&files[0].tokens, Path::new(&files[0].source.path), true)?;
    for file in &files[1..] {
        let parsed = parse_file(&file.tokens, Path::new(&file.source.path), false)
            .map_err(|mut error| { error.line -= file.source.offset; error })?;
        ast.append(parsed);
    }
    Ok(ast)
}

fn parse_file(tokens: &[Tok], path: &Path, entry: bool) -> Result<Ast, Diagnostic> {
    let role_error = role_error(tokens, path, entry);
    let parsed = crate::parser::parse_tokens(tokens, &path.to_string_lossy());
    match (parsed, role_error) {
        (Ok(ast), None) => Ok(ast),
        (Ok(_), Some(error)) | (Err(error), None) => Err(error),
        (Err(syntax), Some(role)) => Err([syntax, role].into_iter().min_by_key(crate::diagnostic_order).unwrap()),
    }
}

fn role_error(tokens: &[Tok], path: &Path, entry: bool) -> Option<Diagnostic> {
    let interface = path.extension().is_some_and(|extension| extension == "hdi");
    let mut outer = true;
    for token in tokens {
        let invalid = (interface && token.kind == TokKind::KwImplement)
            || (!entry && outer && token.kind == TokKind::Def);
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
    error
}
