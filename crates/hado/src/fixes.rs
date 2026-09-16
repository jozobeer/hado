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
//! E517（`=> if | ...` の条件列を引数の腕へ）、E507（腕がすべて自分の pattern を
//! 組み直す分岐は scrutinee）、E508（`None` の再伝播は `let p? =`）。

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
        walk_item(item, source, &lines, &tokens, &index, &ast.branch_spans, &mut found);
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
    let mut settled = false;
    for _ in 0..LIMIT {
        let edits = fixes(&text);
        if edits.is_empty() {
            settled = true;
            break;
        }
        applied += edits.len();
        text = splice(&text, &edits);
    }
    if !settled {
        return Err(LIMIT);
    }
    // §58.1: the bulk form lays the text out as well. `check --fix` applies the
    // diagnostic-anchored rewrites only, so it never comes through here.
    let (laid_out, layout_applied) = layout(&text);
    applied += layout_applied;
    if applied == 0 {
        return Ok(None);
    }
    Ok(Some((laid_out, applied)))
}

/// §58.1: 行末の空白（スペースとタブ）を消し、ファイルの終わりを改行 1 つにする。
/// 空のファイルはそのまま。字下げのタブと改行コードは変えない。
fn layout(source: &str) -> (String, usize) {
    if source.is_empty() {
        return (String::new(), 0);
    }
    let body = source.trim_end_matches('\n');
    let mut applied = 0;
    let mut out = String::with_capacity(source.len() + 1);
    for (index, line) in body.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_end_matches([' ', '\t']);
        if trimmed.len() != line.len() {
            applied += 1;
        }
        out.push_str(trimmed);
    }
    out.push('\n');
    if body.len() + 1 != source.len() {
        applied += 1;
    }
    (out, applied)
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
    matches!(code, "E501" | "E502" | "E503" | "E504" | "E506" | "E512" | "E507" | "E520" | "E508" | "E510" | "E515" | "E516" | "E517")
}

// --- E520: Bool リテラルの腕を Boolean 分岐へ ---

/// §24: `if b | true => x | false => y` と `if b | true => x | y` の canonical form は
/// `if b => x | y`。腕は真偽で選び直すので、書かれた順が逆でも値は変わらない。
/// 式の範囲は `branch_range` に頼るので、枝の位置にある形だけを扱う。
#[allow(clippy::too_many_arguments)]
fn boolean_branch_fix(
    node: &Node,
    arms: &[(Vec<Pattern>, Node)],
    fallback: Option<&Node>,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
) -> Option<Fix> {
    let NodeKind::PatternBranch { scrutinees, .. } = &node.kind else { return None };
    if scrutinees.len() != 1 {
        return None;
    }
    let literal = |patterns: &[Pattern]| -> Option<bool> {
        let [pattern] = patterns else { return None };
        let PatternKind::Literal(value) = &pattern.kind else { return None };
        let NodeKind::Bool(value) = &value.kind else { return None };
        Some(*value)
    };
    // 腕 2 本は真偽で 1 対 1、腕 1 本は fallback が残り半分。Bool の分岐は 2 本なので、
    // 腕 2 本と fallback が同居する形には正準形が無い。
    let (then, or_else) = match (arms, fallback) {
        ([(first, first_body), (second, second_body)], None) => match (literal(first)?, literal(second)?) {
            (true, false) => (first_body, second_body),
            (false, true) => (second_body, first_body),
            _ => return None,
        },
        ([(only, only_body)], Some(alternative)) => match literal(only)? {
            true => (only_body, alternative),
            false => (alternative, only_body),
        },
        _ => return None,
    };
    let (start, end) = branch_range(node, Tail::If, source, lines, tokens, index, branches)?;
    if source[start..end].contains("//") {
        return None;
    }
    let replacement = format!(
        "if {} => {} | {}",
        parser::canonical(&scrutinees[0]),
        parser::canonical(then),
        parser::canonical(or_else)
    );
    Some(Fix {
        code: "E520",
        line: node.span.line,
        col: node.span.col,
        start,
        end,
        canonical: flatten(&replacement),
        replacement,
    })
}

// --- E503: `| _ => body` を fallback `| body` へ ---

