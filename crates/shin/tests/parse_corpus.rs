//! R4 acceptance harness (PLAN.md "判定" and "コーパス外プローブ").
//!
//! 1. `expression_tree_round_trip`: generate 20 expression trees with a
//!    fixed-seed LCG, render each to a source string with randomly mixed
//!    redundant parentheses / whitespace / `--` line comments, parse it as
//!    `def main = <source>`, and require the fully-parenthesized canonical
//!    form of the parsed tree to equal the canonical form built directly
//!    from the generated tree.
//! 2. `corpus_counts`: enumerate `tests/accept` and `tests/reject` with
//!    `read_dir`, print the accept / E1xx / non-E1xx contract lines, and
//!    check the parse/compile divergence (non-E1xx fail only at the
//!    non-syntax `M3+` stage; E1xx fail at parse).
//! 3. `gen_probe`: five `def main = <a> + <b>` pairs through the production
//!    path (`compile_to_object` -> `link_executable` -> run).
//! 4. `decoy_probes`: three lexically-similar success/failure pairs.

use std::path::{Path, PathBuf};

use shin::parser::{BinOp, Node, NodeKind, Span, canonical, parse_source};

/// Repository root: two levels above `CARGO_MANIFEST_DIR` (`crates/shin`).
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/shin is two levels below the repo root")
        .to_path_buf()
}

