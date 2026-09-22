use hado::check_source;

fn rejected(source: &str, code: &str, anchor: &str) {
    let error = match check_source(source, "columns.hd") {
        Err(error) => error,
        Ok(_) => panic!("expected {code}: {source}"),
    };
    assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
    let prefix = &source[..source.find(anchor).unwrap()];
    assert_eq!(
        (error.line, error.col),
        (
            prefix.matches('\n').count() + 1,
            prefix.rsplit('\n').next().unwrap().chars().count() + 1
        ),
        "{source}: {error}"
    );
}

fn accepted(source: &str) {
    check_source(source, "columns.hd").unwrap();
}

const PAIR: &str = "type C2 = | A | B\n";

#[test]
fn two_column_coverage_matches_the_product_of_constructors() {
    let cells = [("A", "A"), ("A", "B"), ("B", "A"), ("B", "B")];
    for mask in 1u8..16 {
        let arms: String = cells
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(index, (left, right))| format!("  | {left}, {right} => {index}\n"))
            .collect();
        let complete = mask == 15;
        let source =
            format!("{PAIR}def f (a: C2, b: C2): Int =>\n  if a, b\n{arms}def main = f A B\n");
        if complete {
            accepted(&source);
        } else {
            rejected(&source, "E402", "if a, b");
        }
        let with_fallback = source.replace("def main", "  | 4\ndef main");
        if complete {
            rejected(&with_fallback, "E403", "| 4");
        } else {
            accepted(&with_fallback);
        }
    }
}

#[test]
fn wildcard_on_one_column_covers_that_constructor_product() {
    accepted(&format!(
        "{PAIR}def f (a: C2, b: C2): Int =>
  if a, b
  | A, _ => 0
  | B, _ => 1
def main = f A B
"
    ));
    rejected(
        &format!(
            "{PAIR}def f (a: C2, b: C2): Int =>
  if a, b
  | A, _ => 0
  | B, _ => 1
  | 2
def main = f A B
"
        ),
        "E403",
        "| 2",
    );
}

#[test]
fn all_variable_or_underscore_rows_are_redundant() {
    rejected(
        "def f (a: Int List, b: Int List): Int =>\n  if a, b\n  | x, y => List.length x + List.length y\ndef main = f [] []\n",
        "E512",
        "x, y",
    );
    rejected(
        "def f (a: Int List, b: Int List): Int =>\n  if a, b\n  | x, _ => List.length x\ndef main = f [] []\n",
        "E512",
        "x, _",
    );
    rejected(
        "def f (a: Int List, b: Int List): Int =>\n  if a, b\n  | _, _ => 0\ndef main = f [] []\n",
        "E503",
        "_, _",
    );
    accepted(
        "def f (a: Int List, b: Int List): Int List =>
  if a, b
  | _, [] => a
  | [], _ => b
  | [_, ..._], [_, ..._] => a
def main = f [] []
",
    );
}

#[test]
fn boolean_literals_are_e520_only_for_a_single_column() {
    rejected(
        "def f (a: Bool): Int =>\n  if a\n  | true => 1\n  | false => 2\ndef main = f true\n",
        "E520",
        "if a",
    );
    accepted(
        "def f (a: Bool, b: Bool): Int =>
  if a, b
  | true, true => 1
  | true, false => 2
  | false, _ => 3
def main = f true false
",
    );
}

#[test]
fn non_boolean_literals_require_fallback_in_any_column() {
    rejected(
        "def f (a: Int, b: Int List): Int =>\n  if a, b\n  | 1, [] => 0\n  | _, [_, ..._] => 1\ndef main = f 1 []\n",
        "E402",
        "if a, b",
    );
    accepted(
        "def f (a: Int, b: Int List): Int =>\n  if a, b\n  | 1, [] => 0\n  | _, [_, ..._] => 1\n  | 2\ndef main = f 1 []\n",
    );
}

#[test]
fn duplicate_and_unreachable_rows_use_covers_on_every_column() {
    rejected(
        &format!(
            "{PAIR}def f (a: C2, b: C2): Int =>\n  if a, b\n  | A, _ => 0\n  | A, _ => 1\n  | B, _ => 2\ndef main = f A B\n"
        ),
        "E405",
        "| A, _ => 1",
    );
    rejected(
        &format!(
            "{PAIR}def f (a: C2, b: C2): Int =>\n  if a, b\n  | A, _ => 0\n  | A, B => 1\n  | B, _ => 2\ndef main = f A B\n"
        ),
        "E403",
        "| A, B => 1",
    );
    accepted(&format!(
        "{PAIR}def f (a: C2, b: C2): Int =>
  if a, b
  | A, B => 0
  | A, A => 1
  | B, _ => 2
def main = f A B
"
    ));
}

#[test]
fn arm_conditions_keep_condition_column_rules_and_skip_e520() {
    rejected(
        "def f (x: Int Option): Int =>\n  if x\n  | None => 0\n  | Some(v) => if\n    | v > 0 => v\ndef main = f (Some(1))\n",
        "E401",
        "if\n    | v > 0",
    );
    accepted(
        "def f (x: Int Option): Int =>\n  if x\n  | None => 0\n  | Some(v) => if\n    | v > 0 => v\n    | 0\ndef main = f (Some(1))\n",
    );
    rejected(
        "def f (x: Int Option): Int =>\n  if x\n  | None => 0\n  | Some(v) => if\n    | v == 1 => 1\n    | v == 2 => 2\n    | 0\ndef main = f (Some(1))\n",
        "E511",
        "if\n    | v == 1",
    );
    rejected(
        "def f (x: Int Option): Int =>\n  if x\n  | None => 0\n  | Some(v) => if\n    | v > 0 => 1\n    | v > 0 => 2\n    | 0\ndef main = f (Some(1))\n",
        "E406",
        "| v > 0 => 2",
    );
}