/// §29: the canonical form of a catch-all `_` is the fallback. Removing `_ =>`
/// leaves the arm's body as the fallback; a comment after the arrow is outside
/// the removed range, so it stays.
/// §26 / §57: the last arm's whole row is one variable pattern, so the value is
/// still bound there and the arm is the fallback with a name. A `check --fix` run
/// applies only one range, so the rewrite is one edit: the pattern through the end
/// of the arm becomes the body with the binding replaced by the scrutinee.
fn catch_all_variable_fix(
    node: &Node,
    arms: &[(Vec<Pattern>, Node)],
    fallback: Option<&Node>,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
) -> Option<Fix> {
    let last = arms.last()?;
    if fallback.is_some() || last.0.len() != 1 {
        return None;
    }
    let pattern = &last.0[0];
    if !matches!(&pattern.kind, PatternKind::Name(name) if !crate::names::is_upper_reference(name)) {
        return None;
    }
    let NodeKind::PatternBranch { scrutinees, .. } = &node.kind else { return None };
    let [scrutinee] = scrutinees.as_slice() else { return None };
    let NodeKind::Name(_) = &scrutinee.kind else { return None };
    let body = &last.1;
    let start = lines.offset(source, pattern.span);
    let (_, end) = branch_range(node, Tail::If, source, lines, tokens, index, branches)?;
    // A comment would be lost, and a `let` could bind the same name again.
    if start >= end || source[start..end].contains("//") || source[start..end].contains("let") {
        return None;
    }
    let pattern_at = index.position(pattern.span)?;
    let binding = source[start..lines.offset(source, tokens.get(pattern_at + 1)?.span)].trim();
    let scrutinee_at = index.position(scrutinee.span)?;
    let scrutinee_start = lines.offset(source, scrutinee.span);
    let value = source[scrutinee_start..lines.offset(source, tokens.get(scrutinee_at + 1)?.span)].trim();
    if binding.is_empty() || value.is_empty() {
        return None;
    }
    let mut replacement = String::new();
    let mut at = index.position(body.span)?;
    while let Some(token) = tokens.get(at) {
        let token_start = lines.offset(source, token.span);
        if token_start >= end || matches!(token.kind, TokKind::Eof) {
            break;
        }
        let token_end = match tokens.get(at + 1) {
            Some(next) => lines.offset(source, next.span).min(end),
            None => end,
        };
        let text = &source[token_start..token_end];
        if text.trim() == binding {
            replacement.push_str(value);
        } else {
            replacement.push_str(text);
        }
        at += 1;
    }
    if replacement.trim().is_empty() {
        return None;
    }
    // The substituted form is still redundant when every arm rebuilds the
    // scrutinee (§41). The checker refuses to fold a fallback because it may
    // re-evaluate the scrutinee, but this scrutinee is a plain variable, so the
    // fold cannot change how often an effect runs. Each arm is checked here: a
    // constructor arm must return its own pattern, a variable arm the scrutinee.
    let scrutinee_text = parser::canonical(scrutinee);
    let mut reconstructs = replacement.trim() == scrutinee_text.trim();
    for (arm_patterns, arm_body) in &arms[..arms.len() - 1] {
        reconstructs = reconstructs && arm_patterns.len() == 1;
        if !reconstructs {
            break;
        }
        let arm_pattern = &arm_patterns[0];
        let Some(at) = index.position(arm_pattern.span) else { reconstructs = false; break };
        let Some(next) = tokens.get(at + 1) else { reconstructs = false; break };
        let pattern_text = source[lines.offset(source, arm_pattern.span)..lines.offset(source, next.span)].trim();
        let body_text = parser::canonical(arm_body);
        reconstructs = match &arm_pattern.kind {
            PatternKind::Name(name) if crate::names::is_upper_reference(name) => body_text.trim() == pattern_text,
            PatternKind::Name(_) | PatternKind::Underscore => body_text.trim() == scrutinee_text.trim(),
            _ => body_text.trim() == pattern_text,
        };
    }
    if reconstructs {
        let (whole_start, whole_end) = branch_range(node, Tail::If, source, lines, tokens, index, branches)?;
        if whole_start < whole_end && !source[whole_start..whole_end].contains("//") {
            return Some(Fix {
                code: "E512",
                line: pattern.span.line,
                col: pattern.span.col,
                start: whole_start,
                end: whole_end,
                replacement: scrutinee_text.clone(),
                canonical: scrutinee_text,
            });
        }
    }
    Some(Fix {
        code: "E512",
        line: pattern.span.line,
        col: pattern.span.col,
        start,
        end,
        canonical: format!("| {}", replacement.trim()),
        replacement,
    })
}

