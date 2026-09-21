//! §58: `hado lsp` speaks the Language Server Protocol over stdio.
//!
//! JSON-RPC bodies are framed with a `Content-Length` header and serialized by
//! `serde_json`; only protocol messages reach stdout. The editor holds text it
//! has not saved, so every analysis goes through `Project::load_with_overlay`
//! and a document's bytes never come back from disk. Positions leave hado as
//! 1-based Unicode scalars (§50) and are converted to the encoding the client
//! negotiated here.

use crate::parser::Diagnostic;
use crate::project::{self, Project};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// The unit a client counts characters in. LSP lets the client list its
/// preferences and the server advertise the one it will use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionEncoding {
    Utf8,
    #[default]
    Utf16,
    Utf32,
}

impl PositionEncoding {
    /// The name as `positionEncoding` spells it: one string, never an array.
    pub fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Utf16 => "utf-16",
            Self::Utf32 => "utf-32",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "utf-8" => Some(Self::Utf8),
            "utf-16" => Some(Self::Utf16),
            "utf-32" => Some(Self::Utf32),
            _ => None,
        }
    }

    /// §58: the first encoding of the client's list this server can speak. LSP
    /// names the preference in `general.positionEncodings`; with no list, or a
    /// list this server cannot use, the protocol default is `utf-16`.
    pub fn negotiate(encodings: Option<&[String]>) -> Self {
        let Some(encodings) = encodings else {
            return Self::default();
        };
        encodings
            .iter()
            .find_map(|name| Self::from_name(name))
            .unwrap_or_default()
    }
}

/// The number of units of `encoding` before the `scalars`-th Unicode scalar of
/// one line. A count past the end of the line stops at its end.
pub fn units_before(line: &str, scalars: usize, encoding: PositionEncoding) -> usize {
    line.chars()
        .take(scalars)
        .map(|character| match encoding {
            PositionEncoding::Utf8 => character.len_utf8(),
            PositionEncoding::Utf16 => character.len_utf16(),
            PositionEncoding::Utf32 => 1,
        })
        .sum()
}

/// The byte offset of the `scalars`-th scalar of `line`.
pub fn scalar_to_byte(line: &str, scalars: usize) -> usize {
    units_before(line, scalars, PositionEncoding::Utf8)
}

/// The UTF-16 code unit offset of the `scalars`-th scalar of `line`; a
/// surrogate pair counts twice.
pub fn scalar_to_utf16(line: &str, scalars: usize) -> usize {
    units_before(line, scalars, PositionEncoding::Utf16)
}

/// The 0-based LSP line and character of the 1-based scalar line and column
/// (§50) of `source`.
pub fn encode_position(
    encoding: PositionEncoding,
    source: &str,
    line: usize,
    col: usize,
) -> (usize, usize) {
    let text = nth_line(source, line);
    (
        line.saturating_sub(1),
        units_before(text, col.saturating_sub(1), encoding),
    )
}

/// The 1-based scalar line and column (§50) of a 0-based LSP position. A
/// character that lands inside one scalar rounds down to its start.
pub fn decode_position(
    encoding: PositionEncoding,
    source: &str,
    line: usize,
    character: usize,
) -> (usize, usize) {
    let text = nth_line(source, line + 1);
    let mut units = 0;
    let mut scalars = 0;
    for scalar in text.chars() {
        let width = match encoding {
            PositionEncoding::Utf8 => scalar.len_utf8(),
            PositionEncoding::Utf16 => scalar.len_utf16(),
            PositionEncoding::Utf32 => 1,
        };
        if units + width > character {
            break;
        }
        units += width;
        scalars += 1;
    }
    (line + 1, scalars + 1)
}

fn nth_line(source: &str, line: usize) -> &str {
    source.split('\n').nth(line.saturating_sub(1)).unwrap_or("")
}

/// §58: the `initialize` result. `positionEncoding` is the one string the
/// client and this server agreed on, and the sync kind is Full (1), so a
/// `didChange` carries the whole document.
pub fn initialize_result(encoding: PositionEncoding) -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,
            "hoverProvider": true,
            "codeActionProvider": true,
            "positionEncoding": encoding.name(),
        }
    })
}

