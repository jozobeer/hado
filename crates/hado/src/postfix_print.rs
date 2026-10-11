//! §57・§58.1 の正準形の印字器。
//!
//! `e as p ->`・`e ->` の束縛と、`if` の閉じ（本体に `->` があれば `/if`、
//! 無ければ続きがあるときだけ `if(...)`）で、構文木を 1 通りに綴る。
//!
//! 綴りは構文木から決まる。`if` の本体（腕と fallback）に束縛が 1 つでも
//! あれば `/if` で閉じ、無ければ「続きがある位置」では `if(...)`、末尾では
//! 閉じない。`fn` も同じで、本体に束縛があれば `fn x => … /fn`、無ければ
//! 今の `fn(x => …)` のまま。`module` と `implement` は `/module`・
//! `/implement` で閉じ、`def` は閉じない。

use super::source_print::{literal, parameters, return_type, type_parameters, type_source};
use super::*;

/// 1 ファイル分の綴り。`library` 見出しのあとに項目を並べる。
///
/// §58.1: 字句が読み飛ばした行コメントは、構文木が持つ位置へ置き直す。
/// コメントは直後の項目に属し、その項目の見出しから行数分だけ上に来る
/// （§57 の「コメントを消さない」）。
pub fn program(ast: &Ast) -> String {
    let mut out = String::new();
    if ast.library.is_some() {
        out.push_str("library\n");
    }
    out.push_str(&items_text_with_breaks(&ast.items, &ast.comments));
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    place_comments(ast, out)
}