fn discard_let_fix(
    node: &Node,
    pat: &Pattern,
    value: &Node,
    code: &'static str,
    canonical: &str,
    source: &str,
    lines: &LineMap,
) -> Option<Fix> {
    if !matches!(pat.kind, PatternKind::Underscore) {
        return None;
    }
    let start = lines.offset(source, node.span);
    let end = lines.offset(source, value.span);
    if start >= end || source[start..end].contains("//") {
        return None;
    }
    Some(Fix {
        code,
        line: pat.span.line,
        col: pat.span.col,
        start,
        end,
        replacement: "do ".to_string(),
        canonical: canonical.to_string(),
    })
}

/// §42: when both results of a condition branch are the same, the condition is
/// redundant and the branch collapses to that one result. One range.
fn identical_bool_fix(
    node: &Node,
    then: &Node,
    els: &Node,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
) -> Option<Fix> {
    let text = parser::canonical(then);
    if text.is_empty() || text.contains('\n') || text != parser::canonical(els) {
        return None;
    }
    let body_start = lines.offset(source, then.span);
    if !source[body_start..].starts_with(&text) {
        return None;
    }
    let (start, end) = match branch_range(node, Tail::If, source, lines, tokens, index, branches) {
        Some(range) => range,
        None => (
            lines.offset(source, node.span),
            lines.offset(source, els.span) + text.len(),
        ),
    };
    if start >= end || start > body_start || end < body_start + text.len() {
        return None;
    }
    if source[start..end].contains("//") {
        return None;
    }
    Some(Fix {
        code: "E502",
        line: node.span.line,
        col: node.span.col,
        start,
        end,
        canonical: flatten(&text),
        replacement: text,
    })
}

/// §40: a plain let binding that discards a pure value is redundant; the
/// binding disappears and the body stays. Only a bare name or a literal is
/// provably effect-free (E506 fires for effectful discards too, so anything
/// else is left for the author to rewrite as a `do`).
fn discard_plain_let_fix(
    node: &Node,
    pat: &Pattern,
    value: &Node,
    body: &Node,
    canonical: &str,
    source: &str,
    lines: &LineMap,
) -> Option<Fix> {
    if !matches!(pat.kind, PatternKind::Underscore) {
        return None;
    }
    let start = lines.offset(source, node.span);
    let value_start = lines.offset(source, value.span);
    let end = lines.offset(source, body.span);
    if start >= end || value_start >= end {
        return None;
    }
    let head = source[value_start..end].trim_end();
    let text = head.strip_suffix("->").unwrap_or(head).trim();
    let bare_name = !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_');
    let literal = text.len() >= 2 && text.starts_with('"') && text.ends_with('"');
    if !bare_name && !literal {
        return None;
    }
    if source[start..end].contains("//") {
        return None;
    }
    Some(Fix {
        code: "E506",
        line: pat.span.line,
        col: pat.span.col,
        start,
        end,
        canonical: flatten(canonical),
        replacement: String::new(),
    })
}