#[test]
fn arm_conditions_cover_their_pattern_row() {
    accepted(
        "def f (x: Int Option): Int =>
  if x
  | Some(v) => if
    | v > 0 => v
    | 0
  | None => 1
def main = f (Some(1))
",
    );
    rejected(
        "def f (x: Int Option): Int =>
  if x
  | Some(v) => if
    | v > 0 => v
    | 0
def main = f (Some(1))
",
        "E402",
        "if x",
    );
}

#[test]
fn bindings_from_every_column_are_visible_including_arm_conditions() {
    accepted(
        "def f (a: Int Option, b: Int List): Int =>
  if a, b
  | Some(v), [x, ..._] => if
    | v > x => v
    | x
  | None, _ => 0
  | _, [] => 1
def main = f (Some(1)) [2]
",
    );
    rejected(
        "def f (a: Int Option, b: Int List): Int =>
  if a, b
  | Some(unused), [] => 0
  | None, [] => 1
  | _, [_, ..._] => 2
def main = f (Some(1)) []
",
        "E505",
        "unused",
    );
}

#[test]
fn rewrap_repropagate_and_sole_parameter_stay_unary() {
    accepted(
        "def f (a: Int Option, b: Int Option): Int Option =>
  if a, b
  | Some(x), None => Some(x)
  | None, Some(y) => Some(y)
  | Some(x), Some(y) => Some(x + y)
  | None, None => None
def main = f (Some(1)) None
",
    );
    accepted(
        "def f x y =>
  if x, y
  | Some(a), None => a
  | None, Some(b) => b
  | Some(a), Some(b) => a + b
  | 0
def main = f (Some(1)) None
",
    );
}

fn int_constant_copies(ir: &str, value: i64) -> usize {
    ir.matches(&format!("Constant(Int({value}))")).count()
}

#[test]
fn three_column_four_row_wildcard_body_stays_within_constructor_power() {
    // A wildcard row copies its body once per surviving constructor prefix, so
    // the count is bounded by the number of constructor tuples for the columns
    // (2^columns); the exact specialization order is not asserted. The
    // two-column case is the same bound over a smaller tuple space.
    for (columns, source) in [
        (
            2,
            format!(
                "{PAIR}def f (a: C2, b: C2): Int =>
  if a, b
  | A, B => 1
  | B, B => 2
  | _, A => 99
def main = f A B
"
            ),
        ),
        (
            3,
            format!(
                "{PAIR}def f (a: C2, b: C2, c: C2): Int =>
  if a, b, c
  | A, A, B => 1
  | A, B, B => 2
  | B, _, B => 3
  | _, _, A => 99
def main = f A A A
"
            ),
        ),
    ] {
        let ir = hado::dump_mono(&source, "t").unwrap();
        let copies = int_constant_copies(&ir, 99);
        assert!(
            copies >= 1,
            "{columns}-column wildcard body was not copied\n{ir}"
        );
        assert!(
            copies <= (1 << columns),
            "{columns}-column wildcard body copies {copies}, bound 2^{columns}\n{ir}"
        );
        assert!(
            ir.contains("Constant(Int(1))") && ir.contains("Constant(Int(2))"),
            "{ir}"
        );
    }
}

#[test]
fn arm_condition_tail_call_stays_tail_in_two_columns() {
    let source = format!(
        "{PAIR}def rec down a b n =>
  if a, b
  | A, _ => if
    | n <= 0 => 42
    | down a b (n - 1)
  | B, _ => 0
def main = down A B 3
"
    );
    let ir = hado::dump_mono(&source, "t").unwrap();
    assert!(ir.contains("TailCall { function: FunctionId(0)"), "{ir}");
    assert!(!ir.contains("CallDirect { function: FunctionId(0)"), "{ir}");
}

#[test]
fn bool_columns_lower_to_nested_branches() {
    let source = "def f (a: Bool, b: Bool): Int =>
  if a, b
  | true, true => 1
  | true, false => 2
  | false, _ => 3
def main = f true false
";
    let ir = hado::dump_mono(source, "t").unwrap();
    assert!(ir.contains("Branch {"), "{ir}");
    assert!(ir.contains("Constant(Int(1))"), "{ir}");
    assert!(ir.contains("Constant(Int(3))"), "{ir}");
}

#[test]
fn record_column_binds_fields_before_the_remaining_switch() {
    let source = "type User = { age: Int }
type Flag = | On | Off
def f (user: User, flag: Flag): Int =>
  if user, flag
  | { age = n }, On => n
  | { age = _ }, Off => 0
def main = f { age = 3 } On
";
    let ir = hado::dump_mono(source, "t").unwrap();
    assert!(ir.contains("RecordGet {"), "{ir}");
    assert!(ir.contains("field: \"age\""), "{ir}");
    assert!(ir.contains("(\"On\""), "{ir}");
    assert!(ir.contains("(\"Off\""), "{ir}");
}