/// 項目を並べる。本体の中にコメントを持つ項目は、本体を次の行へ送る。
///
/// §57: コメントは同じ定義の中に残す。1 行に収まる本体にコメントがあると
/// その行の中に置けないので、`=>` の後ろで改行して本体を次の行へ置く。
fn items_text_with_breaks(items: &[Item], comments: &[crate::lexer::Comment]) -> String {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            // この項目の本体は、次の項目が始まる行で終わる。次の項目が
            // 無ければファイルの末尾まで。
            let next = items.get(index + 1).map_or(usize::MAX, item_start_line);
            item_text_with_breaks(item, comments, next)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `module` と `implement` の本体を閉じの印と一緒に組む。本体が空のときは空行を
/// 入れない。空行は §58.1 で残すので、空の本体の中の空行が本体の一部として読まれ、
/// 印字し直すたびに 1 つ増える。
fn closed_body(header: String, body: String, close: &str) -> String {
    if body.is_empty() {
        format!("{header}\n{close}")
    } else {
        format!("{header}\n{body}\n{close}")
    }
}

fn item_text_with_breaks(
    item: &Item,
    comments: &[crate::lexer::Comment],
    next_line: usize,
) -> String {
    match item {
        Item::ModuleSig(module) => closed_body(
            format!("module {}", module.name),
            items_text_with_breaks(&module.items, comments),
            "/module",
        ),
        Item::ModuleImpl(module) => match &module.form {
            ImplementationForm::Ordinary => closed_body(
                format!("implement {} as {}", module.sig, module.name),
                items_text_with_breaks(&module.items, comments),
                "/implement",
            ),
            ImplementationForm::Functor(parameters) => {
                let parameters = parameters
                    .iter()
                    .map(|parameter| format!("{}: {}", parameter.name, parameter.contract))
                    .collect::<Vec<_>>()
                    .join(", ");
                closed_body(
                    format!("implement {} as {}({parameters})", module.sig, module.name),
                    items_text_with_breaks(&module.items, comments),
                    "/implement",
                )
            }
            form => {
                let _ = form;
                item_text(item)
            }
        },
        Item::Def(def) => match body_break_line(def, comments, next_line) {
            // 本体の中にコメントがあるので、`=>` の後ろで改行する。
            Some(_) => split_body(item_text(item)),
            None => item_text(item),
        },
        _ => item_text(item),
    }
}

/// 本体の中にコメントを持つ `def` なら `true`。
///
/// 本体は見出しの行から本体の式の最終行まで。コメントがその範囲に入って
/// いれば本体の中である。`=>` の後ろで本体が次の行から始まるときのコメントは
/// 本体の先頭より前にあるが、同じ定義の中に残す（§58.1）。本体の終わりより
/// 後ろのコメント（同じ行の行末に付くもの）は本体の外である。範囲の外にある
/// コメント（次の項目に付くもの、次のまとまりの見出し）も本体の中ではない。
fn body_break_line(
    def: &Def,
    comments: &[crate::lexer::Comment],
    next_line: usize,
) -> Option<usize> {
    let DefKind::Function { body, .. } = &def.kind else {
        return None;
    };
    // 本体の最終行。見出しと同じ行を指すことがあるので、見出しの行で下限を取る。
    let last = body.span.end_line.max(def.span.line);
    // 本体の中のコメントは、見出しの行から本体の終わりまでの位置に入るもの。
    // ただし次の項目の直上（1 行上）にあるものは、その項目に付くので本体の
    // 中ではない。
    comments
        .iter()
        .any(|comment| {
            comment.span.line >= def.span.line
                && comment.span.line <= last
                && !after(comment.span, body.span)
                && comment.span.line + 1 != next_line
        })
        .then_some(def.span.line)
}

/// `position` が `end` より後ろか。同じ行なら列で比べる。
fn after(position: Span, end: Span) -> bool {
    position.line > end.end_line || (position.line == end.end_line && position.col > end.end_col)
}

/// `item` の印字を、本体が始まる `=>` で 2 行に割る。
fn split_body(text: String) -> String {
    // 印字は `def … => <本体>` の形なので、最初の ` => ` で割る。
    match text.find(" => ") {
        Some(at) => format!(
            "{}\n  {}",
            text[..at + 3].trim_end(),
            text[at + 4..].trim_start()
        ),
        None => text,
    }
}

/// コメントの 1 つを印字する行。行頭のコメントはそのまま、字下げの付いた
/// 位置のコメントは 2 の字下げを付ける（本体の内側のコメントを保つ）。
fn comment_line(comment: &crate::lexer::Comment) -> String {
    let text = comment.text.trim_end();
    if comment.span.col == 1 {
        text.to_string()
    } else {
        format!("  {text}")
    }
}

/// コメントを、属する項目の高さへ戻す。
///
/// 印字し直した行と元のソースの行を突き合わせるのではなく、**項目の印字が
/// 何行目から始まるかを積み上げて**求める。`items_text` は `module` の本体の
/// 入れ子項目も印字するので、印字の行数は `ast.items` の数より多い。項目の
/// 印字の行数をそのまま積めば、入れ子の分も自然に数えられる。
///
/// コメントが `module` や `implement` の本体の中にあるときは、**入れ子の
/// 項目**を基準にする。外側の項目で数えると、本体の中のコメントが本体の
/// 外へ出てしまう（§57 の「同じ定義の前後に残す」）。
fn place_comments(ast: &Ast, canonical: String) -> String {
    let mut lines: Vec<String> = canonical
        .trim_end_matches('\n')
        .split('\n')
        .map(str::to_string)
        .collect();
    // 入れ子の項目も含めた、各項目の印字が始まる行。
    // `library` 見出しの分だけ後ろへずれる。
    let items = ItemLines::new(&ast.items, usize::from(ast.library.is_some()));
    // 挿入するものを (印字の位置, 元のソースの行, 中身) で集める。
    let mut inserts: Vec<(usize, usize, Vec<String>)> = Vec::new();
    // §58.1: 空行は残す。印字し直した結果には空行が無いので、元のソースで
    // 空行が挟まっていたところへ入れ直す。空行は「その次に来るもの」の
    // 直前に付く。次に来るものは項目のことも、コメントの塊のこともある。
    // どちらの位置に置くかは、下のコメントの処理と突き合わせて決める。
    let blank_before = blank_runs(&ast.blank_lines);
    let blocks = comment_blocks(&ast.comments);
    // 空行は、その次に来るコメントの塊か項目の位置へ置く。
    for line in blank_before {
        if let Some(at) = blank_insert_position(line, ast, &items, lines.len()) {
            inserts.push((at, line, vec![String::new()]));
        }
    }
    for block in blocks {
        let first = block.first().expect("block is never empty");
        let at = comment_insert_position(first, &items, lines.len());
        let text: Vec<String> = block.iter().map(comment_line).collect();
        inserts.push((at.min(lines.len()), first.span.line, text));
    }
    insert_collected(&mut lines, inserts);
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// 空行の連なりを 1 つにまとめ、それぞれの先頭の行を返す。
fn blank_runs(blank_lines: &[usize]) -> Vec<usize> {
    let mut runs = Vec::new();
    for line in blank_lines {
        // 空行の次が空行のこともある。連なりは 1 つにまとめる。
        if runs.last().is_some_and(|prev| *prev + 1 == *line) {
            continue;
        }
        runs.push(*line);
    }
    runs
}

/// 連続するコメントを 1 つの塊にする。1 行ずつ置くと、同じ項目に付く
/// 複数行のコメントが互いの間に項目を挟んでしまう。塊は後ろから処理する。
fn comment_blocks(comments: &[crate::lexer::Comment]) -> Vec<Vec<crate::lexer::Comment>> {
    let mut blocks: Vec<Vec<crate::lexer::Comment>> = Vec::new();
    for comment in comments {
        match blocks.last_mut() {
            Some(block)
                if block
                    .last()
                    .is_some_and(|c| c.span.line + 1 == comment.span.line) =>
            {
                block.push(comment.clone());
            }
            _ => blocks.push(vec![comment.clone()]),
        }
    }
    blocks.reverse();
    blocks
}

/// 空行を入れ直す印字の位置。後ろに何も無い空行は置かない（`None`）。
/// 空の本体の中の空行などがこれに当たり、置くと本体の外（ファイルの末尾）へ
/// 出て、走らせるたびに増える。
fn blank_insert_position(
    line: usize,
    ast: &Ast,
    items: &ItemLines,
    height: usize,
) -> Option<usize> {
    // この空行より後ろで最初に始まるものの位置。
    let next_comment = ast
        .comments
        .iter()
        .find(|c| c.span.line > line)
        .map(|c| c.span.line);
    let next_item = items.flat.iter().find(|(source, _, _)| *source > line);
    let at = match (next_comment, next_item) {
        // 次が項目で、その項目の方がコメントより先なら、項目の位置。
        (Some(comment), Some((source, printed, _))) if comment >= *source => *printed,
        (Some(comment), _) => {
            let index = items.slot(comment)?;
            items.flat[index].1
        }
        (None, Some((_, printed, _))) => *printed,
        (None, None) => return None,
    };
    Some(at.min(height))
}

/// コメントの塊を入れ直す印字の位置。後ろに項目が無いときは、その本体を持つ
/// 項目の**中**の元の相対位置へ置く（§57 の「同じ定義の前後に残す」）。
/// どの項目より後ろなら、ファイルの末尾へ置く。
fn comment_insert_position(
    first: &crate::lexer::Comment,
    items: &ItemLines,
    height: usize,
) -> usize {
    match items.slot(first.span.line) {
        Some(index) => {
            let (_, printed, _) = items.flat[index];
            // 空行は入れ直すので、コメントの塊はその項目の直上に来る。
            printed
        }
        None => match items.enclosing(first.span.line) {
            Some(index) => {
                let (source, printed, item_height) = items.flat[index];
                // 見出しの行から何行目か。コメントは本体の中にあるので、
                // 見出しの次の行から数える。本体の外へは出さない。
                let offset = (first.span.line - source).min(item_height);
                printed + offset
            }
            None => height,
        },
    }
}

/// 集めた挿入を、印字の位置・次にソースの行の順に並べ、後ろから差し込む。
/// 後ろから入れると、前へ入れるときに後ろの位置がずれない。
fn insert_collected(lines: &mut Vec<String>, inserts: Vec<(usize, usize, Vec<String>)>) {
    let mut inserts = inserts;
    inserts.sort_by_key(|(at, source, _)| (*at, *source));
    for (at, _, chunk) in inserts.into_iter().rev() {
        let at = at.min(lines.len());
        for (offset, line) in chunk.into_iter().enumerate() {
            lines.insert((at + offset).min(lines.len()), line);
        }
    }
}

/// 入れ子の項目も含めた、各項目の印字開始行。
///
/// `items_text` は入れ子の項目も印字するので、平坦化した順が印字の順と
/// 一致する。外側の項目は自分の見出しの 1 行を占め、本体は入れ子の項目が
/// 占め、閉じの 1 行で終わる。
struct ItemLines {
    /// `(元のソースの開始行, 印字の開始行, 印字の行数)` を印字の順に並べたもの。
    flat: Vec<(usize, usize, usize)>,
}

impl ItemLines {
    fn new(items: &[Item], start: usize) -> Self {
        let mut flat = Vec::new();
        Self::walk(items, start, &mut flat);
        Self { flat }
    }

    /// `items` を平坦化し、次に使える印字行を返す。
    fn walk(items: &[Item], start: usize, flat: &mut Vec<(usize, usize, usize)>) -> usize {
        let mut cursor = start;
        for item in items {
            let here = flat.len();
            flat.push((item_start_line(item), cursor, 0));
            match item {
                // 見出しの次の行から本体が始まり、閉じの 1 行で終わる。
                Item::ModuleSig(module) => {
                    cursor = Self::walk(&module.items, cursor + 1, flat) + 1;
                }
                Item::ModuleImpl(module) => {
                    cursor = Self::walk(&module.items, cursor + 1, flat) + 1;
                }
                _ => cursor += item_text(item).lines().count(),
            }
            // 自分の見出しから、次の項目が始まるまでが自分の占める範囲。
            flat[here].2 = cursor - flat[here].1;
        }
        cursor
    }

    /// その行を含む項目（コメントより前に始まる最後の項目）。
    /// 本体の中のコメントを、その本体を持つ項目へ収めるのに使う。
    fn enclosing(&self, line: usize) -> Option<usize> {
        self.flat.iter().rposition(|(source, _, _)| *source <= line)
    }

    /// コメントが付く項目の位置。コメントより後ろで最初に始まる項目である。
    /// 後ろに項目が無ければ `None`（そのコメントより後ろに項目が無い）。
    fn slot(&self, line: usize) -> Option<usize> {
        self.flat.iter().position(|(source, _, _)| *source > line)
    }
}

/// 項目が始まる元のソースの行。
fn item_start_line(item: &Item) -> usize {
    item_span(item).line
}

fn item_span(item: &Item) -> &Span {
    match item {
        Item::Def(def) => &def.span,
        Item::SigDef(def) => &def.span,
        Item::TypeDef(def) => &def.span,
        Item::ModuleSig(module) => &module.span,
        Item::ModuleImpl(module) => &module.span,
        Item::Effect(effect) => &effect.span,
        Item::Handler(handler) => &handler.span,
    }
}

/// 式 1 つを、末尾の位置として 1 行で綴る。正準形の案内（`canonical form:`）に使う。
pub fn node_text(node: &Node) -> String {
    expr(node, 2, true).replace('\n', " ")
}

/// 構文木の中に `->` の束縛（`Let`・`LetQ`・`DoQ`）があるか。
pub(crate) fn contains_continuation(node: &Node) -> bool {
    match &node.kind {
        NodeKind::Let { .. } | NodeKind::LetQ { .. } | NodeKind::DoQ { .. } => true,
        NodeKind::Neg(c) | NodeKind::Not(c) => contains_continuation(c),
        NodeKind::Bin(_, l, r) | NodeKind::Pipe(l, r) => {
            contains_continuation(l) || contains_continuation(r)
        }
        NodeKind::Apply(f, args) | NodeKind::Call(f, args) => {
            contains_continuation(f) || args.iter().any(contains_continuation)
        }
        NodeKind::Field(l, _) => contains_continuation(l),
        NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) => {
            fields.iter().any(|(_, value)| contains_continuation(value))
        }
        NodeKind::List(items, rest) => {
            items.iter().any(contains_continuation)
                || rest.as_deref().is_some_and(contains_continuation)
        }
        NodeKind::Fn { body, .. } | NodeKind::With { body, .. } => contains_continuation(body),
        NodeKind::BoolBranch { then, els, .. } => {
            contains_continuation(then) || contains_continuation(els)
        }
        NodeKind::CondBranch { arms, fallback } => {
            arms.iter().any(|(_, body)| contains_continuation(body))
                || fallback.as_deref().is_some_and(contains_continuation)
        }
        NodeKind::PatternBranch { arms, fallback, .. } => {
            arms.iter().any(|(_, body)| contains_continuation(body))
                || fallback.as_deref().is_some_and(contains_continuation)
        }
        _ => false,
    }
}

/// `if` の本体（腕と fallback）に `->` があるか。Issue の規則の判定そのもの。
pub(crate) fn branch_has_arrow(node: &Node) -> bool {
    match &node.kind {
        NodeKind::BoolBranch { then, els, .. } => {
            contains_continuation(then) || contains_continuation(els)
        }
        NodeKind::CondBranch { arms, fallback } => {
            arms.iter().any(|(_, body)| contains_continuation(body))
                || fallback.as_deref().is_some_and(contains_continuation)
        }
        NodeKind::PatternBranch { arms, fallback, .. } => {
            arms.iter().any(|(_, body)| contains_continuation(body))
                || fallback.as_deref().is_some_and(contains_continuation)
        }
        _ => contains_continuation(node),
    }
}

fn items_text(items: &[Item]) -> String {
    items.iter().map(item_text).collect::<Vec<_>>().join("\n")
}

pub(crate) fn item_text(item: &Item) -> String {
    match item {
        Item::Def(def) => def_text(def),
        Item::SigDef(def) => format!(
            "def {}{}{}{}",
            type_parameters(&def.type_params),
            def.name,
            parameters(&def.params),
            return_type(def.ret.as_ref())
        ),
        Item::TypeDef(def) => {
            let body = def
                .ty
                .as_ref()
                .map(|ty| format!(" = {}", type_source(ty)))
                .unwrap_or_default();
            let shared = if def.shared { "shared " } else { "" };
            format!(
                "{shared}type {}{}{body}",
                type_parameters(&def.params),
                def.name
            )
        }
        Item::ModuleSig(module) => format!(
            "module {}\n{}\n/module",
            module.name,
            items_text(&module.items)
        ),
        Item::ModuleImpl(module) => implementation_text(module),
        Item::Effect(effect) => effect_text(effect),
        Item::Handler(handler) => handler_text(handler),
    }
}

fn def_text(def: &Def) -> String {
    let rec = if def.rec { "rec " } else { "" };
    let tail = match &def.kind {
        DefKind::Value(body) => format!(" = {}", expr(body, 2, true)),
        DefKind::Function { params, ret, body } => {
            if let NodeKind::CondBranch { arms, fallback } = &body.kind {
                format!(
                    "{}{}{}",
                    parameters(params),
                    return_type(ret.as_ref()),
                    cond_arms(arms, fallback.as_deref(), 2, true)
                )
            } else if is_arm_function(params, body) {
                arm_arms(body, 2, true)
            } else {
                format!(
                    "{}{} => {}",
                    parameters(params),
                    return_type(ret.as_ref()),
                    expr(body, 2, true)
                )
            }
        }
    };
    format!("def {rec}{}{tail}", def.name)
}

fn is_arm_function(params: &[Param], body: &Node) -> bool {
    params.len() == 1
        && params[0].name == "$argument"
        && matches!(body.kind, NodeKind::PatternBranch { .. })
}

/// `def f | …` と `fn(| …)` の腕の並び。scrutinee は書かない。
fn arm_arms(body: &Node, indent: usize, tail: bool) -> String {
    let NodeKind::PatternBranch { arms, fallback, .. } = &body.kind else {
        return expr(body, indent, tail);
    };
    pattern_arms(arms, fallback.as_deref(), indent, tail)
}

fn pattern_arms(
    arms: &[(Vec<Pattern>, Node)],
    fallback: Option<&Node>,
    indent: usize,
    tail: bool,
) -> String {
    let mut out = String::new();
    for (patterns, value) in arms {
        out.push_str(&format!(
            " | {} => {}",
            comma_patterns(patterns),
            arm_value(value, indent, tail)
        ));
    }
    if let Some(fallback) = fallback {
        out.push_str(&format!(" | {}", arm_value(fallback, indent, tail)));
    }
    out
}

/// 腕の本体。本体が `->` を含むなら括弧で包む。包まないと、後ろに続く
/// `| 次の腕` が本体の内側の分岐の腕として読まれる（`| A => 2 as y -> y | B => 3`
/// の `| B` は内側の fallback になる）。
fn arm_value(value: &Node, indent: usize, tail: bool) -> String {
    let text = expr(value, indent, tail);
    if contains_continuation(value) {
        format!("({text})")
    } else {
        text
    }
}

fn cond_arms(arms: &[(Node, Node)], fallback: Option<&Node>, indent: usize, tail: bool) -> String {
    let mut out = String::new();
    for (condition, value) in arms {
        out.push_str(&format!(
            " | {} => {}",
            condition_text(condition, indent),
            expr(value, indent, tail)
        ));
    }
    if let Some(fallback) = fallback {
        out.push_str(&format!(" | {}", expr(fallback, indent, tail)));
    }
    out
}

fn implementation_text(module: &ModuleImpl) -> String {
    let header = format!("implement {} as {}", module.sig, module.name);
    match &module.form {
        ImplementationForm::Ordinary => {
            closed_body(header, items_text(&module.items), "/implement")
        }
        ImplementationForm::Functor(parameters) => {
            let parameters = parameters
                .iter()
                .map(|parameter| format!("{}: {}", parameter.name, parameter.contract))
                .collect::<Vec<_>>()
                .join(", ");
            closed_body(
                format!("{header}({parameters})"),
                items_text(&module.items),
                "/implement",
            )
        }
        ImplementationForm::Application { functor, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{header} = {}({arguments})", functor.name)
        }
    }
}

fn effect_text(effect: &EffectDecl) -> String {
    let ops = effect
        .ops
        .iter()
        .map(effect_op_text)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "effect {}{}{}",
        type_parameters(&effect.type_params),
        effect.name,
        if ops.is_empty() {
            String::new()
        } else {
            format!("\n{ops}")
        }
    )
}