/// The `Content-Length` frame around `payload`. The length is the byte count,
/// so a body with multibyte characters travels whole.
pub fn frame_message(payload: &str) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    let mut framed = Vec::with_capacity(header.len() + payload.len());
    framed.extend_from_slice(header.as_bytes());
    framed.extend_from_slice(payload.as_bytes());
    framed
}

/// The next body on `reader`, or `None` at end of input. Only the headers the
/// transport needs are read; anything else before the blank line is skipped.
pub fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(length) = length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing Content-Length",
        ));
    };
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok(Some(String::from_utf8_lossy(&body).into_owned()))
}

/// Write one framed body and flush, so a client waiting on a response is not
/// left with it in the buffer. These are the only bytes written to stdout.
pub fn write_message<W: Write>(writer: &mut W, payload: &str) -> io::Result<()> {
    writer.write_all(&frame_message(payload))?;
    writer.flush()
}

/// The path of a `file:` URI. Neovim sends `file:///abs/path`, so the empty
/// authority is stripped; `%XX` escapes are decoded because a path may hold
/// spaces or non-ASCII bytes.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let path = if rest.starts_with('/') {
        rest
    } else {
        rest.find('/').map_or("", |index| &rest[index..])
    };
    Some(PathBuf::from(percent_decode(path)))
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
            {
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// A path without `.` or `..` components, so the path a URI names and the path
/// a project enumerates are the same even when the user opened the file with a
/// different spelling. The filesystem is not consulted: an unsaved document
/// has no entry there.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// §58: the `initialize` result for the client's preferences, then serve
/// messages until `exit`. The return value is the process exit code: 0 after a
/// `shutdown`, 1 when `exit` arrives without one or stdin closes first.
pub fn serve() -> i32 {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        loop {
            match read_message(&mut reader) {
                Ok(Some(text)) => {
                    let Ok(message) = serde_json::from_str::<Value>(&text) else {
                        continue;
                    };
                    if sender.send(message).is_err() {
                        return;
                    }
                }
                _ => return,
            }
        }
    });
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    let mut server = Server::default();
    loop {
        let Ok(message) = receiver.recv() else {
            return 1;
        };
        if let Action::Exit(code) = server.handle(&message, &mut writer) {
            return code;
        }
        // Let a burst settle before analyzing: `didChange` messages that arrive
        // together collapse into one analysis of the newest text, so a version
        // the next change supersedes is never published. Requests are answered
        // as they arrive, so this only delays the diagnostics.
        let deadline = Instant::now() + SETTLE_LIMIT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match receiver.recv_timeout(SETTLE.min(remaining)) {
                Ok(message) => {
                    if let Action::Exit(code) = server.handle(&message, &mut writer) {
                        return code;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        if server.dirty {
            server.publish(&mut writer);
            server.dirty = false;
        }
    }
}

/// How long to keep taking messages before publishing. Neovim debounces its own
/// `didChange` by 150 ms, so this only groups a burst that is already in flight,
/// and `SETTLE_LIMIT` bounds the wait under continuous input.
const SETTLE: Duration = Duration::from_millis(20);
const SETTLE_LIMIT: Duration = Duration::from_millis(200);

/// What one message asked the loop to do.
enum Action {
    Continue,
    Exit(i32),
}

/// One open document: the version the client last sent, its bytes, and the path
/// those bytes belong to.
#[derive(Default)]
struct Document {
    version: i64,
    text: String,
    path: PathBuf,
}

#[derive(Default)]
struct Server {
    documents: BTreeMap<String, Document>,
    folders: Vec<PathBuf>,
    root: Option<PathBuf>,
    encoding: PositionEncoding,
    /// The version last published per URI, so an older result never replaces a
    /// newer one (§58).
    published: BTreeMap<String, i64>,
    shutdown: bool,
    /// A `didOpen` / `didChange` / `didClose` since the last analysis.
    dirty: bool,
}

impl Server {
    fn handle<W: Write>(&mut self, message: &Value, out: &mut W) -> Action {
        let method = message["method"].as_str().unwrap_or_default();
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => {
                self.initialize(&params);
                respond(out, &id, initialize_result(self.encoding));
            }
            "initialized" => {}
            "shutdown" => {
                self.shutdown = true;
                respond(out, &id, Value::Null);
            }
            "exit" => return Action::Exit(if self.shutdown { 0 } else { 1 }),
            "textDocument/didOpen" => {
                self.did_open(&params);
                self.dirty = true;
            }
            "textDocument/didChange" => {
                self.did_change(&params);
                self.dirty = true;
            }
            "textDocument/didClose" => {
                self.did_close(&params);
                self.dirty = true;
            }
            "textDocument/hover" => {
                let result = self.hover(&params);
                respond(out, &id, result);
            }
            "textDocument/codeAction" => {
                let result = self.code_action(&params);
                respond(out, &id, result);
            }
            _ => {
                if id.is_some() {
                    respond_error(out, &id, -32601, "method not found");
                }
            }
        }
        Action::Continue
    }

    fn initialize(&mut self, params: &Value) {
        let encodings: Option<Vec<String>> = params["capabilities"]["general"]["positionEncodings"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect()
            });
        self.encoding = PositionEncoding::negotiate(encodings.as_deref());
        self.folders = params["workspaceFolders"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|folder| folder["uri"].as_str())
                    .filter_map(uri_to_path)
                    .map(|path| normalize_path(&path))
                    .collect()
            })
            .unwrap_or_default();
        self.root = params["rootUri"]
            .as_str()
            .and_then(uri_to_path)
            .map(|path| normalize_path(&path));
    }

    fn did_open(&mut self, params: &Value) {
        let document = &params["textDocument"];
        let (Some(uri), Some(text)) = (document["uri"].as_str(), document["text"].as_str()) else {
            return;
        };
        let Some(path) = uri_to_path(uri) else {
            return;
        };
        self.documents.insert(
            uri.to_string(),
            Document {
                version: document["version"].as_i64().unwrap_or(0),
                text: text.to_string(),
                path: normalize_path(&path),
            },
        );
    }

    /// FULL sync (§58): the whole text arrives in `contentChanges[0].text`, so
    /// the old bytes are replaced rather than patched.
    fn did_change(&mut self, params: &Value) {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return;
        };
        let Some(change) = params["contentChanges"]
            .as_array()
            .and_then(|changes| changes.first())
        else {
            return;
        };
        let Some(text) = change["text"].as_str() else {
            return;
        };
        let Some(document) = self.documents.get_mut(uri) else {
            return;
        };
        document.version = params["textDocument"]["version"]
            .as_i64()
            .unwrap_or(document.version + 1);
        document.text = text.to_string();
    }

    fn did_close(&mut self, params: &Value) {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return;
        };
        self.documents.remove(uri);
    }

    /// §58: the type of the top-level `def` under the cursor. A contract has no
    /// `def`, so its hover comes from the first candidate that could hold it.
    /// `check_all(false)` keeps the effect row of `main`, which is what
    /// `check --print-type` prints, so the two agree.
    fn hover(&self, params: &Value) -> Value {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return Value::Null;
        };
        let Some(document) = self.documents.get(uri) else {
            return Value::Null;
        };
        if let Some(version) = params["textDocument"]["version"].as_i64() {
            if version != document.version {
                return Value::Null;
            }
        }
        let Some(line) = params["position"]["line"].as_u64() else {
            return Value::Null;
        };
        let Some(character) = params["position"]["character"].as_u64() else {
            return Value::Null;
        };
        let (line, col) = decode_position(
            self.encoding,
            &document.text,
            line as usize,
            character as usize,
        );
        let entry = if is_extension(&document.path, "hdi") {
            match self.candidates_for(&document.path).into_iter().next() {
                Some(entry) => entry,
                None => return Value::Null,
            }
        } else {
            document.path.clone()
        };
        let Ok(project) = Project::load_with_overlay(&entry, &self.overlay()) else {
            return Value::Null;
        };
        let Some(name) = project.def_name_at(&document.path, line, col) else {
            return Value::Null;
        };
        let (_, checked) = project.check_all(false);
        let Some(checked) = checked else {
            return Value::Null;
        };
        if !checked.has_type(Some(&name)) {
            return Value::Null;
        }
        match checked.type_of(&name) {
            Some(ty) => json!({"contents": {"kind": "plaintext", "value": ty}}),
            None => Value::Null,
        }
    }

    /// §58: the local rewrites (§57) that start where the requested range
    /// starts. The edit is the rewrite's own byte range, so applying it makes
    /// the document byte-for-byte the one `check --fix` would write. A contract
    /// has no rewritable expression, so its list is empty.
    fn code_action(&self, params: &Value) -> Value {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return json!([]);
        };
        let Some(document) = self.documents.get(uri) else {
            return json!([]);
        };
        if let Some(version) = params["textDocument"]["version"].as_i64() {
            if version != document.version {
                return json!([]);
            }
        }
        if is_extension(&document.path, "hdi") {
            return json!([]);
        }
        let Some(line) = params["range"]["start"]["line"].as_u64() else {
            return json!([]);
        };
        let Some(character) = params["range"]["start"]["character"].as_u64() else {
            return json!([]);
        };
        let (line, col) = decode_position(
            self.encoding,
            &document.text,
            line as usize,
            character as usize,
        );
        let mut actions = Vec::new();
        for fix in crate::fixes::fixes(&document.text) {
            if fix.line != line || fix.col != col {
                continue;
            }
            let (start_line, start_col) = project::position_at(&document.text, fix.start);
            let (end_line, end_col) = project::position_at(&document.text, fix.end);
            let (start_line, start_character) =
                encode_position(self.encoding, &document.text, start_line, start_col);
            let (end_line, end_character) =
                encode_position(self.encoding, &document.text, end_line, end_col);
            let mut changes = Map::new();
            changes.insert(
                uri.to_string(),
                json!([{
                    "range": {
                        "start": {"line": start_line, "character": start_character},
                        "end": {"line": end_line, "character": end_character},
                    },
                    "newText": fix.replacement,
                }]),
            );
            actions.push(json!({
                "title": fix.canonical,
                "kind": "quickfix",
                "edit": {"changes": changes},
            }));
        }
        Value::Array(actions)
    }

    fn overlay(&self) -> project::Overlay {
        self.documents
            .values()
            .map(|document| (document.path.clone(), document.text.clone()))
            .collect()
    }

    /// Publish one `publishDiagnostics` per open document, an empty array
    /// included, and clear every URI that had diagnostics before it closed.
    fn publish<W: Write>(&mut self, out: &mut W) {
        let analyzed = self.analyze();
        for (uri, diagnostics) in &analyzed {
            let version = self
                .documents
                .get(uri)
                .map_or(0, |document| document.version);
            self.publish_one(out, uri, version, diagnostics);
        }
        let open: BTreeSet<&str> = self.documents.keys().map(String::as_str).collect();
        let closed: Vec<(String, i64)> = self
            .published
            .iter()
            .filter(|(uri, _)| !open.contains(uri.as_str()))
            .map(|(uri, version)| (uri.clone(), *version))
            .collect();
        for (uri, version) in closed {
            self.publish_one(out, &uri, version, &[]);
            self.published.remove(&uri);
        }
    }

    fn publish_one<W: Write>(
        &mut self,
        out: &mut W,
        uri: &str,
        version: i64,
        diagnostics: &[Value],
    ) {
        if self
            .published
            .get(uri)
            .is_some_and(|published| version < *published)
        {
            return;
        }
        let message = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "version": version, "diagnostics": diagnostics},
        });
        let _ = write_message(out, &message.to_string());
        self.published.insert(uri.to_string(), version);
    }

    /// The diagnostics of every open document, taken from the overlay. `.hd`
    /// documents are entries themselves; a `.hdi` takes the candidates of §58,
    /// and its diagnostics are the merge of the candidates that read it.
    fn analyze(&self) -> BTreeMap<String, Vec<Value>> {
        let overlay = self.overlay();
        let open: Vec<PathBuf> = self
            .documents
            .values()
            .map(|document| document.path.clone())
            .collect();
        let mut collected: BTreeMap<PathBuf, Vec<Diagnostic>> = BTreeMap::new();
        for entry in self.entry_candidates() {
            for (path, diagnostic) in diagnostics_of(&entry, &overlay, &open) {
                collected.entry(path).or_default().push(diagnostic);
            }
        }
        self.documents
            .iter()
            .map(|(uri, document)| {
                let diagnostics = collected
                    .get(&document.path)
                    .map_or_else(Vec::new, |list| self.render(list, &document.text));
                (uri.clone(), diagnostics)
            })
            .collect()
    }

    /// The entries a change is analyzed against. Open `.hd` documents are
    /// entries in their own right; with only contracts open, each contract
    /// picks the folder it belongs to and the `.hd` files under it.
    fn entry_candidates(&self) -> BTreeSet<PathBuf> {
        let hd = self.open_paths("hd");
        if !hd.is_empty() {
            return hd.into_iter().collect();
        }
        let mut entries = BTreeSet::new();
        for contract in self.open_paths("hdi") {
            entries.extend(self.candidates_for(&contract));
        }
        entries
    }

    /// The candidates for one contract: the open `.hd` documents when there are
    /// any, otherwise the `.hd` files under the folder the contract selects.
    fn candidates_for(&self, contract: &Path) -> Vec<PathBuf> {
        let hd = self.open_paths("hd");
        if !hd.is_empty() {
            return hd;
        }
        project::hd_files_under(&self.contract_root(contract))
            .into_iter()
            .map(|path| normalize_path(&path))
            .collect()
    }

    /// §58: the longest workspace folder whose PATH COMPONENTS are a prefix of
    /// the contract's, then `rootUri`, then the first folder. With no folders
    /// at all, the outermost ancestor that declares `library`, else the
    /// contract's own directory.
    fn contract_root(&self, contract: &Path) -> PathBuf {
        if self.folders.is_empty() && self.root.is_none() {
            return library_root(contract);
        }
        let mut best: Option<&Path> = None;
        for folder in &self.folders {
            if component_prefix(folder, contract)
                && best.is_none_or(|current| {
                    folder.components().count() > current.components().count()
                })
            {
                best = Some(folder);
            }
        }
        best.map(Path::to_path_buf)
            .or_else(|| self.root.clone())
            .or_else(|| self.folders.first().cloned())
            .unwrap_or_else(|| {
                contract
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf()
            })
    }

    fn open_paths(&self, extension: &str) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .documents
            .values()
            .filter(|document| is_extension(&document.path, extension))
            .map(|document| document.path.clone())
            .collect();
        paths.sort_by_key(|path| self.sort_key(path));
        paths
    }

    /// The byte order §50 uses: the path relative to the folder it belongs to,
    /// so the order does not depend on where the workspace root sits.
    fn sort_key(&self, path: &Path) -> Vec<u8> {
        let mut best: Option<&Path> = None;
        for root in self
            .folders
            .iter()
            .map(PathBuf::as_path)
            .chain(self.root.as_deref())
        {
            if component_prefix(root, path)
                && best
                    .is_none_or(|current| root.components().count() > current.components().count())
            {
                best = Some(root);
            }
        }
        match best.and_then(|root| path.strip_prefix(root).ok()) {
            Some(relative) => relative.as_os_str().as_encoded_bytes().to_vec(),
            None => path.as_os_str().as_encoded_bytes().to_vec(),
        }
    }

    /// Deduplicate by URI/line/column/code and order by the checker's stage
    /// order, so a merge does not depend on which candidate was opened first.
    fn render(&self, diagnostics: &[Diagnostic], text: &str) -> Vec<Value> {
        let mut ordered: Vec<&Diagnostic> = Vec::new();
        let mut seen = BTreeSet::new();
        for diagnostic in diagnostics {
            let key = (diagnostic.line, diagnostic.col, diagnostic.code.clone());
            if seen.insert(key) {
                ordered.push(diagnostic);
            }
        }
        ordered.sort_by_key(|diagnostic| {
            (crate::diagnostic_order(diagnostic), diagnostic.code.clone())
        });
        ordered
            .into_iter()
            .map(|diagnostic| diagnostic_value(self.encoding, text, diagnostic))
            .collect()
    }
}

