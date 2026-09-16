//! 正準形の局所修正（fix-it）。
//!
//! 冗長な書き方を、意味を変えずに正準形へ置き換える編集を 1 つの型 `Fix` に
//! まとめる。`hado check --fix` がこれを適用し、将来の `hado fmt` と
//! LSP の code action も同じ一覧を共有する。
//!
//! 編集は「ソースのバイト範囲 + 置換文字列」で表す。範囲は字句（`Span` は
//! 行・桁しか持たない）から、ソース自身の行頭表を使ってバイト位置に直す。
//! **範囲に `//` を含む修正は出さない。** コメントを消さないためである。
//!
//! 対象は §41・§44・§46・§53 の正準形のうち、まわりの木を変えずに書ける
//! ものだけ: E510（式の `Nil` / `Cons`）、E516（パターンの `Nil` / `Cons`）、
//! E517（`=> if | ...` の条件列を引数の腕へ）。

use crate::lexer::{lex_full, Span, Tok, TokKind};
use crate::parser::{self, Ast, DefKind, Item, Node, NodeKind, Pattern, PatternKind};
use std::collections::HashMap;

/// One local rewrite. `start..end` is a byte range of the source, `replacement`
/// is the canonical text written in its place, and `canonical` is the same
/// rewrite as a diagnostic hint. `line`/`col` is the anchor the checker uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    pub code: &'static str,
    pub line: usize,
    pub col: usize,
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub canonical: String,
}

/// The canonical rewrites a `check` diagnostic offers, in source order.
/// Overlapping edits keep the outermost one, so a `Cons` chain is fixed whole.
pub fn fixes(source: &str) -> Vec<Fix> {
    let Ok(ast) = parser::parse_source(source, "fix") else {
        return Vec::new();
    };
    let Ok(tokens) = lex_full(source) else {
        return Vec::new();
    };
    let lines = LineMap::new(source);
    let index = TokenIndex::new(&tokens);
    let mut found = Vec::new();

    for span in &ast.long_condition_functions {
        if let Some(fix) = condition_arm_fix(source, &lines, &tokens, &index, &ast, *span) {
            found.push(fix);
        }
    }
    for item in &ast.items {
        walk_item(item, source, &lines, &tokens, &index, &mut found);
    }

    // 同じ開始位置では外側（終わりが後ろ）を先に残し、重なる編集を落とす。
    found.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut accepted: Vec<Fix> = Vec::new();
    let mut end = 0;
    for fix in found {
        if fix.start >= end {
            end = fix.end;
            accepted.push(fix);
        }
    }
    accepted
}

/// The rewrite for one diagnostic, matched by code and anchor.
pub fn fix_at(source: &str, code: &str, line: usize, col: usize) -> Option<Fix> {
    fixes(source)
        .into_iter()
        .find(|fix| fix.code == code && fix.line == line && fix.col == col)
}

/// The canonical form to show next to a diagnostic, if one of the rewrites
/// starts exactly there.
pub fn hint(source: &str, code: &str, line: usize, col: usize) -> Option<String> {
    fix_at(source, code, line, col).map(|fix| fix.canonical)
}

/// Apply every local rewrite to the source, in passes, until none applies:
/// the bulk form `hado fmt` and `build --fix` use (§58.1). `Ok(None)` when the
/// source is already canonical or does not parse, `Err(limit)` when the
/// rewrites did not settle within `limit` passes.
pub fn format(source: &str) -> Result<Option<(String, usize)>, usize> {
    const LIMIT: usize = 64;
    let mut text = source.to_string();
    let mut applied = 0;
    for _ in 0..LIMIT {
        let edits = fixes(&text);
        if edits.is_empty() {
            return Ok(if applied == 0 { None } else { Some((text, applied)) });
        }
        applied += edits.len();
        text = splice(&text, &edits);
    }
    Err(LIMIT)
}

/// Write the disjoint edits back into the source. `fixes` returns them sorted
/// by start and never overlapping, so the tail is written first.
fn splice(source: &str, edits: &[Fix]) -> String {
    let mut updated = String::with_capacity(source.len());
    let mut at = 0;
    for fix in edits {
        updated.push_str(&source[at..fix.start]);
        updated.push_str(&fix.replacement);
        at = fix.end;
    }
    updated.push_str(&source[at..]);
    updated
}

pub fn is_fixable(code: &str) -> bool {
    matches!(code, "E501" | "E510" | "E515" | "E516" | "E517")
}