fn catch_all_fallback_fix(
    arms: &[(Vec<Pattern>, Node)],
    fallback: Option<&Node>,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
) -> Option<Fix> {
    // A real fallback has no pattern to remove, and the diagnostic is not about
    // a nested `_` such as `Member(name, _)`.
    let last = arms.last()?;
    if fallback.is_some() || last.0.len() != 1 {
        return None;
    }
    let pattern = &last.0[0];
    if !matches!(pattern.kind, PatternKind::Underscore) {
        return None;
    }
    let body = &last.1;
    let start = lines.offset(source, pattern.span);
    // The `_` pattern cannot contain an arrow, so the first one at the top level
    // after it ends the arm.
    let mut at = index.position(pattern.span)?;
    let mut depth = 0i32;
    let mut end = None;
    while let Some(token) = tokens.get(at) {
        match token.kind {
            TokKind::LParen | TokKind::LBracket | TokKind::LBrace => depth += 1,
            TokKind::RParen | TokKind::RBracket | TokKind::RBrace => depth -= 1,
            TokKind::Arrow if depth == 0 => {
                end = Some(lines.offset(source, token.span) + 2);
                break;
            }
            TokKind::Eof => break,
            _ => {}
        }
        at += 1;
    }
    let mut end = end?;
    // Leave the blank before `_` as the one space after `|`.
    while end < source.len() && matches!(source.as_bytes()[end], b' ' | b'\t') {
        end += 1;
    }
    if end > source.len() || source[start..end].contains("//") {
        return None;
    }
    Some(Fix {
        code: "E503",
        line: pattern.span.line,
        col: pattern.span.col,
        start,
        end,
        replacement: String::new(),
        canonical: format!("| {}", parser::canonical(body)),
    })
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

#[allow(clippy::too_many_arguments)]
fn walk_item(
    item: &Item,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
    found: &mut Vec<Fix>,
) {
    match item {
        Item::Def(def) => {
            if let Some(fix) = sole_parameter_fix(def, source, lines, tokens, index) {
                found.push(fix);
            }
            let arms = match &def.kind {
                DefKind::Function { params, .. } => arm_form(params),
                _ => false,
            };
            match &def.kind {
                DefKind::Value(body) => {
                    // 値の定義の本体は末尾なので、`let p? =` の伝播先はこの定義。
                    if let Some(fix) = repropagate_fix(
                        body,
                        Tail::If,
                        None,
                        source,
                        lines,
                        tokens,
                        index,
                        branches,
                    ) {
                        found.push(fix);
                    }
                    walk_node(body, source, lines, tokens, index, branches, found);
                }
                DefKind::Function { body, .. } => {
                    if arms {
                        let identity = format!(
                            "def {}{} value => value",
                            if def.rec { "rec " } else { "" },
                            def.name
                        );
                        if let Some(fix) =
                            rewrap_arm_fix(body, &identity, Tail::Def, source, lines, tokens, index, branches)
                        {
                            found.push(fix);
                        }
                    }
                    let tail = if arms { Tail::Def } else { Tail::If };
                    if let Some(fix) =
                        repropagate_fix(body, tail, Some(def), source, lines, tokens, index, branches)
                    {
                        found.push(fix);
                    }
                    walk_node(body, source, lines, tokens, index, branches, found);
                }
            }
        }
        Item::ModuleImpl(module) => {
            for item in &module.items {
                walk_item(item, source, lines, tokens, index, branches, found);
            }
        }
        _ => {}
    }
}

/// The §31 / §32 arm forms (`def f | ...`, `fn(| ...)`) lower to one parameter
/// named `$argument`, which cannot be written in source.
fn arm_form(params: &[parser::Param]) -> bool {
    matches!(params, [parameter] if parameter.name == "$argument")
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

// --- E507 / E508: Option の再包装と再伝播 ---

/// The shape of the function whose body is the branch (§57). `let p? =`
/// propagates absence out of the enclosing function, so a rewrite is only
/// offered where the branch is that function's body.
#[derive(Clone, Copy, PartialEq)]
enum Tail {
    /// A branch in tail position: the branch itself is replaced.
    If,
    /// The §31 `def f | ...` form: the definition is rewritten with `value`.
    Def,
    /// The §31 `fn(| ...)` form.
    Fn,
}

/// E507: `if E | ...` is `E` when every arm rebuilds its own pattern.
#[allow(clippy::too_many_arguments)]
fn rewrap_if_fix(
    node: &Node,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
) -> Option<Fix> {
    let scrutinee = crate::check::rewrap_branch(node)?;
    // The arm forms carry their own rewrite: their scrutinee is `$argument` and
    // the branch spans the whole definition.
    if matches!(&scrutinee.kind, NodeKind::Name(name) if name == "$argument") {
        return None;
    }
    let NodeKind::PatternBranch { fallback, .. } = &node.kind else { return None };
    // A fallback re-evaluates the scrutinee; folding it into one evaluation can
    // change how often an effect runs.
    if fallback.is_some() {
        return None;
    }
    let (start, end) = branch_range(node, Tail::If, source, lines, tokens, index, branches)?;
    if source[start..end].contains("//") {
        return None;
    }
    let replacement = parser::canonical(scrutinee);
    Some(Fix {
        code: "E507",
        line: node.span.line,
        col: node.span.col,
        start,
        end,
        canonical: flatten(&replacement),
        replacement,
    })
}

/// E507 for the §31 arm forms: every arm rebuilds the value, so the definition
/// is the identity function.
#[allow(clippy::too_many_arguments)]
fn rewrap_arm_fix(
    node: &Node,
    replacement: &str,
    tail: Tail,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
) -> Option<Fix> {
    if crate::check::rewrap_branch(node).is_none() {
        return None;
    }
    let NodeKind::PatternBranch { fallback, .. } = &node.kind else { return None };
    if fallback.is_some() {
        return None;
    }
    let (start, end) = branch_range(node, tail, source, lines, tokens, index, branches)?;
    if source[start..end].contains("//") {
        return None;
    }
    Some(Fix {
        code: "E507",
        line: node.span.line,
        col: node.span.col,
        start,
        end,
        replacement: replacement.into(),
        canonical: replacement.into(),
    })
}

/// E508: `if E | Some(p) => body | None => None` in tail position is
/// `let p? = E -> body` (§36). The rewrite needs a binder that the body uses:
/// with none, or with one the body ignores, `do` is the canonical form.
#[allow(clippy::too_many_arguments)]
fn repropagate_fix(
    node: &Node,
    tail: Tail,
    def: Option<&parser::Def>,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
) -> Option<Fix> {
    let NodeKind::PatternBranch {
        scrutinees,
        arms,
        fallback,
    } = &node.kind
    else {
        return None;
    };
    if fallback.is_some() {
        return None;
    }
    let [scrutinee] = scrutinees.as_slice() else { return None };
    let [(patterns, body), (absence_patterns, absence_body)] = arms.as_slice() else {
        return None;
    };
    let ([pattern], [absence]) = (patterns.as_slice(), absence_patterns.as_slice()) else {
        return None;
    };
    if !matches!(&absence.kind, PatternKind::Name(name) if name == "None")
        || !crate::normalize::rebuilds_pattern(absence, absence_body)
    {
        return None;
    }
    // `let p? = E` matches `p` against the value inside the Option (§36), so the
    // first arm must be `Some(p)` with one argument.
    let PatternKind::Constructor(name, arguments) = &pattern.kind else { return None };
    if name != "Some" || arguments.len() != 1 {
        return None;
    }
    let inner = &arguments[0];
    let binders = crate::names::bindings(inner);
    if binders.is_empty()
        || !binders
            .iter()
            .all(|(binder, _)| crate::names::references(body, binder))
    {
        return None;
    }
    // The rewritten definition names its argument `value`; a free `value` in
    // the body would be captured by that name.
    if tail != Tail::If && crate::names::references(body, "value") {
        return None;
    }
    // With every arm rebuilding, E507 is reported and its rewrite is shorter.
    if crate::check::rewrap_branch(node).is_some() {
        return None;
    }
    // The anchor is the `None` arm's `|`, the position the checker reports
    // (check::arm_diagnostic moves the code there through branch_anchors).
    let pipe = tokens.get(index.position(absence.span)?.checked_sub(1)?)?;
    if !matches!(pipe.kind, TokKind::Pipe) {
        return None;
    }
    let (_, first_pipe, _) = *branch_entry(node.span, branches)?;
    let (start, end) = branch_range(node, tail, source, lines, tokens, index, branches)?;
    if source[start..end].contains("//") {
        return None;
    }
    // The scrutinee ends where the first arm's `|` begins; the first arm's body
    // ends where the `None` arm's `|` begins.
    let value_end = trim_back(source, lines.offset(source, first_pipe));
    let body_end = trim_back(source, lines.offset(source, pipe.span));
    let value = match tail {
        Tail::If => source[lines.offset(source, scrutinee.span)..value_end].trim(),
        Tail::Def | Tail::Fn => "value",
    };
    let body_text = &source[lines.offset(source, body.span)..body_end];
    let let_form = format!("let {}? = {} -> {}", pattern_text(inner), value, body_text);
    let replacement = match tail {
        Tail::If => let_form,
        Tail::Def => {
            let def = def?;
            format!(
                "def {}{} value => {let_form}",
                if def.rec { "rec " } else { "" },
                def.name
            )
        }
        Tail::Fn => format!("fn(value => {let_form})"),
    };
    Some(Fix {
        code: "E508",
        line: pipe.span.line,
        col: pipe.span.col,
        start,
        end,
        canonical: flatten(&replacement),
        replacement,
    })
}

/// The byte range of one branch: its first token through its last arm. `None`
/// when the parser recorded no branch at that span.
fn branch_range(
    node: &Node,
    tail: Tail,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
) -> Option<(usize, usize)> {
    let (_, _, after) = *branch_entry(node.span, branches)?;
    let start = lines.offset(source, node.span);
    // The `fn` arm form is parenthesized and the parser stops before the `)`
    // that closes it, so the range has to include that `)`.
    let end = if tail == Tail::Fn {
        let mut depth = 0i32;
        let mut close = None;
        for token in &tokens[index.position(node.span)?..] {
            match token.kind {
                TokKind::LParen => depth += 1,
                TokKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(lines.offset(source, token.span) + 1);
                        break;
                    }
                }
                TokKind::Eof => break,
                _ => {}
            }
        }
        close?
    } else {
        trim_back(source, lines.offset(source, after))
    };
    (start <= end).then_some((start, end))
}