/// One diagnostic as LSP sees it: a 0-based range in the negotiated encoding,
/// severity 1, the code, and `hado` as the source.
pub fn diagnostic_value(encoding: PositionEncoding, text: &str, error: &Diagnostic) -> Value {
    let (line, character) = encode_position(encoding, text, error.line, error.col);
    let (end_line, end_character) = encode_position(encoding, text, error.end_line, error.end_col);
    json!({
        "range": {
            "start": {"line": line, "character": character},
            "end": {"line": end_line, "character": end_character},
        },
        "severity": 1,
        "code": error.code,
        "source": "hado",
        "message": error.message,
    })
}

/// The diagnostics one entry contributes, paired with the physical file they
/// belong to. A load that fails with a positioned diagnostic still reports it:
/// a syntax error is exactly the kind of thing the editor must show.
fn diagnostics_of(
    entry: &Path,
    overlay: &project::Overlay,
    open: &[PathBuf],
) -> Vec<(PathBuf, Diagnostic)> {
    match Project::load_with_overlay(entry, overlay) {
        Ok(project) => {
            let paths: HashMap<String, PathBuf> = project
                .source_texts()
                .into_iter()
                .map(|(display, physical, _)| (display, physical))
                .collect();
            let (errors, _) = project.check_all(true);
            errors
                .into_iter()
                .map(|error| {
                    let physical = paths
                        .get(&error.path)
                        .cloned()
                        .unwrap_or_else(|| entry.to_path_buf());
                    (physical, project.ended(&error))
                })
                .collect()
        }
        Err(project::LoadError::Diagnostic(error)) => {
            let root = entry.parent().unwrap_or_else(|| Path::new("."));
            vec![(normalize_path(&root.join(&error.path)), error)]
        }
        // §51: an invalid namespace directory is a load refusal, not an E-code.
        // Dropping it here publishes an empty list, so the editor looks clean
        // while `hado check` refuses. The refusal belongs to the analysis root,
        // not to the entry that happened to walk it first, so every open
        // document under that root gets it. A `.hdi` opened on its own is not an
        // entry, and would otherwise never see the refusal of its candidates.
        Err(project::LoadError::Input(message)) => {
            let root = entry.parent().unwrap_or_else(|| Path::new("."));
            open.iter()
                .filter(|path| component_prefix(root, path))
                .map(|path| {
                    let diagnostic = Diagnostic {
                        related: None,
                        path: path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.to_string_lossy().into_owned()),
                        line: 1,
                        col: 1,
                        end_line: 1,
                        end_col: 1,
                        code: None,
                        message: message.clone(),
                    };
                    (path.clone(), diagnostic)
                })
                .collect()
        }
    }
}

