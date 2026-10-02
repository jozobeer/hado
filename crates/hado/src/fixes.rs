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
//! 組み直す分岐は scrutinee）、E508（`None` の再伝播は `e as p? ->`）。

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
    let mut found = Vec::new();
    collect_fixes(source, &ast, &mut found);
    prune_overlaps(found)
}

/// The rewrites a parsed source offers, in the order they were found (the
/// caller sorts and prunes them). The caller may supply the AST it already
/// parsed: §58.4 の診断の範囲は書き換えの範囲なので、CLI と LSP は自分の
/// 解析から範囲を引く（解析を 2 度行わない）。
fn collect_fixes(source: &str, ast: &parser::Ast, found: &mut Vec<Fix>) {
    let Ok(tokens) = lex_full(source) else {
        return;
    };
    let lines = LineMap::new(source);
    let index = TokenIndex::new(&tokens);
    let walk = Walk {
        source,
        lines: &lines,
        tokens: &tokens,
        index: &index,
        branches: &ast.branch_spans,
    };

    for span in &ast.long_condition_functions {
        if let Some(fix) = condition_arm_fix(source, &lines, &tokens, &index, ast, *span) {
            found.push(fix);
        }
    }
    for item in &ast.items {
        walk_item(item, &walk, found);
    }
}