// --- E517: `def f x => if | ...` を `def f x | ...` へ ---

fn condition_arm_fix(
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    ast: &Ast,
    span: Span,
) -> Option<Fix> {
    let mut at = index.position(span)?;
    let mut depth = 0i32;
    while at < tokens.len() {
        match tokens[at].kind {
            TokKind::LParen | TokKind::LBracket | TokKind::LBrace => depth += 1,
            TokKind::RParen | TokKind::RBracket | TokKind::RBrace => depth -= 1,
            TokKind::Arrow if depth == 0 => break,
            TokKind::Eof => return None,
            _ => {}
        }
        at += 1;
    }
    let arrow = tokens.get(at)?;
    if !matches!(arrow.kind, TokKind::Arrow) {
        return None;
    }
    let if_token = tokens.get(at + 1)?;
    if !matches!(if_token.kind, TokKind::KwIf) {
        return None;
    }
    let mut start = lines.offset(source, arrow.span);
    let mut end = lines.offset(source, if_token.span) + 2;
    // Take the surrounding blanks too, so the rewrite leaves no trailing space.
    while start > 0 && matches!(source.as_bytes()[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    while end < source.len() && matches!(source.as_bytes()[end], b' ' | b'\t') {
        end += 1;
    }
    let def = def_at(&ast.items, span)?;
    Some(Fix {
        code: "E517",
        line: span.line,
        col: span.col,
        start,
        end,
        replacement: String::new(),
        canonical: parser::item_source(&Item::Def(def.clone())),
    })
}

fn def_at<'a>(items: &'a [Item], span: Span) -> Option<&'a parser::Def> {
    for item in items {
        match item {
            Item::Def(def) if def.span == span => return Some(def),
            Item::ModuleImpl(module) => {
                if let Some(found) = def_at(&module.items, span) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

// --- E510 / E516: 書かれた List 形 ---

fn walk_item(
    item: &Item,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    found: &mut Vec<Fix>,
) {
    match item {
        Item::Def(def) => {
            if let Some(fix) = sole_parameter_fix(def, source, lines, tokens, index) {
                found.push(fix);
            }
            match &def.kind {
                DefKind::Value(body) => walk_node(body, source, lines, tokens, index, found),
                DefKind::Function { body, .. } => walk_node(body, source, lines, tokens, index, found),
            }
        }
        Item::ModuleImpl(module) => {
            for item in &module.items {
                walk_item(item, source, lines, tokens, index, found);
            }
        }
        _ => {}
    }
}

// --- E515: `def f x => if x | ...` の引数を腕へ移す ---

fn sole_parameter_fix(
    def: &parser::Def,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
) -> Option<Fix> {
    let DefKind::Function { params, ret, body } = &def.kind else { return None };
    if !crate::check::sole_parameter_branch(params, ret.as_ref(), body) {
        return None;
    }
    let [parameter] = params.as_slice() else { return None };
    let NodeKind::PatternBranch { scrutinees, .. } = &body.kind else { return None };
    let [scrutinee] = scrutinees.as_slice() else { return None };
    let canonical = parser::arm_function_source(def);
    // 型注釈の無い引数は括弧を取らない（`def f (x)` は E100）ので、綴りは名前だけ。
    let after_name = tokens.get(index.position(def.name_span)? + 1)?;
    if !matches!(&after_name.kind, TokKind::Name(name) if name == &parameter.name) {
        return None;
    }
    let mut start = lines.offset(source, after_name.span);
    let end = lines.offset(source, scrutinee.span) + parameter.name.len();
    // 空白も範囲に入れ、行末に空白を残さない。
    while start > 0 && matches!(source.as_bytes()[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    if end > source.len() || source[start..end].contains("//") {
        return None;
    }
    Some(Fix {
        code: "E515",
        line: def.span.line,
        col: def.span.col,
        start,
        end,
        replacement: String::new(),
        canonical,
    })
}

// --- E501: `let x = v -> x` を `v` にする ---

fn let_identity_fix(
    anchor: Span,
    bound: &str,
    body: Span,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
) -> Option<Fix> {
    let at = index.position(anchor)?;
    if !matches!(tokens.get(at)?.kind, TokKind::KwLet) {
        return None;
    }
    // 綴りが constructor のパターン（`Nil` は `[]` と同じ AST になる）は値の
    // 束縛ではないので、小文字で始まる名前だけを対象にする。値の綴りも
    // トークンで確かめる（`[]` を `Nil` と見て 1 文字余分に食わないため）。
    if !bound.starts_with(char::is_lowercase) {
        return None;
    }
    if !matches!(&tokens.get(at + 1)?.kind, TokKind::Name(name) if name == bound) {
        return None;
    }
    let equals = tokens.get(at + 2)?;
    if !matches!(equals.kind, TokKind::Eq) {
        return None;
    }
    // 値の式は入れ子の `let` で `->` を持ち得るので、分離点は本体の直前の
    // `->` で決める（字句の走査では決まらない）。
    let body_at = index.position(body)?;
    if !matches!(tokens.get(body_at - 1)?.kind, TokKind::DashArrow) {
        return None;
    }
    let start = lines.offset(source, anchor);
    let value_start = lines.offset(source, equals.span) + 1;
    let value_end = lines.offset(source, tokens[body_at - 1].span);
    let end = lines.offset(source, body) + bound.len();
    if value_start > value_end || end > source.len() || source[start..end].contains("//") {
        return None;
    }
    let replacement = source[value_start..value_end].trim().to_string();
    if replacement.is_empty() {
        return None;
    }
    Some(Fix {
        code: "E501",
        line: anchor.line,
        col: anchor.col,
        start,
        end,
        canonical: replacement.clone(),
        replacement,
    })
}

fn walk_node(
    node: &Node,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    found: &mut Vec<Fix>,
) {
    match &node.kind {
        NodeKind::Name(name) if name == "Nil" => {
            if spelled(source, lines, node.span, "Nil") {
                push_edit(found, "E510", node.span, "[]", source, lines, 3);
            }
        }
        NodeKind::Call(callee, _) | NodeKind::Apply(callee, _) => {
            if matches!(&callee.kind, NodeKind::Name(name) if name == "Cons") {
                if let Some(text) = list_expression(node) {
                    push_cons_edit(found, "E510", node.span, &text, source, lines, tokens, index);
                }
            }
        }
        NodeKind::Let { pat, body, .. } => {
            walk_pattern(pat, source, lines, tokens, index, found);
            if let (PatternKind::Name(bound), NodeKind::Name(returned)) = (&pat.kind, &body.kind) {
                if bound == returned {
                    if let Some(fix) = let_identity_fix(node.span, bound, body.span, source, lines, tokens, index) {
                        found.push(fix);
                    }
                }
            }
        }
        NodeKind::LetQ { pat, .. } => {
            walk_pattern(pat, source, lines, tokens, index, found);
        }
        NodeKind::PatternBranch { arms, .. } => {
            for (patterns, _) in arms {
                for pattern in patterns {
                    walk_pattern(pattern, source, lines, tokens, index, found);
                }
            }
        }
        _ => {}
    }
    // 書かれた `Cons(...)` の綴り自体は呼び出し側で報告するので、被演算子は飛ばす。
    let cons_call = matches!(&node.kind, NodeKind::Call(callee, _) | NodeKind::Apply(callee, _)
        if matches!(&callee.kind, NodeKind::Name(name) if name == "Cons"));
    for (position, child) in crate::check::children(node).into_iter().enumerate() {
        if cons_call && position == 0 {
            continue;
        }
        walk_node(child, source, lines, tokens, index, found);
    }
}

fn walk_pattern(
    pattern: &Pattern,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    found: &mut Vec<Fix>,
) {
    match &pattern.kind {
        PatternKind::Name(name) if name == "Nil" => {
            if spelled(source, lines, pattern.span, "Nil") {
                push_edit(found, "E516", pattern.span, "[]", source, lines, 3);
            }
        }
        PatternKind::Constructor(name, args) if name == "Cons" && args.len() == 2 => {
            if let Some(text) = parser::list_pattern_source(pattern) {
                push_cons_edit(found, "E516", pattern.span, &text, source, lines, tokens, index);
            }
        }
        PatternKind::Constructor(_, args) => {
            for arg in args {
                walk_pattern(arg, source, lines, tokens, index, found);
            }
        }
        PatternKind::Record(fields) | PatternKind::NamedConstructor(_, fields) => {
            for (_, field) in fields {
                if let Some(field) = field {
                    walk_pattern(field, source, lines, tokens, index, found);
                }
            }
        }
        _ => {}
    }
}

/// The canonical `[a, ...b]` text of a written `Cons` / `Nil` expression,
/// or `None` when the node is not a list.
fn list_expression(node: &Node) -> Option<String> {
    if matches!(&node.kind, NodeKind::Name(name) if name == "Nil") {
        return Some("[]".into());
    }
    let (NodeKind::Call(callee, args) | NodeKind::Apply(callee, args)) = &node.kind else {
        return None;
    };
    if !matches!(&callee.kind, NodeKind::Name(name) if name == "Cons") || args.len() != 2 {
        return None;
    }
    let mut parts = vec![parser::canonical(&args[0])];
    let mut tail = &args[1];
    loop {
        match &tail.kind {
            NodeKind::Name(name) if name == "Nil" => break,
            NodeKind::Call(callee, args) | NodeKind::Apply(callee, args)
                if matches!(&callee.kind, NodeKind::Name(name) if name == "Cons") && args.len() == 2 =>
            {
                parts.push(parser::canonical(&args[0]));
                tail = &args[1];
            }
            _ => {
                parts.push(format!("...{}", parser::canonical(tail)));
                break;
            }
        }
    }
    Some(format!("[{}]", parts.join(", ")))
}

/// True when the source at `anchor` spells `text`. The canonical `[]` and the
/// written `Nil` share one AST node, so an edit must check the spelling before
/// it fires: `hado fmt` walks the AST without a diagnostic to vouch for it.
fn spelled(source: &str, lines: &LineMap, anchor: Span, text: &str) -> bool {
    let start = lines.offset(source, anchor);
    let end = start + text.len();
    end <= source.len() && &source[start..end] == text
}

// --- 編集の組み立て ---

#[allow(clippy::too_many_arguments)]
fn push_edit(
    found: &mut Vec<Fix>,
    code: &'static str,
    anchor: Span,
    replacement: &str,
    source: &str,
    lines: &LineMap,
    length: usize,
) {
    let start = lines.offset(source, anchor);
    let end = start + length;
    if end > source.len() || source[start..end].contains("//") {
        return;
    }
    found.push(Fix {
        code,
        line: anchor.line,
        col: anchor.col,
        start,
        end,
        replacement: replacement.into(),
        canonical: replacement.into(),
    });
}

#[allow(clippy::too_many_arguments)]
fn push_cons_edit(
    found: &mut Vec<Fix>,
    code: &'static str,
    anchor: Span,
    replacement: &str,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
) {
    let Some(at) = index.position(anchor) else { return };
    if !matches!(tokens.get(at).map(|tok| &tok.kind), Some(TokKind::Name(name)) if name == "Cons") {
        return;
    }
    let Some(open) = tokens.get(at + 1) else { return };
    let TokKind::LParen = open.kind else { return };
    let mut depth = 0i32;
    let mut close = None;
    for (offset, token) in tokens[at + 1..].iter().enumerate() {
        match token.kind {
            TokKind::LParen => depth += 1,
            TokKind::RParen => {
                depth -= 1;
                if depth == 0 {
                    close = Some(at + 1 + offset);
                    break;
                }
            }
            TokKind::Eof => break,
            _ => {}
        }
    }
    let Some(close) = close else { return };
    let start = lines.offset(source, anchor);
    let end = lines.offset(source, tokens[close].span) + 1;
    if end > source.len() || source[start..end].contains("//") {
        return;
    }
    found.push(Fix {
        code,
        line: anchor.line,
        col: anchor.col,
        start,
        end,
        replacement: replacement.into(),
        canonical: replacement.into(),
    });
}

// --- 位置の道具 ---

struct LineMap {
    starts: Vec<usize>,
}

impl LineMap {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        for (at, ch) in source.char_indices() {
            if ch == '\n' {
                starts.push(at + 1);
            }
        }
        Self { starts }
    }

    /// The byte offset of a 1-based line / column span. `col` counts characters.
    fn offset(&self, source: &str, span: Span) -> usize {
        let Some(&start) = self.starts.get(span.line.saturating_sub(1)) else {
            return source.len();
        };
        source[start..]
            .char_indices()
            .nth(span.col.saturating_sub(1))
            .map(|(at, _)| start + at)
            .unwrap_or(source.len())
    }
}

struct TokenIndex {
    by_span: HashMap<(usize, usize), usize>,
}

impl TokenIndex {
    fn new(tokens: &[Tok]) -> Self {
        Self {
            by_span: tokens
                .iter()
                .enumerate()
                .map(|(at, token)| ((token.span.line, token.span.col), at))
                .collect(),
        }
    }

    fn position(&self, span: Span) -> Option<usize> {
        self.by_span.get(&(span.line, span.col)).copied()
    }
}