fn respond<W: Write>(out: &mut W, id: &Option<Value>, result: Value) {
    let Some(id) = id else {
        return;
    };
    let message = json!({"jsonrpc": "2.0", "id": id, "result": result});
    let _ = write_message(out, &message.to_string());
}

fn respond_error<W: Write>(out: &mut W, id: &Option<Value>, code: i64, message: &str) {
    let Some(id) = id else {
        return;
    };
    let body = json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}});
    let _ = write_message(out, &body.to_string());
}

fn is_extension(path: &Path, extension: &str) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some(extension)
}

/// Whether every component of `prefix` is the same component of `path`, in
/// order. `<work>/w` is a prefix of `<work>/w/app` but not of
/// `<work>/w/application`; a string prefix would match neither correctly.
fn component_prefix(prefix: &Path, path: &Path) -> bool {
    let mut path_components = path.components();
    prefix
        .components()
        .all(|component| path_components.next() == Some(component))
}

/// The outermost ancestor of the contract that declares `library`, else the
/// contract's own directory. A `.hd` with a `library` declaration is an
/// analysis root; it is never built, so no E206 is raised for it.
pub fn library_root(contract: &Path) -> PathBuf {
    let parent = contract.parent().unwrap_or_else(|| Path::new("."));
    let mut root = parent.to_path_buf();
    for ancestor in parent.ancestors() {
        if directory_declares_library(ancestor) {
            root = ancestor.to_path_buf();
        }
    }
    root
}

fn directory_declares_library(directory: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_extension(&path, "hd") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Ok(tokens) = crate::lexer::lex_full(&text) {
            if tokens
                .iter()
                .any(|token| token.kind == crate::lexer::TokKind::KwLibrary)
            {
                return true;
            }
        }
    }
    false
}