/// The rewrites that survive the overlap rule: at the same start the outermost
/// one wins, so a `Cons` chain is rewritten whole. The caller may pass the
/// rewrites it already collected; this sorts them in place.
pub fn prune_overlaps(mut found: Vec<Fix>) -> Vec<Fix> {
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

/// The byte range a local rewrite (§57) replaces, when one starts at the
/// anchor. This is the diagnostic's end as well: §58.4 の終端は書き換える
/// 範囲の終端で、LSP の code action の range と同じものである。
pub fn fix_range(source: &str, code: &str, line: usize, col: usize) -> Option<(usize, usize)> {
    fix_at(source, code, line, col).map(|fix| (fix.start, fix.end))
}

/// The same range, taken from a project's own parse: the LSP checks a text that
/// is not on disk, so the rewrites must come from the same AST the diagnostic
/// came from rather than from reading the path back.
pub(crate) fn range_in(
    ast: &parser::Ast,
    source: &str,
    code: &str,
    line: usize,
    col: usize,
) -> Option<(usize, usize)> {
    let mut found = Vec::new();
    collect_fixes(source, ast, &mut found);
    prune_overlaps(found)
        .into_iter()
        .find(|fix| fix.code == code && fix.line == line && fix.col == col)
        .map(|fix| (fix.start, fix.end))
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
///
/// §58.1: 一括整形は書き換えの後に**全体を印字し直す**。`check --fix` は
/// 診断のアンカーに一致する書き換えだけを当てるので、ここを通らない。
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
    // §58.1: 印字し直して字下げと改行を正準形へ揃える。コメントは構文木が
    // 位置を持っているので消えない（§57）。
    let printed = match crate::parser::parse_source(&text, "<fmt>") {
        Ok(ast) => crate::parser::canonical_program(&ast),
        // 読めない入力は印字し直さない。呼ぶ側が検査で報告する。
        Err(_) => text.clone(),
    };
    // 行末の空白とファイル末尾の改行も揃える。
    let (laid_out, layout_applied) = layout(&printed);
    if laid_out != source {
        applied += 1;
    }
    applied += layout_applied;
    if laid_out == source {
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
    matches!(
        code,
        "E505"
            | "E501"
            | "E502"
            | "E503"
            | "E506"
            | "E511"
            | "E512"
            | "E507"
            | "E520"
            | "E508"
            | "E510"
            | "E515"
            | "E516"
            | "E517"
    )
}

// --- E505: 使われていない束縛を `_` にする ---

/// The checker reports an unused arm binding (E505) with the binding's span;
/// this renames exactly that span to `_`, which the checker exempts (check.rs).
/// The predicate is the checker's own (`names::bindings` / `names::references`),
/// so the rewrite cannot drift from the diagnostic.
fn unused_binding_fix(
    pattern: &Pattern,
    body: &Node,
    source: &str,
    lines: &LineMap,
) -> Option<Fix> {
    for (name, span) in crate::names::bindings(pattern) {
        if matches!(name, "_" | "$argument") || crate::names::references(body, name) {
            continue;
        }
        let start = lines.offset(source, span);
        let end = start + name.len();
        if end > source.len() || &source[start..end] != name {
            return None;
        }
        return Some(Fix {
            code: "E505",
            line: span.line,
            col: span.col,
            start,
            end,
            replacement: "_".into(),
            canonical: "_".into(),
        });
    }
    None
}

// --- E520: Bool リテラルの腕を Boolean 分岐へ ---

/// §24: `if b | true => x | false => y` と `if b | true => x | y` の canonical form は
/// `if b => x | y`。腕は真偽で選び直すので、書かれた順が逆でも値は変わらない。
/// 式の範囲は `branch_range` に頼るので、枝の位置にある形だけを扱う。
fn boolean_branch_fix(
    node: &Node,
    arms: &[(Vec<Pattern>, Node)],
    fallback: Option<&Node>,
    walk: &Walk<'_>,
) -> Option<Fix> {
    let Walk {
        source,
        lines,
        tokens,
        index,
        branches,
    } = *walk;
    let NodeKind::PatternBranch { scrutinees, .. } = &node.kind else {
        return None;
    };
    if scrutinees.len() != 1 {
        return None;
    }
    let literal = |patterns: &[Pattern]| -> Option<bool> {
        let [pattern] = patterns else { return None };
        let PatternKind::Literal(value) = &pattern.kind else {
            return None;
        };
        let NodeKind::Bool(value) = &value.kind else {
            return None;
        };
        Some(*value)
    };
    // 腕 2 本は真偽で 1 対 1、腕 1 本は fallback が残り半分。Bool の分岐は 2 本なので、
    // 腕 2 本と fallback が同居する形には正準形が無い。
    let (then, or_else) = match (arms, fallback) {
        ([(first, first_body), (second, second_body)], None) => {
            match (literal(first)?, literal(second)?) {
                (true, false) => (first_body, second_body),
                (false, true) => (second_body, first_body),
                _ => return None,
            }
        }
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
    // `if(...)` の形は節点の範囲が閉じ `)` まで伸びる。書き換えても閉じを保つ。
    let entry = branch_entry(node.span, branches)?;
    let parenthesized = lines.end_offset(source, node.span) > lines.end_offset(source, entry.2);
    let scrutinee = parser::canonical(&scrutinees[0]);
    let then = parser::canonical(then);
    let or_else = parser::canonical(or_else);
    let replacement = if parenthesized {
        format!("if({scrutinee} => {then} | {or_else})")
    } else {
        format!("if {scrutinee} => {then} | {or_else}")
    };
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
    walk: &Walk<'_>,
) -> Option<Fix> {
    let Walk {
        source,
        lines,
        tokens,
        index,
        branches,
    } = *walk;
    let (pattern, body) = catch_all_arm(arms, fallback)?;
    let scrutinee = catch_all_scrutinee(node)?;
    let start = lines.offset(source, pattern.span);
    let end = catch_all_range(start, node, source, lines, tokens, index, branches)?;
    let (binding, value) =
        catch_all_binding_value(pattern, scrutinee, start, source, lines, tokens, index)?;
    let replacement = substitute_binding(body, end, &binding, &value, walk)?;
    let scrutinee_text = parser::canonical(scrutinee);
    // The substituted form is still redundant when every arm rebuilds the
    // scrutinee (§41). The checker refuses to fold a fallback because it may
    // re-evaluate the scrutinee, but this scrutinee is a plain variable, so the
    // fold cannot change how often an effect runs. Each arm is checked here: a
    // constructor arm must return its own pattern, a variable arm the scrutinee.
    if arms_reconstruct(
        arms,
        &scrutinee_text,
        &replacement,
        source,
        lines,
        tokens,
        index,
    ) {
        // 畳めない事情（分岐の範囲が取れない・範囲にコメントがある）があれば、
        // 素の `| replacement` の修正へ落とす。
        if let Some(fix) = catch_all_folded_fix(node, pattern, scrutinee_text, walk) {
            return Some(fix);
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

/// `_` でない 1 つの名前を束ねる、既定の無い最後の腕。
fn catch_all_arm<'a>(
    arms: &'a [(Vec<Pattern>, Node)],
    fallback: Option<&Node>,
) -> Option<(&'a Pattern, &'a Node)> {
    let last = arms.last()?;
    if fallback.is_some() || last.0.len() != 1 {
        return None;
    }
    let pattern = &last.0[0];
    if !matches!(&pattern.kind, PatternKind::Name(name) if !crate::names::is_upper_reference(name))
    {
        return None;
    }
    Some((pattern, &last.1))
}

/// 分岐の scrutinee が 1 つの名前のとき。
fn catch_all_scrutinee(node: &Node) -> Option<&Node> {
    let NodeKind::PatternBranch { scrutinees, .. } = &node.kind else {
        return None;
    };
    let [scrutinee] = scrutinees.as_slice() else {
        return None;
    };
    let NodeKind::Name(_) = &scrutinee.kind else {
        return None;
    };
    Some(scrutinee)
}

/// 腕を書き換える範囲の終わり。既定の位置とコメントを確かめる。
fn catch_all_range(
    start: usize,
    node: &Node,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
    branches: &[(Span, Span, Span)],
) -> Option<usize> {
    let (_, end) = branch_range(node, Tail::If, source, lines, tokens, index, branches)?;
    // The arm forms (§31, §32) carry `$argument` as their scrutinee and their
    // branch spans the whole definition, so the range below would run past the
    // arm's own body. They have their own rewrites (E515/E517).
    if matches!(&node.kind, NodeKind::PatternBranch { scrutinees, .. }
        if scrutinees
            .iter()
            .any(|s| matches!(&s.kind, NodeKind::Name(name) if name == "$argument")))
    {
        return None;
    }
    // A comment would be lost, and an `as` binding could bind the same name again.
    if start >= end || source[start..end].contains("//") || source[start..end].contains(" as ") {
        return None;
    }
    Some(end)
}

/// 腕の `x =>` と scrutinee の綴りを取り出す（どちらも空白を落とす）。
fn catch_all_binding_value(
    pattern: &Pattern,
    scrutinee: &Node,
    start: usize,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
) -> Option<(String, String)> {
    let binding = source
        [start..lines.offset(source, tokens.get(index.position(pattern.span)? + 1)?.span)]
        .trim();
    let scrutinee_start = lines.offset(source, scrutinee.span);
    let value = source[scrutinee_start
        ..lines.offset(
            source,
            tokens.get(index.position(scrutinee.span)? + 1)?.span,
        )]
        .trim();
    if binding.is_empty() || value.is_empty() {
        return None;
    }
    Some((binding.to_string(), value.to_string()))
}

/// 腕の本体を書き写し、束縛と同じ綴りのトークンを scrutinee の値で置き換える。
/// 何も書かれていない本体は書き換えを出さない。
fn substitute_binding(
    body: &Node,
    end: usize,
    binding: &str,
    value: &str,
    walk: &Walk<'_>,
) -> Option<String> {
    let Walk {
        source,
        lines,
        tokens,
        index,
        ..
    } = *walk;
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
    Some(replacement)
}

/// §41: 分岐全体を scrutinee ちょうどに置き換える。腕ごとの判定は
/// `arms_reconstruct` が済ませている。
fn catch_all_folded_fix(
    node: &Node,
    pattern: &Pattern,
    scrutinee_text: String,
    walk: &Walk<'_>,
) -> Option<Fix> {
    let Walk {
        source,
        lines,
        tokens,
        index,
        branches,
    } = *walk;
    let (whole_start, whole_end) =
        branch_range(node, Tail::If, source, lines, tokens, index, branches)?;
    if whole_start >= whole_end || source[whole_start..whole_end].contains("//") {
        return None;
    }
    Some(Fix {
        code: "E512",
        line: pattern.span.line,
        col: pattern.span.col,
        start: whole_start,
        end: whole_end,
        replacement: scrutinee_text.clone(),
        canonical: scrutinee_text,
    })
}

/// すべての腕が自分の pattern を組み直しているか（§41 の畳み込みの条件）。
fn arms_reconstruct(
    arms: &[(Vec<Pattern>, Node)],
    scrutinee_text: &str,
    replacement: &str,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
) -> bool {
    if replacement.trim() != scrutinee_text.trim() {
        return false;
    }
    for (arm_patterns, arm_body) in &arms[..arms.len() - 1] {
        let [arm_pattern] = arm_patterns.as_slice() else {
            return false;
        };
        let Some(reconstructs) = arm_reconstructs(
            arm_pattern,
            arm_body,
            scrutinee_text,
            source,
            lines,
            tokens,
            index,
        ) else {
            return false;
        };
        if !reconstructs {
            return false;
        }
    }
    true
}

/// 1 つの腕が自分の pattern を組み直しているか。位置が引けなければ `None`。
fn arm_reconstructs(
    arm_pattern: &Pattern,
    arm_body: &Node,
    scrutinee_text: &str,
    source: &str,
    lines: &LineMap,
    tokens: &[Tok],
    index: &TokenIndex,
) -> Option<bool> {
    let at = index.position(arm_pattern.span)?;
    let next = tokens.get(at + 1)?;
    let pattern_text =
        source[lines.offset(source, arm_pattern.span)..lines.offset(source, next.span)].trim();
    let body_text = parser::canonical(arm_body);
    Some(match &arm_pattern.kind {
        PatternKind::Name(name) if crate::names::is_upper_reference(name) => {
            body_text.trim() == pattern_text
        }
        PatternKind::Name(_) | PatternKind::Underscore => body_text.trim() == scrutinee_text.trim(),
        _ => body_text.trim() == pattern_text,
    })
}

/// §42: when both results of a condition branch are the same, the condition is
/// redundant and the branch collapses to that one result. One range.
fn identical_bool_fix(node: &Node, then: &Node, els: &Node, walk: &Walk<'_>) -> Option<Fix> {
    let Walk {
        source,
        lines,
        tokens,
        index,
        branches,
    } = *walk;
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

/// §40: a plain binding that discards a pure value is redundant; the binding
/// disappears and the body stays. Only a bare name or a literal is provably
/// effect-free (E506 fires for effectful discards too, so anything else is left
/// for the author to rewrite as `e ->`).
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
    // 後置では値の後ろに ` as _ -> ` が続く。値そのものの綴りだけを見る。
    let text = source[value_start..lines.end_offset(source, value.span)].trim();
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

/// §41: `let x = v -> x` returns the bound value unchanged, so the binding is
/// redundant and the whole expression becomes that value.
fn identity_let_fix(
    node: &Node,
    pat: &Pattern,
    value: &Node,
    body: &Node,
    source: &str,
    lines: &LineMap,
) -> Option<Fix> {
    let bound = match &pat.kind {
        PatternKind::Name(name) => name.to_string(),
        _ => return None,
    };
    if bound.is_empty() || !bound.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    let start = lines.offset(source, node.span);
    let value_start = lines.offset(source, value.span);
    let body_start = lines.offset(source, body.span);
    if start >= body_start || value_start >= body_start {
        return None;
    }
    // §11: 後置では `e as p ->` が束縛の綴りなので、値の後ろの `as p ->` ごと
    // 落とす。値そのものを残す（`g 4 as x -> x` は `g 4` になる）。
    let head = source[value_start..body_start].trim_end();
    let text = head
        .strip_suffix("->")
        .map(str::trim_end)
        .and_then(|head| head.rsplit_once(" as "))
        .map_or_else(
            || head.strip_suffix("->").unwrap_or(head).trim().to_string(),
            |(value, _)| value.trim_end().to_string(),
        );
    if text.is_empty() {
        return None;
    }
    let body_end = body_start + bound.len();
    if body_end > source.len() || &source[body_start..body_end] != bound.as_str() {
        return None;
    }
    if parser::canonical(body) != bound {
        return None;
    }
    if source[start..body_end].contains("//") {
        return None;
    }
    Some(Fix {
        code: "E501",
        line: node.span.line,
        col: node.span.col,
        start,
        end: body_end,
        canonical: flatten(&text),
        replacement: text,
    })
}

fn literal_pattern_fix(
    node: &Node,
    arms: &[(Node, Node)],
    fallback: Option<&Node>,
    walk: &Walk<'_>,
) -> Option<Fix> {
    let Walk {
        source,
        lines,
        tokens,
        index,
        branches,
    } = *walk;
    if arms.is_empty() || fallback.is_none() {
        return None;
    }
    let (start, end) = branch_range(node, Tail::If, source, lines, tokens, index, branches)?;
    if start >= end || end > source.len() || source[start..end].contains("//") {
        return None;
    }
    let (name, cuts) = literal_arms(arms, start, end, source, lines)?;
    let first = cuts[0].0;
    let head = source[start..first].trim_end();
    let head = head.strip_suffix('|').map(|h| h.trim_end()).unwrap_or(head);
    if !head.starts_with("def ") {
        return None;
    }
    let prefix = literal_prefix(head, &name)?;
    if prefix.len() + 1 >= head.len() {
        return None;
    }
    let replacement = literal_replacement(&cuts, prefix, start + head.len(), source, end, first)?;
    Some(Fix {
        code: "E511",
        line: node.span.line,
        col: node.span.col,
        start,
        end,
        canonical: flatten(&replacement),
        replacement,
    })
}

/// 腕ごとの `x ==` の共通の左辺と、その演算子までの切り取り位置。
fn literal_arms(
    arms: &[(Node, Node)],
    start: usize,
    end: usize,
    source: &str,
    lines: &LineMap,
) -> Option<(String, Vec<(usize, usize)>)> {
    let mut name: Option<String> = None;
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for (cond, body) in arms {
        let cond_start = lines.offset(source, cond.span);
        let body_start = lines.offset(source, body.span);
        if cond_start < start || body_start <= cond_start || body_start > end {
            return None;
        }
        let segment = &source[cond_start..body_start];
        let eq = segment.find("==")?;
        let lhs = segment[..eq].trim();
        if lhs.is_empty() || !lhs.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        match &name {
            None => name = Some(lhs.to_string()),
            Some(seen) => {
                if seen != lhs {
                    return None;
                }
            }
        }
        let after = &segment[eq + 2..];
        let spaces = after.len() - after.trim_start().len();
        cuts.push((cond_start, cond_start + eq + 2 + spaces));
    }
    Some((name?, cuts))
}

/// 分岐の頭から共通の名前を落とした前置き。`def` の引数まで。
fn literal_prefix<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    match head.strip_suffix(name) {
        Some(prefix) if prefix.ends_with(|c: char| c.is_whitespace()) => Some(prefix.trim_end()),
        _ => None,
    }
}

/// 各腕の `x ==` を落とし、`def` の頭を最初の腕の行頭へ改めて置く。
fn literal_replacement(
    cuts: &[(usize, usize)],
    prefix: &str,
    head_end: usize,
    source: &str,
    end: usize,
    first: usize,
) -> Option<String> {
    let line_start = source[..first].rfind('\n').map(|i| i + 1).unwrap_or(0);
    // 正準形は `def` の見出しと最初の腕が同じ行に載る。そのとき行頭から腕までを
    // そのまま前置きにすると見出しが二重になるので、見出しの後ろだけを残す。
    let lead_start = line_start.max(head_end);
    let mut replacement = String::new();
    replacement.push_str(prefix);
    replacement.push('\n');
    replacement.push_str(&source[lead_start..first]);
    let mut cursor = first;
    for (cut_start, cut_end) in cuts {
        if *cut_start < cursor || *cut_end > end {
            return None;
        }
        replacement.push_str(&source[cursor..*cut_start]);
        cursor = *cut_end;
    }
    replacement.push_str(&source[cursor..end]);
    Some(replacement)
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
    let arrow = top_level_arrow(tokens, index.position(pattern.span)?)?;
    let mut end = lines.offset(source, tokens[arrow].span) + 2;
    // Leave the blank before `_` as the one space after `|`.
    end = skip_blanks(source, end);
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

/// 入れ子の外側にある最初の `=>` のトークン位置。
fn top_level_arrow(tokens: &[Tok], at: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut at = at;
    while let Some(token) = tokens.get(at) {
        match token.kind {
            TokKind::LParen | TokKind::LBracket | TokKind::LBrace => depth += 1,
            TokKind::RParen | TokKind::RBracket | TokKind::RBrace => depth -= 1,
            TokKind::Arrow if depth == 0 => return Some(at),
            TokKind::Eof => return None,
            _ => {}
        }
        at += 1;
    }
    None
}

/// 位置 `at` から続く空白（スペースとタブ）を飛ばした位置。
fn skip_blanks(source: &str, at: usize) -> usize {
    let mut at = at;
    while at < source.len() && matches!(source.as_bytes()[at], b' ' | b'\t') {
        at += 1;
    }
    at
}

/// 位置 `at` より前の空白（スペースとタブ）を落とした位置。
fn trim_blanks_back(source: &str, at: usize) -> usize {
    let mut at = at;
    while at > 0 && matches!(source.as_bytes()[at - 1], b' ' | b'\t') {
        at -= 1;
    }
    at
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
    let at = top_level_arrow(tokens, index.position(span)?)?;
    let if_token = tokens.get(at + 1)?;
    if !matches!(if_token.kind, TokKind::KwIf) {
        return None;
    }
    // Take the surrounding blanks too, so the rewrite leaves no trailing space.
    let start = trim_blanks_back(source, lines.offset(source, tokens[at].span));
    let end = skip_blanks(source, lines.offset(source, if_token.span) + 2);
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

fn def_at(items: &[Item], span: Span) -> Option<&parser::Def> {
    for item in items {
        match item {
            // The record names the definition by where it starts; §58.4 で範囲が
            // 伸びても、同じ定義を指す。
            Item::Def(def) if crate::check::same_start(def.span, span) => return Some(def),
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

fn walk_item(item: &Item, walk: &Walk<'_>, found: &mut Vec<Fix>) {
    let mut found = Found { fixes: found };
    walk_item_in(item, walk, &mut found);
}

/// `Walk` を組んだ後の本体。`Item` の種類ごとに、定義の腕と本体の再帰を分ける。
fn walk_item_in(item: &Item, walk: &Walk<'_>, found: &mut Found<'_>) {
    match item {
        Item::Def(def) => {
            found.push(sole_parameter_fix(
                def,
                walk.source,
                walk.lines,
                walk.tokens,
                walk.index,
            ));
            let arms = match &def.kind {
                DefKind::Function { params, .. } => arm_form(params),
                _ => false,
            };
            match &def.kind {
                DefKind::Value(body) => {
                    // 値の定義の本体は末尾なので、`as p? ->` の伝播先はこの定義。
                    found.push(repropagate_fix(body, Tail::If, None, walk));
                    walk_node(body, walk, found);
                }
                DefKind::Function { body, .. } => {
                    walk_def_function(def, body, arms, walk, found);
                    walk_node(body, walk, found);
                }
            }
        }
        Item::ModuleImpl(module) => {
            for item in &module.items {
                walk_item_in(item, walk, found);
            }
        }
        _ => {}
    }
}

/// `def` の関数の腕。腕の形なら恒等の書き換えを試す。
fn walk_def_function(
    def: &parser::Def,
    body: &Node,
    arms: bool,
    walk: &Walk<'_>,
    found: &mut Found<'_>,
) {
    if arms {
        let identity = format!(
            "def {}{} value => value",
            if def.rec { "rec " } else { "" },
            def.name
        );
        found.push(rewrap_arm_fix(body, &identity, Tail::Def, walk));
    }
    let tail = if arms { Tail::Def } else { Tail::If };
    found.push(repropagate_fix(body, tail, Some(def), walk));
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
    let DefKind::Function { params, ret, body } = &def.kind else {
        return None;
    };
    if !crate::check::sole_parameter_branch(params, ret.as_ref(), body) {
        return None;
    }
    let [parameter] = params.as_slice() else {
        return None;
    };
    let NodeKind::PatternBranch { scrutinees, .. } = &body.kind else {
        return None;
    };
    let [scrutinee] = scrutinees.as_slice() else {
        return None;
    };
    let canonical = parser::arm_function_source(def);
    // 型注釈の無い引数は括弧を取らない（`def f (x)` は E100）ので、綴りは名前だけ。
    let after_name = tokens.get(index.position(def.name_span)? + 1)?;
    if !matches!(&after_name.kind, TokKind::Name(name) if name == &parameter.name) {
        return None;
    }
    let mut start = lines.offset(source, after_name.span);
    let end = lines.offset(source, scrutinee.span) + parameter.name.len();
    // 空白も範囲に入れ、行末に空白を残さない。
    start = trim_blanks_back(source, start);
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
    let equals = let_identity_equals(tokens, at, bound)?;
    // 値の式は入れ子の `let` で `->` を持ち得るので、分離点は本体の直前の
    // `->` で決める（字句の走査では決まらない）。
    let body_at = index.position(body)?;
    let arrow = body_dash_arrow(tokens, body_at)?;
    let start = lines.offset(source, anchor);
    let value_start = lines.offset(source, tokens[equals].span) + 1;
    let value_end = lines.offset(source, tokens[arrow].span);
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

/// `let <bound> =` の綴りをトークンで確かめ、`=` の位置を返す。
/// 綴りが constructor のパターン（`Nil` は `[]` と同じ AST になる）は値の
/// 束縛ではないので、小文字で始まる名前だけを対象にする。値の綴りもトークンで
/// 確かめる（`[]` を `Nil` と見て 1 文字余分に食わないため）。
fn let_identity_equals(tokens: &[Tok], at: usize, bound: &str) -> Option<usize> {
    let_identity_head(tokens, at, bound)?;
    if !matches!(&tokens.get(at + 1)?.kind, TokKind::Name(name) if name == bound) {
        return None;
    }
    let equals = tokens.get(at + 2)?;
    if !matches!(equals.kind, TokKind::Eq) {
        return None;
    }
    Some(at + 2)
}

/// `let` と、値の束縛として書ける綴り（小文字で始まる名前）であること。
fn let_identity_head(tokens: &[Tok], at: usize, bound: &str) -> Option<()> {
    if !matches!(tokens.get(at)?.kind, TokKind::KwLet) {
        return None;
    }
    if !bound.starts_with(char::is_lowercase) {
        return None;
    }
    Some(())
}

/// 本体の直前の `->` のトークン位置。
fn body_dash_arrow(tokens: &[Tok], body_at: usize) -> Option<usize> {
    if !matches!(tokens.get(body_at - 1)?.kind, TokKind::DashArrow) {
        return None;
    }
    Some(body_at - 1)
}

// --- E507 / E508: Option の再包装と再伝播 ---

/// The shape of the function whose body is the branch (§57). A rewrite is only
/// offered where the branch is that function's body, so the surrounding
/// continuation does not change meaning.
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
fn rewrap_if_fix(node: &Node, walk: &Walk<'_>) -> Option<Fix> {
    let Walk {
        source,
        lines,
        tokens,
        index,
        branches,
    } = *walk;
    let scrutinee = crate::check::rewrap_branch(node)?;
    // The arm forms carry their own rewrite: their scrutinee is `$argument` and
    // the branch spans the whole definition.
    if matches!(&scrutinee.kind, NodeKind::Name(name) if name == "$argument") {
        return None;
    }
    let NodeKind::PatternBranch { fallback, .. } = &node.kind else {
        return None;
    };
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
fn rewrap_arm_fix(node: &Node, replacement: &str, tail: Tail, walk: &Walk<'_>) -> Option<Fix> {
    let Walk {
        source,
        lines,
        tokens,
        index,
        branches,
    } = *walk;
    crate::check::rewrap_branch(node)?;
    let NodeKind::PatternBranch { fallback, .. } = &node.kind else {
        return None;
    };
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
/// `E as p? -> body` (§57). The rewrite needs a binder that the body uses:
/// with none, or with one the body ignores, `E ->` is the canonical form.
fn repropagate_fix(
    node: &Node,
    tail: Tail,
    def: Option<&parser::Def>,
    walk: &Walk<'_>,
) -> Option<Fix> {
    let Walk { source, lines, .. } = *walk;
    let Repropagate {
        scrutinee,
        body,
        pattern,
        absence,
    } = repropagate_parts(node)?;
    // The rewritten definition names its argument `value`; a free `value` in
    // the body would be captured by that name.
    if tail != Tail::If && crate::names::references(body, "value") {
        return None;
    }
    // With every arm rebuilding, E507 is reported and its rewrite is shorter.
    if crate::check::rewrap_branch(node).is_some() {
        return None;
    }
    let (pipe, start, end, first_pipe) = repropagate_range(node, absence, tail, walk)?;
    // The scrutinee ends where the first arm's `|` begins; the first arm's body
    // ends where the `None` arm's `|` begins.
    let value_end = trim_back(source, lines.offset(source, first_pipe));
    let body_end = trim_back(source, lines.offset(source, pipe));
    let value = if tail == Tail::If {
        source[lines.offset(source, scrutinee.span)..value_end].trim()
    } else {
        "value"
    };
    let body_text = &source[lines.offset(source, body.span)..body_end];
    // §57: 正準形は `<scrutinee> as p? -> <body>`（後置の Option の伝播）。
    let binding = format!("{value} as {}? -> {body_text}", pattern_text(pattern));
    let replacement = repropagate_replacement(tail, def, binding)?;
    Some(Fix {
        code: "E508",
        line: pipe.line,
        col: pipe.col,
        start,
        end,
        canonical: flatten(&replacement),
        replacement,
    })
}

/// 再伝播の形（§36）から取り出した部品。
struct Repropagate<'a> {
    scrutinee: &'a Node,
    body: &'a Node,
    pattern: &'a Pattern,
    absence: &'a Pattern,
}

/// §36: `Some(p)` と `None` の 2 腕だけの分岐から、書き換えに要る部品を取り出す。
fn repropagate_parts(node: &Node) -> Option<Repropagate<'_>> {
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
    let [scrutinee] = scrutinees.as_slice() else {
        return None;
    };
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
    // `as p? -> E` matches `p` against the value inside the Option (§36), so the
    // first arm must be `Some(p)` with one argument.
    let PatternKind::Constructor(name, arguments) = &pattern.kind else {
        return None;
    };
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
    Some(Repropagate {
        scrutinee,
        body,
        pattern: inner,
        absence,
    })
}

/// 書き換える範囲と `None` 腕の `|`。既定の位置とコメントを確かめる。
fn repropagate_range(
    node: &Node,
    absence: &Pattern,
    tail: Tail,
    walk: &Walk<'_>,
) -> Option<(Span, usize, usize, Span)> {
    let Walk {
        source,
        lines,
        tokens,
        index,
        branches,
    } = *walk;
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
    Some((pipe.span, start, end, first_pipe))
}

/// 末尾の形ごとの書き換えテキスト。`as p? ->` の束縛を内側に置く。
fn repropagate_replacement(
    tail: Tail,
    def: Option<&parser::Def>,
    binding: String,
) -> Option<String> {
    Some(match tail {
        Tail::If => binding,
        Tail::Def => {
            let def = def?;
            format!(
                "def {}{} value => {binding}",
                if def.rec { "rec " } else { "" },
                def.name
            )
        }
        // §12.1: 本体に `->` があるので `fn ... /fn` で閉じる。
        Tail::Fn => format!("fn value => {binding} /fn"),
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
        // `after` is the branch's LAST token, so its own end is the end of the
        // branch. Taking only its start would leave that token outside the
        // rewrite (`if b | true => 1 | false => 0` lost its trailing `0`).
        let by_token = trim_back(source, lines.end_offset(source, after));
        // `if(...)` の閉じ `)` は最後のトークンの外側にある。parser が節点の範囲に
        // 入れた `)` の終端までを書き換えの範囲にする。
        by_token.max(lines.end_offset(source, node.span))
    };
    (start <= end).then_some((start, end))
}

/// The parser's record for one branch: its start, its first `|`, and the token
/// after it.
fn branch_entry(span: Span, branches: &[(Span, Span, Span)]) -> Option<&(Span, Span, Span)> {
    branches
        .iter()
        .find(|(start, _, _)| crate::check::same_start(*start, span))
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

/// 修正の組み立てに要る不変の文脈（ソース・行表・字句・位置表・分岐表）。走査も
/// 編集の組み立てもこの 5 つをそのまま引き回すので、1 つにまとめて引数の数を
/// 固定する。中身はすべて共有参照なので `Copy` で渡せる。
#[derive(Clone, Copy)]
struct Walk<'a> {
    source: &'a str,
    lines: &'a LineMap,
    tokens: &'a [Tok],
    index: &'a TokenIndex,
    branches: &'a [(Span, Span, Span)],
}

/// 見つかった修正の集積先。`Walk` は不変だが、こちらは走査のあいだ書き換わる。
struct Found<'a> {
    fixes: &'a mut Vec<Fix>,
}

impl Found<'_> {
    /// 修正が 1 つ得られたときだけ積む。`if let Some(fix) = ... { found.push(fix) }`
    /// の繰り返しを 1 か所に畳む。
    fn push(&mut self, fix: Option<Fix>) {
        if let Some(fix) = fix {
            self.fixes.push(fix);
        }
    }
}

/// `let` の腕。束縛の削除・恒等の検出と、`let x = ... x` の書き換えをまとめる。
fn walk_let(
    node: &Node,
    pat: &Pattern,
    value: &Node,
    body: &Node,
    walk: &Walk<'_>,
    found: &mut Found<'_>,
) {
    found.push(discard_plain_let_fix(
        node,
        pat,
        value,
        body,
        "（束縛を削除）",
        walk.source,
        walk.lines,
    ));
    found.push(identity_let_fix(
        node,
        pat,
        value,
        body,
        walk.source,
        walk.lines,
    ));
    walk_pattern(pat, walk, found);
    if let (PatternKind::Name(bound), NodeKind::Name(returned)) = (&pat.kind, &body.kind) {
        if bound == returned {
            found.push(let_identity_fix(
                node.span,
                bound,
                body.span,
                walk.source,
                walk.lines,
                walk.tokens,
                walk.index,
            ));
        }
    }
}

fn walk_node(node: &Node, walk: &Walk<'_>, found: &mut Found<'_>) {
    match &node.kind {
        NodeKind::Name(name) if name == "Nil" => walk_written_nil(node, walk, found),
        NodeKind::Call(callee, _) | NodeKind::Apply(callee, _) => {
            walk_written_cons(callee, node, walk, found)
        }
        NodeKind::Let {
            pat, value, body, ..
        } => walk_let(node, pat, value, body, walk, found),
        NodeKind::LetQ { pat, .. } => walk_let_q(pat, walk, found),
        NodeKind::BoolBranch { cond, then, els } => {
            walk_bool_branch(node, cond, then, els, walk, found)
        }
        NodeKind::CondBranch { arms, fallback, .. } => {
            walk_cond_branch(node, arms, fallback, walk, found)
        }
        NodeKind::PatternBranch { arms, fallback, .. } => {
            walk_pattern_branch(node, arms, fallback.as_deref(), walk, found)
        }
        NodeKind::Fn { params, body } => walk_fn(params, body, walk, found),
        _ => {}
    }
    walk_children(node, walk, found);
}

/// 綴りが `Nil` の名前。canonical な `[]` と同じ AST なので綴りを確かめる。
fn walk_written_nil(node: &Node, walk: &Walk<'_>, found: &mut Found<'_>) {
    if spelled(walk.source, walk.lines, node.span, "Nil") {
        push_edit(found.fixes, "E510", node.span, "[]", walk, 3);
    }
}

/// 呼び出しの綴りが `Cons` なら `[a, ...b]` へ。
fn walk_written_cons(callee: &Node, node: &Node, walk: &Walk<'_>, found: &mut Found<'_>) {
    if !matches!(&callee.kind, NodeKind::Name(name) if name == "Cons") {
        return;
    }
    if let Some(text) = list_expression(node) {
        push_cons_edit(found.fixes, "E510", node.span, &text, walk);
    }
}

/// `if` の腕。同じ値の分岐の検出と、条件・両腕の再帰。
fn walk_bool_branch(
    node: &Node,
    cond: &Node,
    then: &Node,
    els: &Node,
    walk: &Walk<'_>,
    found: &mut Found<'_>,
) {
    found.push(identical_bool_fix(node, then, els, walk));
    walk_node(cond, walk, found);
    walk_node(then, walk, found);
    walk_node(els, walk, found);
}

/// 子を順に辿る。書かれた `Cons(...)` の被演算子は呼び出し側で報告するので飛ばす。
fn walk_children(node: &Node, walk: &Walk<'_>, found: &mut Found<'_>) {
    let cons_call = matches!(&node.kind, NodeKind::Call(callee, _) | NodeKind::Apply(callee, _)
        if matches!(&callee.kind, NodeKind::Name(name) if name == "Cons"));
    for (position, child) in crate::check::children(node).into_iter().enumerate() {
        if cons_call && position == 0 {
            continue;
        }
        walk_node(child, walk, found);
    }
}

/// `if` の腕。条件式の書き換えと、各腕・既定の再帰。
fn walk_cond_branch(
    node: &Node,
    arms: &[(Node, Node)],
    fallback: &Option<Box<Node>>,
    walk: &Walk<'_>,
    found: &mut Found<'_>,
) {
    found.push(literal_pattern_fix(node, arms, fallback.as_deref(), walk));
    for (cond, body) in arms {
        walk_node(cond, walk, found);
        walk_node(body, walk, found);
    }
    if let Some(fallback) = fallback {
        walk_node(fallback, walk, found);
    }
}

/// `case` の腕。§57 の組み直しと、使われない束縛の検出をまとめる。
fn walk_pattern_branch(
    node: &Node,
    arms: &[(Vec<Pattern>, Node)],
    fallback: Option<&Node>,
    walk: &Walk<'_>,
    found: &mut Found<'_>,
) {
    found.push(catch_all_fallback_fix(
        arms,
        fallback,
        walk.source,
        walk.lines,
        walk.tokens,
        walk.index,
    ));
    found.push(catch_all_variable_fix(node, arms, fallback, walk));
    found.push(boolean_branch_fix(node, arms, fallback, walk));
    // §57: 腕がすべて自分の pattern を組み直す分岐は scrutinee ちょうど。
    // この書き換えは位置を選ばない（式の値が変わらない）。
    found.push(rewrap_if_fix(node, walk));
    for (patterns, body) in arms {
        for pattern in patterns {
            found.push(unused_binding_fix(pattern, body, walk.source, walk.lines));
            walk_pattern(pattern, walk, found);
        }
    }
}

/// `as p? ->` の腕。`?` の束縛そのものには局所的な書き換えが無いので、
/// pattern の中の書き換え（E516 など）だけを拾う。値と本体は `walk_children`
/// が `check::children` を通って見る。
fn walk_let_q(pat: &Pattern, walk: &Walk<'_>, found: &mut Found<'_>) {
    walk_pattern(pat, walk, found);
}

/// `fn` の腕。腕の形なら恒等の書き換えを試し、末尾の伝播先を決める。
fn walk_fn(params: &[parser::Param], body: &Node, walk: &Walk<'_>, found: &mut Found<'_>) {
    let arms = arm_form(params);
    if arms {
        found.push(rewrap_arm_fix(body, "fn(value => value)", Tail::Fn, walk));
    }
    // `fn` の本体もその `fn` の末尾なので、伝播先はこの `fn`（§36）。
    let tail = if arms { Tail::Fn } else { Tail::If };
    found.push(repropagate_fix(body, tail, None, walk));
}

fn walk_pattern(pattern: &Pattern, walk: &Walk<'_>, found: &mut Found<'_>) {
    match &pattern.kind {
        PatternKind::Name(name) if name == "Nil" => walk_written_nil_pattern(pattern, walk, found),
        PatternKind::Constructor(name, args) if name == "Cons" && args.len() == 2 => {
            walk_written_cons_pattern(pattern, walk, found)
        }
        PatternKind::Constructor(_, args) => walk_pattern_args(args, walk, found),
        PatternKind::Record(fields) | PatternKind::NamedConstructor(_, fields) => {
            walk_pattern_fields(fields, walk, found)
        }
        _ => {}
    }
}

/// 綴りが `Nil` のパターン。
fn walk_written_nil_pattern(pattern: &Pattern, walk: &Walk<'_>, found: &mut Found<'_>) {
    if spelled(walk.source, walk.lines, pattern.span, "Nil") {
        push_edit(found.fixes, "E516", pattern.span, "[]", walk, 3);
    }
}

/// 綴りが `Cons` のパターンを `[a, ...b]` へ。
fn walk_written_cons_pattern(pattern: &Pattern, walk: &Walk<'_>, found: &mut Found<'_>) {
    if let Some(text) = parser::list_pattern_source(pattern) {
        push_cons_edit(found.fixes, "E516", pattern.span, &text, walk);
    }
}

/// `Constructor` の引数を辿る。
fn walk_pattern_args(args: &[Pattern], walk: &Walk<'_>, found: &mut Found<'_>) {
    for arg in args {
        walk_pattern(arg, walk, found);
    }
}

/// `Record` / `NamedConstructor` のフィールドを辿る。
fn walk_pattern_fields(
    fields: &[(parser::DeclaredName, Option<Pattern>)],
    walk: &Walk<'_>,
    found: &mut Found<'_>,
) {
    for (_, field) in fields {
        if let Some(field) = field {
            walk_pattern(field, walk, found);
        }
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
    list_parts(&args[1], &mut parts);
    Some(format!("[{}]", parts.join(", ")))
}

/// `Cons` の連なりを `[a, b, ...rest]` の要素へ広げる。
fn list_parts(tail: &Node, parts: &mut Vec<String>) {
    let mut tail = tail;
    loop {
        match &tail.kind {
            NodeKind::Name(name) if name == "Nil" => return,
            NodeKind::Call(callee, args) | NodeKind::Apply(callee, args)
                if matches!(&callee.kind, NodeKind::Name(name) if name == "Cons")
                    && args.len() == 2 =>
            {
                parts.push(parser::canonical(&args[0]));
                tail = &args[1];
            }
            _ => {
                parts.push(format!("...{}", parser::canonical(tail)));
                return;
            }
        }
    }
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

fn push_edit(
    found: &mut Vec<Fix>,
    code: &'static str,
    anchor: Span,
    replacement: &str,
    walk: &Walk<'_>,
    length: usize,
) {
    let Walk { source, lines, .. } = *walk;
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

fn push_cons_edit(
    found: &mut Vec<Fix>,
    code: &'static str,
    anchor: Span,
    replacement: &str,
    walk: &Walk<'_>,
) {
    let Walk {
        source,
        lines,
        tokens,
        index,
        ..
    } = *walk;
    let Some(at) = index.position(anchor) else {
        return;
    };
    if !matches!(tokens.get(at).map(|tok| &tok.kind), Some(TokKind::Name(name)) if name == "Cons") {
        return;
    }
    let Some(open) = tokens.get(at + 1) else {
        return;
    };
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
    /// The byte offset just past `span` (its exclusive end), in the same line
    /// table. §58.4 の終端と同じく、対象の最後のスカラの次を指す。
    fn end_offset(&self, source: &str, span: Span) -> usize {
        let Some(&start) = self.starts.get(span.end_line.saturating_sub(1)) else {
            return source.len();
        };
        source[start..]
            .char_indices()
            .nth(span.end_col.saturating_sub(1))
            .map(|(at, _)| start + at)
            .unwrap_or(source.len())
    }

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