/// `*.shin` files directly under `dir`, sorted. Anything else is ignored.
fn list_shin(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            println!("parse_corpus_unreadable_dir: {} ({e})", dir.display());
            return out;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("shin") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Value of the first header line starting with `marker`
/// (e.g. `-- expect:` or `-- error:`), if it is non-empty.
fn header_value(text: &str, marker: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix(marker)?;
        let value = rest.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

/// Whether a rejection code is one of the syntax-level E1xx codes.
fn is_e1xx(code: &str) -> bool {
    code.len() == 4 && code.starts_with("E1") && code[2..].bytes().all(|b| b.is_ascii_digit())
}

fn read_rel(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Corpus enumeration (PLAN.md "判定").
// ---------------------------------------------------------------------------

#[test]
fn corpus_counts() {
    let root = repo_root();
    let mut rels: Vec<String> = Vec::new();
    for dir in ["tests/accept", "tests/reject"] {
        for path in list_shin(&root.join(dir)) {
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            rels.push(rel);
        }
    }
    rels.sort();

    let mut accept_ok = Vec::new();
    let mut e1xx_ok = Vec::new();
    let mut non_e1xx_ok = Vec::new();

    for rel in &rels {
        let text = read_rel(&root, rel);
        let code = header_value(&text, "-- error:").unwrap_or_default();
        if rel.starts_with("tests/accept/") {
            match parse_source(&text, rel) {
                Ok(_) => {
                    println!("parse_accept_ok: {rel}");
                    accept_ok.push(rel.clone());
                }
                Err(d) => println!("parse_accept_err: {rel} {d}"),
            }
        } else if is_e1xx(&code) {
            match parse_source(&text, rel) {
                Err(d) => {
                    println!("parse_e1xx_diag: {rel} {d}");
                    if d.code.as_deref() == Some(code.as_str()) {
                        println!("parse_e1xx_ok: {rel} {code}");
                        e1xx_ok.push(rel.clone());
                    }
                }
                Ok(_) => println!("parse_e1xx_ng: {rel} {code}"),
            }
        } else {
            match parse_source(&text, rel) {
                Ok(_) => {
                    println!("parse_non_e1xx_ok: {rel}");
                    non_e1xx_ok.push(rel.clone());
                }
                Err(d) => println!("parse_non_e1xx_err: {rel} {d}"),
            }
        }
    }

    println!("parse_accept_ok_n: {}", accept_ok.len());
    println!("parse_e1xx_ok_n: {}", e1xx_ok.len());
    println!("parse_non_e1xx_ok_n: {}", non_e1xx_ok.len());

    // Parse/compile divergence (PLAN.md コーパス外プローブ 4): a non-E1xx
    // file that parses must not fail compilation at the syntax stage (a
    // parse diagnostic); an E1xx file must fail compilation at parse.
    let work = std::env::temp_dir().join(format!("shin-parse-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    for rel in &rels {
        if rel.starts_with("tests/accept/") {
            continue;
        }
        let text = read_rel(&root, rel);
        let code = header_value(&text, "-- error:").unwrap_or_default();
        let obj = work.join(rel.replace('/', "_"));
        match shin::compile_to_object(&text, &obj) {
            Err(msg) if is_e1xx(&code) => {}
            Err(msg) => {
                assert!(
                    !msg.contains("error[E1"),
                    "non-E1xx {rel} must not fail at the syntax stage, got: {msg}"
                );
            }
            Ok(()) => {}
        }
    }

    assert_eq!(accept_ok.len(), 23, "accept count");
    assert_eq!(e1xx_ok.len(), 7, "E1xx count");
    assert_eq!(non_e1xx_ok.len(), 39, "non-E1xx count");
}

// ---------------------------------------------------------------------------
// Production-path probe (PLAN.md コーパス外プローブ 2).
// ---------------------------------------------------------------------------

#[test]
fn gen_probe() {
    let mut rng = Rng::new(0x6E6F_5EED);
    let work = std::env::temp_dir().join(format!("shin-gen-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    for i in 0..5 {
        let a = (rng.below(1000)) as i64;
        let b = (rng.below(1000)) as i64;
        let src = format!("def main = {a} + {b}");
        let obj = work.join(format!("g{i}.o"));
        let exe = work.join(format!("g{i}"));
        if let Err(e) = shin::compile_to_object(&src, &obj) {
            panic!("gen {i}: compile failed: {e}");
        }
        shin::link_executable(&obj, &exe).unwrap_or_else(|e| panic!("gen {i}: link failed: {e}"));
        let out = std::process::Command::new(&exe).output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let want = (a + b).to_string();
        if stdout == want {
            println!("parse_gen_ok: {a} {b} {}", a + b);
        } else {
            panic!("gen {i}: got stdout {stdout:?}, want {want:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// Decoy probes (PLAN.md コーパス外プローブ 3).
// ---------------------------------------------------------------------------

#[test]
fn decoy_probes() {
    let ok_cases = [
        ("paren-expr", "def main = (1 + 2)"),
        ("string-comment", "def main = \"-- not a comment\""),
        ("juxt-call", "def inc x => x + 1\ndef main = inc 1"),
    ];
    let ng_cases = [
        ("paren-tuple", "def main = (1, 2)", "E105"),
        ("hash-comment", "def main = 1\n# c", "E104"),
        ("paren-call", "def inc x => x + 1\ndef main = inc(1)", "E106"),
    ];

    for (label, src) in ok_cases {
        match parse_source(src, "decoy") {
            Ok(_) => println!("parse_decoy_ok: {label}"),
            Err(d) => panic!("decoy ok-case {label} failed to parse: {d} ({src:?})"),
        }
    }
    for (label, src, code) in ng_cases {
        match parse_source(src, "decoy") {
            Err(d) if d.code.as_deref() == Some(code) => println!("parse_decoy_ng: {label}"),
            other => panic!(
                "decoy ng-case {label}: expected error[{code}], got {other:?} ({src:?})"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Expression-tree round trip (PLAN.md コーパス外プローブ 1).
// ---------------------------------------------------------------------------

/// Fixed-seed linear congruential generator (no external crates).
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

fn node(kind: NodeKind) -> Node {
    Node {
        kind,
        span: Span { line: 0, col: 0 },
    }
}

fn pick_bin_op(rng: &mut Rng) -> BinOp {
    match rng.below(13) {
        0 => BinOp::Add,
        1 => BinOp::Sub,
        2 => BinOp::Mul,
        3 => BinOp::Div,
        4 => BinOp::Mod,
        5 => BinOp::Lt,
        6 => BinOp::Le,
        7 => BinOp::Gt,
        8 => BinOp::Ge,
        9 => BinOp::Eq,
        10 => BinOp::Ne,
        11 => BinOp::And,
        _ => BinOp::Or,
    }
}

/// Precedence used to decide whether a rendered operand needs parentheses
/// (high = binds tighter). Application and `|>` are out of phase-A scope.
fn prec(op: BinOp) -> u8 {
    match op {
        BinOp::Mul | BinOp::Div | BinOp::Mod => 4,
        BinOp::Add | BinOp::Sub => 3,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne => 2,
        BinOp::And => 1,
        BinOp::Or => 0,
    }
}

/// Whether `child` must be parenthesized under `parent` on `side`.
fn parens_needed(child: &Node, parent: BinOp, is_left: bool) -> bool {
    if let NodeKind::Bin(cop, _, _) = &child.kind {
        let cp = prec(*cop);
        let pp = prec(parent);
        if cp < pp {
            true
        } else if cp > pp {
            false
        } else {
            match parent {
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne => {
                    true // comparisons are non-associative
                }
                _ => !is_left, // left-associative: only the right child needs parens
            }
        }
    } else {
        false
    }
}

/// Random separator between two tokens: a space or a newline (+indent) with
/// an optional `--` line comment. Never empty, so `-` never becomes `--`.
fn ws(rng: &mut Rng, indent: usize) -> String {
    match rng.below(4) {
        0 => " ".to_string(),
        1 => "  ".to_string(),
        2 => " ".to_string(),
        _ => {
            let pad = "  ".repeat(indent);
            if rng.chance(50) {
                format!("\n{pad}-- t{}\n{pad}", rng.below(100))
            } else {
                format!("\n{pad}")
            }
        }
    }
}

fn render_operand(
    rng: &mut Rng,
    child: &Node,
    parent: BinOp,
    is_left: bool,
    indent: usize,
) -> String {
    let need = parens_needed(child, parent, is_left);
    let s = render(rng, child, indent);
    if need || rng.chance(40) {
        format!("({s})")
    } else {
        s
    }
}

fn render(rng: &mut Rng, node: &Node, indent: usize) -> String {
    match &node.kind {
        NodeKind::Int(n) => n.to_string(),
        NodeKind::Neg(c) => {
            // The unary minus always carries a trailing space so a nested
            // `Neg(Neg(..))` renders as `- - 2`, never the `--` comment.
            let need = matches!(c.kind, NodeKind::Bin(..));
            let s = render(rng, c, indent + 1);
            let operand = if need || rng.chance(40) {
                format!("({s})")
            } else {
                s
            };
            format!("- {operand}")
        }
        NodeKind::Bin(op, l, r) => {
            let ls = render_operand(rng, l, *op, true, indent + 1);
            let rs = render_operand(rng, r, *op, false, indent + 1);
            format!("{ls}{}{}{}{rs}", ws(rng, indent), op.as_str(), ws(rng, indent))
        }
        other => panic!("render: unexpected node kind {other:?}"),
    }
}

fn gen_expr(rng: &mut Rng, depth: usize) -> Node {
    let leaf_pct = if depth == 0 { 15 } else { 45 };
    if depth >= 3 || rng.chance(leaf_pct) {
        node(NodeKind::Int(rng.below(20) as i64))
    } else if rng.chance(20) {
        let c = gen_expr(rng, depth + 1);
        node(NodeKind::Neg(Box::new(c)))
    } else {
        let op = pick_bin_op(rng);
        let l = gen_expr(rng, depth + 1);
        let r = gen_expr(rng, depth + 1);
        node(NodeKind::Bin(op, Box::new(l), Box::new(r)))
    }
}

#[test]
fn expression_tree_round_trip() {
    let mut rng = Rng::new(0x5EED_4E55);
    let n = 20;
    let mut ok = 0;
    let mut failures: Vec<String> = Vec::new();

    for i in 0..n {
        let tree = gen_expr(&mut rng, 0);
        let expected = canonical(&tree);
        let rendered = render(&mut rng, &tree, 0);
        let src = if rng.chance(30) {
            format!("def main = ({rendered})")
        } else {
            format!("def main = {rendered}")
        };
        match parse_source(&src, "corpus") {
            Ok(ast) => {
                let got = canonical(ast.main().expect("main"));
                if got == expected {
                    println!("parse_tree_ok: {i} {got}");
                    ok += 1;
                } else {
                    println!("parse_tree_mismatch: {i} expected={expected} got={got} src={src:?}");
                    failures.push(format!(
                        "#{i}: expected {expected}, got {got}, src={src:?}"
                    ));
                }
            }
            Err(d) => {
                println!("parse_tree_err: {i} {d} src={src:?}");
                failures.push(format!("#{i}: parse error {d}, src={src:?}"));
            }
        }
    }

    println!("parse_tree_n: {n}");
    assert!(
        failures.is_empty(),
        "round-trip failures:\n{}",
        failures.join("\n")
    );
    assert_eq!(ok, n, "expected all {n} trees to round-trip");
}