fn effect_op_text(op: &EffectOp) -> String {
    let params = if op.params.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            op.params
                .iter()
                .map(|param| {
                    format!(
                        "{}: {}",
                        param.name,
                        type_source(param.ty.as_ref().expect("effect parameter"))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    format!(
        "{} {}{}{}",
        op.kind.as_str(),
        op.name,
        params,
        return_type(op.ret.as_ref())
    )
}

fn handler_text(handler: &HandlerDecl) -> String {
    let params = if handler.params.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            handler
                .params
                .iter()
                .map(|param| {
                    format!(
                        "{}: {}",
                        param.name,
                        type_source(param.ty.as_ref().expect("handler parameter"))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let arms = handler
        .arms
        .iter()
        .map(handler_arm_text)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "handler {}{} as {}{}{}",
        type_parameters(&handler.type_params),
        handler.effect,
        handler.name,
        params,
        if arms.is_empty() {
            String::new()
        } else {
            format!("\n{arms}")
        }
    )
}

fn handler_arm_text(arm: &HandlerArm) -> String {
    match arm {
        HandlerArm::Operation {
            kind,
            name,
            params,
            body,
            ..
        } => {
            let params = params
                .iter()
                .map(|param| format!(" {}", param.name))
                .collect::<Vec<_>>()
                .join("");
            format!(
                "{} {}{} => {}",
                kind.as_str(),
                name,
                params,
                expr(body, 2, true)
            )
        }
        HandlerArm::Return { param, body, .. } => {
            format!("return {} => {}", param.name, expr(body, 2, true))
        }
    }
}

/// `tail` は、その位置に続きが無いこと（束縛や分岐を閉じずに置けること）。
fn expr(node: &Node, indent: usize, tail: bool) -> String {
    expr_prec(node, indent, tail, 0)
}

/// 演算子の強さ（§7）。大きいほど強く結合する。
fn bin_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 2,
        BinOp::And => 3,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne => 4,
        BinOp::Add | BinOp::Sub => 5,
        BinOp::Mul | BinOp::Div | BinOp::Mod => 6,
    }
}

fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne
    )
}

/// `min` より弱い演算子は括弧で包む。
fn expr_prec(node: &Node, indent: usize, tail: bool, min: u8) -> String {
    match &node.kind {
        NodeKind::Int(n) => n.to_string(),
        NodeKind::Float(x) => float_text(*x),
        NodeKind::Char(c) => literal(&c.to_string(), '\''),
        NodeKind::Str(s) => literal(s, '"'),
        NodeKind::Bool(b) => b.to_string(),
        NodeKind::Void => "()".to_string(),
        NodeKind::Name(name) => name.clone(),
        // `- - x` を `--x` と綴ると、字句が `--` をコメントの印と読んで E104。
        // 入れ子の `-` の被演算子は括弧で包む（§7）。
        NodeKind::Neg(c) => neg_text(c, indent, min, tail),
        NodeKind::Not(c) => wrap(
            format!(
                "not {}",
                operand_text(c, indent, enclosed_by_wrap(tail, 7, min), 7)
            ),
            7,
            min,
        ),
        NodeKind::Bin(op, l, r) => bin_text(*op, l, r, indent, min, tail),
        NodeKind::Pipe(l, r) => wrap(
            format!(
                "{} |> {}",
                expr_prec(l, indent, false, 1),
                operand_text(r, indent, enclosed_by_wrap(tail, 1, min), 2)
            ),
            1,
            min,
        ),
        NodeKind::Apply(f, args) => apply_text(f, args, indent, tail),
        NodeKind::Call(f, args) => call_text(f, args, indent),
        NodeKind::Field(l, r) => field_text(l, r, indent),
        NodeKind::Record(fields) => format!("{{ {} }}", fields_text(fields, indent)),
        NodeKind::NamedConstructor(name, fields) => {
            format!("{} {{ {} }}", name.name, fields_text(fields, indent))
        }
        NodeKind::List(items, rest) => list_text(items, rest.as_deref(), indent),
        NodeKind::Let { pat, value, body } => {
            binding(format!(" as {pat}"), value, body, indent, tail)
        }
        NodeKind::LetQ { pat, value, body } => {
            binding(format!(" as {pat}?"), value, body, indent, tail)
        }
        NodeKind::DoQ { value, body } => binding(String::new(), value, body, indent, tail),
        NodeKind::Fn { params, body } => fn_text(params, body, indent),
        NodeKind::BoolBranch { .. }
        | NodeKind::CondBranch { .. }
        | NodeKind::PatternBranch { .. } => branch_text(node, indent, tail),
        NodeKind::With {
            handler,
            args,
            body,
        } => with_text(handler, args, body, indent, tail),
        NodeKind::Resume => "resume".to_string(),
    }
}

/// `- - x` を `--x` と綴ると、字句が `--` をコメントの印と読んで E104。
/// 入れ子の `-` の被演算子は括弧で包む（§7）。
fn neg_text(c: &Node, indent: usize, min: u8, tail: bool) -> String {
    // 入れ子の `-` は `wrap` を通さず自分で `-(…)` と綴る（`--` がコメントの
    // 印になるのを避けるため、§7）。この `)` も §12.1 の閉じを兼ねるので、
    // 中身の右端は入力の終わりと同じ「続きの無い位置」になる。
    let nested = matches!(c.kind, NodeKind::Neg(_));
    let inner = operand_text(c, indent, nested || enclosed_by_wrap(tail, 7, min), 7);
    let text = if nested {
        format!("-({inner})")
    } else {
        format!("-{inner}")
    };
    wrap(text, 7, min)
}

fn bin_text(op: BinOp, l: &Node, r: &Node, indent: usize, min: u8, tail: bool) -> String {
    let prec = bin_prec(op);
    let (left_min, right_min) = if is_comparison(op) {
        (prec + 1, prec + 1)
    } else {
        (prec, prec + 1)
    };
    wrap(
        format!(
            "{} {} {}",
            operand_text(l, indent, false, left_min),
            op.as_str(),
            operand_text(r, indent, enclosed_by_wrap(tail, prec, min), right_min)
        ),
        prec,
        min,
    )
}

/// 演算子の被演算子（二項演算子の両辺、単項演算子の被演算子、`|>` の右辺）と
/// 並置の引数に置く式。`tail` は、その位置に続きが無いこと。
///
/// 続きの無い位置の分岐は裸でも `if(...)` でも置けない。裸の `if …` は
/// `prefix_form` を取り得ない位置（§59 の `unary`・`application`）に置けず
/// （`expected an expression, found if`）、`if(...)` は閉じの `)` の直後に
/// 閉じ・区切り・入力の終わりが来ると E100 になる（§12.1）。どちらでも通る
/// `(if …)` だけがこの位置の綴りなので、明示の括弧で包み、閉じは `)` に任せる。
///
/// 続きの有無は右端の被演算子がそのまま引き継ぐ。引き継ぐのは演算子だけで、
/// 束縛や `with` は引き継がない（裸で被演算子に置けず、括弧が要るため）。
/// 演算子が `wrap` の括弧で包まれるときは、その中の右端も続きが無い
/// （`enclosed_by_wrap`）。親から見て続きがあっても、包まれた中には無い。
fn operand_text(node: &Node, indent: usize, tail: bool, min: u8) -> String {
    match &node.kind {
        NodeKind::Bin(..)
        | NodeKind::Neg(..)
        | NodeKind::Not(..)
        | NodeKind::Pipe(..)
        | NodeKind::Apply(..) => expr_prec(node, indent, tail, min),
        _ if tail && is_branch(node) => format!("({})", enclosed(node, indent)),
        _ => expr_prec(node, indent, false, min),
    }
}

fn apply_text(f: &Node, args: &[Node], indent: usize, tail: bool) -> String {
    // 呼び手が適用のときは括弧で囲む。`(f a) b` を `f a b` と印字すると、2 回の
    // 適用が 1 回の 2 引数呼び出しに変わり、意味が変わる（§36）。括弧の中は
    // 入力の終わりと同じなので、右端の被演算子は続きの無い位置として印字する。
    let mut out = if matches!(f.kind, NodeKind::Apply(..)) {
        format!("({})", expr(f, indent, true))
    } else {
        expr_prec(f, indent, false, 8)
    };
    for (index, argument) in args.iter().enumerate() {
        out.push(' ');
        let last = index + 1 == args.len();
        out.push_str(&apply_argument(argument, indent, last, tail));
    }
    out
}

fn is_branch(node: &Node) -> bool {
    matches!(
        node.kind,
        NodeKind::BoolBranch { .. } | NodeKind::CondBranch { .. } | NodeKind::PatternBranch { .. }
    )
}

/// 適用の引数 1 つを印字する。適用の引数と大文字の名前参照は括弧で包む。
/// 包まないと `(f a) b` が `f a b` になり、適用の回数が変わる（§36）。
/// 括弧の中は入力の終わりと同じなので、右端の被演算子は続きの無い位置として
/// 印字する。最後の引数の後ろに続きが無ければ、その引数自身も続きが無い。
fn apply_argument(argument: &Node, indent: usize, last: bool, tail: bool) -> String {
    if matches!(argument.kind, NodeKind::Apply(..))
        || matches!(&argument.kind, NodeKind::Name(name) if crate::names::is_upper_reference(name))
    {
        format!("({})", expr(argument, indent, true))
    } else if last && is_branch(argument) {
        // 並置の最後の引数の後ろには続きが無い。`if(...)` は続きが無いと
        // E100。括弧で囲み、閉じは `)` に任せる（§12.1）。並置の `f if …`
        // も E100 なので、括弧は外さない。
        format!("({})", enclosed(argument, indent))
    } else {
        // 最後の引数は、適用の後ろに続きが無ければ「続きの無い位置」である。
        // 引数の中の二項演算子・単項演算子の右端もそこに当たる。
        operand_text(argument, indent, last && tail, 9)
    }
}

fn call_text(f: &Node, args: &[Node], indent: usize) -> String {
    format!(
        "{}({})",
        expr_prec(f, indent, false, 9),
        args.iter()
            .map(|a| enclosed(a, indent))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn field_text(l: &Node, r: &FieldName, indent: usize) -> String {
    // `.` は適用より強い（§7）。base が適用だと括弧が無く `f a.b` になり、
    // `.` が最後の引数に付く。`1.a` は浮動小数の字句（E103）にも読まれる。
    // 括弧の中は入力の終わりと同じなので、右端は続きの無い位置として印字する。
    let base = if matches!(
        l.kind,
        NodeKind::Int(_) | NodeKind::Apply(..) | NodeKind::Call(..)
    ) {
        format!("({})", expr(l, indent, true))
    } else {
        expr_prec(l, indent, false, 9)
    };
    format!("{base}.{}", r.name)
}

fn list_text(items: &[Node], rest: Option<&Node>, indent: usize) -> String {
    let mut parts = items
        .iter()
        .map(|item| enclosed(item, indent))
        .collect::<Vec<_>>();
    if let Some(rest) = rest {
        parts.push(format!("...{}", enclosed(rest, indent)));
    }
    format!("[{}]", parts.join(", "))
}

fn with_text(
    handler: &DeclaredName,
    args: &[Node],
    body: &Node,
    indent: usize,
    tail: bool,
) -> String {
    let text = format!(
        "with {}{} -> {}",
        handler.name,
        with_args(args, indent),
        expr(body, indent, true)
    );
    if tail {
        text
    } else {
        format!("({text})")
    }
}

/// 区切り（`,`・`;`）か閉じ括弧（`)`・`]`・`}`）が後ろに来る位置。そこで中の構文も
/// 閉じるので、末尾と同じく印を付けない。
/// 区切りの前の式。`if` は閉じの印を付けず、区切り（`,`・`)` など）に閉じさせる。
/// `fn` は常に閉じるので、区切りの前でも閉じた綴りになる。
fn enclosed(node: &Node, indent: usize) -> String {
    expr(node, indent, true)
}

/// 並置の引数に置く式。区切りの前と同じ扱いにする（§12.1）。
fn single_argument(node: &Node, indent: usize) -> String {
    match &node.kind {
        NodeKind::BoolBranch { .. }
        | NodeKind::CondBranch { .. }
        | NodeKind::PatternBranch { .. } => enclosed(node, indent),
        _ => operand_text(node, indent, true, 0),
    }
}

/// `wrap` が括弧を足す位置（`prec < min`）では、その `)` が §12.1 の閉じを
/// 兼ねるので、中身の右端は入力の終わりと同じ「続きの無い位置」になる。
/// 外から来た `tail` に、包まれた分を足して返す。
fn enclosed_by_wrap(tail: bool, prec: u8, min: u8) -> bool {
    tail || prec < min
}

fn wrap(text: String, prec: u8, min: u8) -> String {
    if prec < min {
        format!("({text})")
    } else {
        text
    }
}

fn binding(suffix: String, value: &Node, body: &Node, indent: usize, tail: bool) -> String {
    let head = format!("{}{suffix} ->", expr(value, indent, false));
    let pad = " ".repeat(indent);
    let body = expr(body, indent, true);
    if tail {
        format!("{head}\n{pad}{body}")
    } else {
        format!("({head}\n{pad}{body})")
    }
}

fn branch_text(node: &Node, indent: usize, tail: bool) -> String {
    let inner = branch_inner(node, indent);
    let pad = " ".repeat(indent);
    if branch_has_arrow(node) {
        if tail {
            format!("if {inner}\n{pad}/if")
        } else {
            format!("(if {inner}\n{pad}/if)")
        }
    } else if tail {
        format!("if {inner}")
    } else {
        format!("if({inner})")
    }
}

fn branch_inner(node: &Node, indent: usize) -> String {
    match &node.kind {
        NodeKind::BoolBranch { cond, then, els } => format!(
            "{} => {} | {}",
            condition_text(cond, indent),
            expr(then, indent, true),
            expr(els, indent, true)
        ),
        NodeKind::CondBranch { arms, fallback } => {
            cond_arms(arms, fallback.as_deref(), indent, true)
        }
        NodeKind::PatternBranch {
            scrutinees,
            arms,
            fallback,
        } => {
            let mut out = scrutinees
                .iter()
                .map(|scrutinee| condition_text(scrutinee, indent))
                .collect::<Vec<_>>()
                .join(", ");
            for (patterns, body) in arms {
                out.push_str(&format!(
                    " | {} => {}",
                    comma_patterns(patterns),
                    expr(body, indent, true)
                ));
            }
            if let Some(fallback) = fallback {
                out.push_str(&format!(" | {}", expr(fallback, indent, true)));
            }
            out
        }
        _ => expr(node, indent, false),
    }
}

/// `if` の条件と scrutinee。先頭が `(` でも、`)` までの最上位に `=>` が無いので
/// `if(` の閉じの形とは読まれない（パーサの `if_paren_closes`）。
fn condition_text(node: &Node, indent: usize) -> String {
    expr(node, indent, false)
}

fn fn_text(params: &[Param], body: &Node, indent: usize) -> String {
    if let NodeKind::CondBranch { arms, fallback } = &body.kind {
        return format!(
            "fn({}{})",
            lambda_parameters(params),
            cond_arms(arms, fallback.as_deref(), indent + 2, false)
        );
    }
    if is_arm_function(params, body) {
        return format!("fn({})", arm_arms(body, indent + 2, false));
    }
    if contains_continuation(body) {
        let pad = " ".repeat(indent);
        format!(
            "fn {} =>\n{}{}\n{pad}/fn",
            lambda_parameters(params),
            " ".repeat(indent + 2),
            expr(body, indent + 2, true)
        )
    } else {
        format!(
            "fn({} => {})",
            lambda_parameters(params),
            expr(body, indent, true)
        )
    }
}

fn lambda_parameters(params: &[Param]) -> String {
    params
        .iter()
        .map(|param| match &param.ty {
            Some(ty) => format!("{}: {}", param.name, type_source(ty)),
            None => param.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn with_args(args: &[Node], indent: usize) -> String {
    if args.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            args.iter()
                .map(|arg| single_argument(arg, indent))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn fields_text(fields: &[(DeclaredName, Node)], indent: usize) -> String {
    fields
        .iter()
        .map(|(field, value)| format!("{} = {}", field.name, enclosed(value, indent)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn comma_patterns(patterns: &[Pattern]) -> String {
    patterns
        .iter()
        .map(|pattern| pattern.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 整数値でも `1.0` のように浮動小数の見た目を保つ。
fn float_text(x: f64) -> String {
    let text = x.to_string();
    if text.contains('.') {
        text
    } else {
        format!("{text}.0")
    }
}
