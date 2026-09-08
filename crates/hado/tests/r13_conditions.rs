use hado::{check_source, parser};

fn random(state: &mut u64, limit: u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*state >> 32) % limit
}

fn rejected(source: &str, code: &str, anchor: &str) {
    let error = check_source(source, "conditions.hd").err().unwrap();
    assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
    let prefix = &source[..source.find(anchor).unwrap()];
    assert_eq!((error.line, error.col),
        (prefix.matches('\n').count() + 1, prefix.rsplit('\n').next().unwrap().chars().count() + 1),
        "{source}: {error}");
}

#[test]
fn generated_functions_reject_long_form_and_execute_short_form() {
    let mut state = 0x13517;
    let mut covered = [[false; 2]; 3];
    for serial in 0..36 {
        let arity = random(&mut state, 3) as usize + 1;
        let typed = random(&mut state, 2) == 1;
        covered[arity - 1][usize::from(typed)] = true;
        let names = (0..arity).map(|i| format!("x{i}")).collect::<Vec<_>>();
        let params = if typed {
            format!("({})", names.iter().map(|n| format!("{n}: Int")).collect::<Vec<_>>().join(", "))
        } else { names.join(" ") };
        let values = (0..arity).map(|_| random(&mut state, 31) as i64 - 15).collect::<Vec<_>>();
        let args = values.iter().map(|v| format!("({v})")).collect::<Vec<_>>().join(" ");
        let threshold = random(&mut state, 41) as i64 - 20;
        let sum: i64 = values.iter().sum();
        let arms = format!("| {} > ({threshold}) => 7 | 3", names.join(" + "));
        let expected = if sum > threshold { "7" } else { "3" };
        let padding = format!("{}{}", "\n".repeat(random(&mut state, 4) as usize), " ".repeat(random(&mut state, 9) as usize));
        for lambda in [false, true] {
            let source = |body: &str| if lambda {
                format!("{padding}def main = (fn {params} {body}) {args}")
            } else {
                let ret = if typed { ": Int" } else { "" };
                format!("{padding}def probe {params}{ret} {body}\ndef main = probe {args}")
            };
            rejected(&source(&format!("=> if {arms}")), "E517", if lambda { "fn " } else { "def probe" });
            rejected(&source(&format!("=> (if {arms})")), "E517", if lambda { "fn " } else { "def probe" });
            let short = source(&arms);
            check_source(&short, "conditions.hd").unwrap();
            assert_round_trip(&short);
            execute(&short, expected, serial * 2 + usize::from(lambda));
            let continuation = source(&format!("=> let bound = {} + 1 -> if | bound > 0 => {} | 2", names.join(" + "), names.join(" + ")));
            check_source(&continuation, "conditions.hd").unwrap();
        }
    }
    assert!(covered.iter().flatten().all(|seen| *seen));
}

fn bodies(ast: &parser::Ast) -> Vec<&parser::Node> {
    ast.items.iter().filter_map(|item| match item {
        parser::Item::Def(def) => Some(match &def.kind {
            parser::DefKind::Value(body) | parser::DefKind::Function { body, .. } => body,
        }),
        _ => None,
    }).collect()
}

fn assert_round_trip(source: &str) {
    let original = parser::parse_source(source, "round.hd").unwrap();
    let printed = parser::canonical_program(&original);
    assert!(!printed.contains("=> (if"), "{printed}");
    let restored = parser::parse_source(&printed, "round.hd").unwrap();
    assert_eq!(bodies(&original), bodies(&restored), "{source}\n{printed}");
    assert_eq!(parser::canonical_program(&restored), printed);
    check_source(&printed, "round.hd").unwrap();
}

#[test]
fn condition_and_pattern_functions_keep_their_diagnostics() {
    rejected("def f x | x == 1 => 2 | 0\ndef main = f 1", "E511", "def f");
    rejected("def main | true => 1 | 0", "E206", "def main");
    rejected("def f x | Some(y) => y | 0\ndef main = f 1", "E204", "y)");
    rejected("def f x | x > 0 => 1\ndef main = f 1", "E401", "def f");
    rejected("def main = (fn x | x > 0 => 1) 1", "E401", "fn ");
    rejected("def f x | x > 0 => 1 | x > 0 => 2 | 0\ndef main = f 1", "E406", "| x > 0 => 2");
    rejected("def f x => if x | Some(y) => y | None => 0\ndef main = f (Some(1))", "E515", "def f");
}

#[test]
fn condition_columns_require_a_condition_arm() {
    for source in ["def f x | 1", "def main = fn x | 1", "def main = if | 1"] {
        let error = parser::parse_source(source, "empty.hd").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"), "{source}: {error}");
        assert_eq!((error.line, error.col), (1, source.len() + 1));
    }
}

fn execute(source: &str, expected: &str, serial: usize) {
    let dir = std::env::temp_dir().join(format!("hado-r13-{}-{serial}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("probe.hd");
    std::fs::write(&path, source).unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&path).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let result = std::process::Command::new(path.with_extension("")).output().unwrap();
    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout).unwrap(), format!("{expected}\n"));
    assert!(result.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}


#[test]
fn long_form_provenance_survives_modules_and_printing_is_canonical() {
    let source = "module type S = sig def f(x: Int): Int end\nmodule M : S = struct def f x => if | x > 0 => 1 | 0 end\ndef main = M.f 2";
    rejected(source, "E517", "def f x");
    let ast = parser::parse_source(source, "module.hd").unwrap();
    let printed = parser::canonical_program(&ast);
    assert!(!printed.contains("=> (if"), "{printed}");
    check_source(&printed, "module.hd").unwrap();
    rejected("def f x => if | missing > x => 1 | 0\ndef main = f 1", "E204", "missing");
    rejected("def f x => if | x > 0 => 1\ndef main = f 1", "E401", "if ");
    check_source("def f x => if x > 0 => 1 | 0\ndef main = f 1", "boolean.hd").unwrap();
}

#[test]
fn condition_functions_cannot_have_an_empty_parameter_list() {
    for source in ["def f: Int | true => 1 | 0", "def f() | true => 1 | 0", "def f(): Int | true => 1 | 0"] {
        let error = parser::parse_source(source, "zero.hd").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"), "{source}: {error}");
    }
    let error = parser::parse_source("def f => 1", "zero.hd").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E109"));
    assert_eq!((error.line, error.col), (1, 7));
}