/// The parser's record for one branch: its start, its first `|`, and the token
/// after it.
fn branch_entry(span: Span, branches: &[(Span, Span, Span)]) -> Option<&(Span, Span, Span)> {
    branches.iter().find(|(start, _, _)| *start == span)
}

/// Blanks between the branch and the next token stay out of the range, so the
/// rewrite leaves the next token where it was.
fn trim_back(source: &str, end: usize) -> usize {
    let mut at = end.min(source.len());
    while at > 0 && source.as_bytes()[at - 1].is_ascii_whitespace() {
        at -= 1;
    }
    at
}

/// A diagnostic prints on one line; the written text keeps the author's breaks.
fn flatten(text: &str) -> String {
    text.replace('\n', " ")
}

/// The canonical spelling of a pattern, with the list forms of E516.
fn pattern_text(pattern: &Pattern) -> String {
    parser::list_pattern_source(pattern).unwrap_or_else(|| pattern.to_string())
}

#[allow(clippy::too_many_arguments)]
fn walk_node(
    node: &Node,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
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
        NodeKind::Let { pat, value, body, .. } => {
            if let Some(fix) = discard_plain_let_fix(node, pat, value, body, "（束縛を削除）", source, lines) {
                found.push(fix);
            }
            walk_pattern(pat, source, lines, tokens, index, found);
            if let (PatternKind::Name(bound), NodeKind::Name(returned)) = (&pat.kind, &body.kind) {
                if bound == returned {
                    if let Some(fix) = let_identity_fix(node.span, bound, body.span, source, lines, tokens, index) {
                        found.push(fix);
                    }
                }
            }
        }
        NodeKind::LetQ { pat, value, .. } => {
            if let Some(fix) = discard_let_fix(node, pat, value, "E504", "do", source, lines) {
                found.push(fix);
            }
            walk_pattern(pat, source, lines, tokens, index, found);
        }
        NodeKind::BoolBranch { cond, then, els } => {
            if let Some(fix) = identical_bool_fix(node, then, els, source, lines, tokens, index, branches) {
                found.push(fix);
            }
            walk_node(cond, source, lines, tokens, index, branches, found);
            walk_node(then, source, lines, tokens, index, branches, found);
            walk_node(els, source, lines, tokens, index, branches, found);
        }
        NodeKind::PatternBranch { arms, fallback, .. } => {
            if let Some(fix) = catch_all_fallback_fix(arms, fallback.as_deref(), source, lines, tokens, index) {
                found.push(fix);
            }
            if let Some(fix) = catch_all_variable_fix(node, arms, fallback.as_deref(), source, lines, tokens, index, branches) {
                found.push(fix);
            }
            if let Some(fix) = boolean_branch_fix(node, arms, fallback.as_deref(), source, lines, tokens, index, branches) {
                found.push(fix);
            }
            // §57: 腕がすべて自分の pattern を組み直す分岐は scrutinee ちょうど。
            // この書き換えは位置を選ばない（式の値が変わらない）。
            if let Some(fix) = rewrap_if_fix(node, source, lines, tokens, index, branches) {
                found.push(fix);
            }
            for (patterns, _) in arms {
                for pattern in patterns {
                    walk_pattern(pattern, source, lines, tokens, index, found);
                }
            }
        }
        NodeKind::Fn { params, body } => {
            let arms = arm_form(params);
            if arms {
                if let Some(fix) =
                    rewrap_arm_fix(body, "fn(value => value)", Tail::Fn, source, lines, tokens, index, branches)
                {
                    found.push(fix);
                }
            }
            // `fn` の本体もその `fn` の末尾なので、伝播先はこの `fn`（§36）。
            let tail = if arms { Tail::Fn } else { Tail::If };
            if let Some(fix) = repropagate_fix(body, tail, None, source, lines, tokens, index, branches)
            {
                found.push(fix);
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
        walk_node(child, source, lines, tokens, index, branches, found);
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
