use std::collections::{BTreeMap, BTreeSet};

use hado::parser::{self, DefKind, ImplementationForm, Item, Node, Param, TypeExpr};

#[derive(Default)]
struct Generator {
    state: u64,
    serial: usize,
    choices: BTreeMap<&'static str, Vec<bool>>,
    declarations: usize,
    allow_resume: bool,
    library_emitted: bool,
    /// 直前の生成で `->` の束縛を綴ったか。`capture` が区間ごとに数える。
    continuation: bool,
    /// `fn(...)` の本体・腕と、pattern の腕の条件列（`arm_conditions`）を
    /// 組んでいる深さ。>0 の間は `->` の束縛を綴らない。どちらも `fn(...)`
    /// の腕に置ける閉じの綴りが無く、正準形が `fn(...)`・`if … /if` のままに
    /// なって往復しないため（§12・§12.1・§26.1）。
    sealed_depth: usize,
    /// 直前の生成式の根が単項 `-`（Neg）か。`- - x` は正準形で `--x` と綴られ、
    /// コメントの印（E104）に読まれるため、`-` の被演算子には Neg を置かない。
    neg: bool,
    /// 直前の生成式の正準形が `fn` で始まるか。二項演算子の右被演算子に
    /// `fn` を直に置けない（E100）ため、右被演算子では `not` を挟む。
    fn_head: bool,
}

impl Generator {
    fn random(&mut self, limit: usize) -> usize {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.state >> 32) as usize) % limit
    }

    fn choose(&mut self, rule: &'static str, count: usize) -> usize {
        let choice = self.random(count);
        self.mark(rule, count, choice);
        choice
    }

    fn mark(&mut self, rule: &'static str, count: usize, choice: usize) {
        let visited = self
            .choices
            .entry(rule)
            .or_insert_with(|| vec![false; count]);
        assert_eq!(visited.len(), count, "inconsistent alternatives for {rule}");
        visited[choice] = true;
    }

    /// 式を生成し、その中に `->` の束縛があるかを返す。`if` の本体と `fn` の
    /// 本体は、束縛の有無で閉じの印が変わる（§12.1）。返した真偽は外側の
    /// 判定にも引き継ぐ（束縛は入れ子のどこにあっても数える）。
    fn capture<T>(&mut self, generate: impl FnOnce(&mut Self) -> T) -> (T, bool) {
        let saved = self.continuation;
        self.continuation = false;
        let value = generate(self);
        let had = self.continuation;
        self.continuation = saved || had;
        (value, had)
    }

    /// `if` の条件や scrutinee、`with` の引数のように、束縛があっても閉じの
    /// 判定に数えない位置の式を生成する（§12.1 の `contains_continuation`）。
    fn disregard<T>(&mut self, generate: impl FnOnce(&mut Self) -> T) -> T {
        let saved = self.continuation;
        self.continuation = false;
        let value = generate(self);
        self.continuation = saved;
        value
    }

    fn name(&mut self, upper: bool) -> String {
        self.mark(if upper { "upper_name" } else { "name" }, 1, 0);
        self.serial += 1;
        let suffix = self.random(10000);
        format!(
            "{}{:_>3}_{suffix}",
            if upper { "Type" } else { "value" },
            self.serial
        )
    }

    fn list(
        &mut self,
        count: usize,
        mut element: impl FnMut(&mut Self) -> String,
        separator: &str,
    ) -> String {
        (0..count)
            .map(|_| element(self))
            .collect::<Vec<_>>()
            .join(separator)
    }

    fn program(&mut self) -> String {
        self.declarations = 0;
        self.library_emitted = false;
        self.mark("program", 1, 0);
        let outer_count = self.choose("outer_item*", 3);
        let section_count = self.choose("section*", 3);
        let outer = self.list(outer_count, |g| g.outer_item(), "\n");
        let sections = self.list(section_count, |g| g.section(), "\n");
        format!("{outer}\n{sections}")
    }

    fn outer_item(&mut self) -> String {
        match self.choose("outer_item", 3) {
            0 if !self.library_emitted => {
                self.library_emitted = true;
                "library".to_string()
            }
            0 | 1 => self.type_definition(),
            _ => self.definition(),
        }
    }

    fn section(&mut self) -> String {
        match self.choose("section", 6) {
            0 => self.module_section(false).0,
            1 => self.implement_section(),
            2 => self.functor_section().0,
            3 => self.apply_section(),
            4 => self.effect_section(),
            _ => self.handler_section(),
        }
    }

    fn definition(&mut self) -> String {
        self.declarations += 1;
        let name = self.name(false);
        match self.choose("definition", 4) {
            0 => format!("def {name} = {}", self.expression(3, false)),
            1 => format!(
                "def {}{name} {} {} => {}",
                self.rec(),
                self.parameters(),
                self.return_type(),
                self.expression(3, false)
            ),
            2 => format!("def {}{name} {}", self.rec(), self.arms(2)),
            _ => format!(
                "def {}{name} {} {} {}",
                self.rec(),
                self.parameters(),
                self.return_type(),
                self.condition_arms(2)
            ),
        }
    }

    fn rec(&mut self) -> &'static str {
        if self.choose("rec?", 2) == 0 {
            ""
        } else {
            "rec "
        }
    }

    fn parameters(&mut self) -> String {
        let count = self.random(3) + 1;
        if self.choose("parameters", 2) == 0 {
            self.list(count, |g| g.name(false), " ")
        } else {
            format!("( {} )", self.list(count, |g| g.typed_param(), " , "))
        }
    }

    fn typed_param(&mut self) -> String {
        self.mark("typed_param", 1, 0);
        format!("{} : {}", self.name(false), self.type_expr(2))
    }

    fn return_type(&mut self) -> String {
        if self.choose("return_type?", 2) == 0 {
            return String::new();
        }
        self.mark("return_type", 1, 0);
        format!(": {}", self.result(2))
    }

    fn type_params(&mut self) -> String {
        if self.choose("type_vars?", 2) == 0 {
            return String::new();
        }
        let count = self.random(3) + 1;
        self.mark("type_vars", 2, usize::from(count > 1));
        if count == 1 {
            self.name(true)
        } else {
            format!("( {} )", self.list(count, |g| g.name(true), " , "))
        }
    }

    fn type_definition(&mut self) -> String {
        self.declarations += 1;
        let name = self.name(true);
        let params = self.type_params();
        let body = match self.choose("type_definition", 3) {
            0 => {
                let count = self.random(3) + 1;
                self.list(count, |g| format!("| {}", g.constructor()), " ")
            }
            1 => self.record_type(),
            _ => format!("{} {}", self.name(true), self.record_type()),
        };
        format!("type {params} {name} = {body}")
    }

    fn constructor(&mut self) -> String {
        let name = self.name(true);
        match self.choose("constructor", 3) {
            0 => return name,
            1 => return format!("{name} {}", self.record_type()),
            _ => {}
        }
        let count = self.random(3) + 1;
        format!("{name} ( {} )", self.list(count, |g| g.type_expr(2), " , "))
    }

    fn record_type(&mut self) -> String {
        self.mark("record_type", 1, 0);
        let count = self.random(3) + 1;
        format!(
            "{{ {} }}",
            self.list(
                count,
                |g| {
                    g.mark("field_type", 1, 0);
                    format!("{} : {}", g.name(false), g.type_expr(2))
                },
                " ; "
            )
        )
    }

    fn type_expr(&mut self, depth: usize) -> String {
        self.mark("type_expr", 1, 0);
        let choice = if depth == 0 { 0 } else { self.random(3) };
        self.mark("type_atom", 3, choice);
        let mut source = match choice {
            0 => self.type_ref(),
            1 => {
                let count = self.random(3) + 2;
                format!(
                    "( {} ) {}",
                    self.list(count, |g| g.type_expr(depth - 1), " , "),
                    self.type_ref()
                )
            }
            _ => format!(
                "Fn ( {} => {} )",
                self.fn_params(depth - 1),
                self.result(depth.saturating_sub(1))
            ),
        };
        let count = self.choose("type_ref*", 3);
        for _ in 0..count {
            source.push_str(&format!(" {}", self.type_ref()));
        }
        source
    }

    fn path(&mut self) -> String {
        if self.choose("path?", 2) == 0 {
            return String::new();
        }
        self.mark("path", 1, 0);
        let count = self.choose("path_segments+", 3) + 1;
        format!("{} :: ", self.list(count, |g| g.name(false), " :: "))
    }

    fn upper_reference(&mut self) -> String {
        format!("{}{}", self.path(), self.name(true))
    }

    fn type_ref(&mut self) -> String {
        if self.choose("type_ref", 2) == 0 {
            self.upper_reference()
        } else {
            format!("{} . {}", self.upper_reference(), self.name(true))
        }
    }

    fn fn_params(&mut self, depth: usize) -> String {
        let count = self.random(3) + 1;
        self.list(count, |g| g.type_expr(depth), " , ")
    }

    fn module_section(&mut self, abstract_member: bool) -> (String, String, String) {
        self.mark("module_section", 1, 0);
        self.declarations += 1;
        let name = self.name(true);
        let abstract_name = self.name(true);
        let count = self.choose("module_item*", 3);
        let mut items = self.list(count, |g| g.module_item(), " ");
        if abstract_member {
            self.declarations += 1;
            items.push_str(&format!(" type {abstract_name}"));
        }
        (
            format!("module {name} {items} /module"),
            name,
            abstract_name,
        )
    }

    fn module_item(&mut self) -> String {
        self.declarations += 1;
        if self.choose("module_item", 2) == 0 {
            return format!("type {}", self.name(true));
        }
        let name = self.name(false);
        let params = self.type_params();
        let count = self.random(3) + 1;
        format!(
            "def {params} {name} ( {} ) : {}",
            self.list(
                count,
                |g| {
                    g.mark("contract_param", 1, 0);
                    g.typed_param()
                },
                " , "
            ),
            self.result(2)
        )
    }

    fn implement_section(&mut self) -> String {
        self.mark("implement_section", 1, 0);
        let (signature, sig_name, abstract_name) = self.module_section(true);
        self.mark("contract_ref", 1, 0);
        let sig_name = format!("{}{}", self.path(), sig_name);
        self.declarations += 1;
        let count = self.choose("implement_item*", 3);
        let items = self.list(count, |g| g.implement_item(&abstract_name), " ");
        format!(
            "{signature}\nimplement {sig_name} as {} {items}\n/implement",
            self.name(true)
        )
    }

    fn functor_section(&mut self) -> (String, String, String) {
        self.mark("functor_section", 1, 0);
        let (signature, sig_name, abstract_name) = self.module_section(true);
        self.mark("contract_ref", 1, 0);
        let name = self.name(true);
        let count = self.choose("functor_param+", 3) + 1;
        let params = self.list(
            count,
            |g| {
                g.mark("functor_param", 1, 0);
                g.mark("contract_ref", 1, 0);
                format!("{} : {}{sig_name}", g.name(true), g.path())
            },
            " , ",
        );
        self.declarations += 2;
        let shared = if self.choose("functor_shared", 2) == 0 {
            "shared "
        } else {
            ""
        };
        (format!("{signature}\nimplement {}{sig_name} as {name} ( {params} ) {shared}type {abstract_name} = Int\n/implement", self.path()), sig_name, name)
    }

    fn apply_section(&mut self) -> String {
        self.mark("apply_section", 1, 0);
        let (definition, signature, functor) = self.functor_section();
        self.mark("contract_ref", 1, 0);
        self.mark("impl_ref", 1, 0);
        let count = self.choose("impl_ref+", 3) + 1;
        let arguments = self.list(
            count,
            |g| {
                g.mark("impl_ref", 1, 0);
                g.upper_reference()
            },
            " , ",
        );
        self.declarations += 1;
        format!(
            "{definition}\nimplement {}{signature} as {} = {}{functor} ( {arguments} )",
            self.path(),
            self.name(true),
            self.path()
        )
    }

    fn implement_item(&mut self, abstract_name: &str) -> String {
        match self.choose("implement_item", 4) {
            0 => self.definition(),
            1 => self.type_definition(),
            2 => {
                self.declarations += 1;
                format!("type {abstract_name} = {}", self.type_expr(2))
            }
            _ => {
                self.declarations += 1;
                format!("shared type {abstract_name} = {}", self.type_expr(2))
            }
        }
    }

    fn expression(&mut self, depth: usize, in_arm: bool) -> String {
        if depth == 0 || self.choose("expression", 2) == 0 {
            self.mark("expression", 2, 0);
            return self.pipe(depth, in_arm);
        }
        self.prefix(depth - 1, in_arm)
    }

    fn prefix(&mut self, depth: usize, in_arm: bool) -> String {
        if self.sealed_depth > 0 {
            // `fn` の本体では `->` を綴れない（§12.1）。ここは pipe に限る。
            return self.pipe(depth, in_arm);
        }
        let choice = self.random(if in_arm { 2 } else { 4 });
        self.prefix_variant(depth, in_arm, choice)
    }

    fn prefix_variant(&mut self, depth: usize, in_arm: bool, choice: usize) -> String {
        self.mark("prefix_form", 4, choice);
        let source = match choice {
            0 => {
                self.mark("binding", 1, 0);
                let pattern = self.pattern(2);
                let (value, _) = self.capture(|g| g.expression(depth, in_arm));
                let (body, _) = self.capture(|g| g.expression(depth, in_arm));
                self.continuation = true;
                format!("( {value} ) as {pattern} -> {body}")
            }
            1 => {
                self.mark("rail_discard", 1, 0);
                let (value, _) = self.capture(|g| g.expression(depth, in_arm));
                let (body, _) = self.capture(|g| g.expression(depth, in_arm));
                self.continuation = true;
                format!("( {value} ) -> {body}")
            }
            2 => self.with_form(depth, in_arm),
            _ => self.branch(depth),
        };
        // 束縛・破棄・with・分岐は根が Neg にならず、`fn` で始まらない。
        self.neg = false;
        self.fn_head = false;
        source
    }

    fn with_form(&mut self, depth: usize, in_arm: bool) -> String {
        self.mark("with_form", 1, 0);
        self.mark("impl_ref", 1, 0);
        let handler = self.upper_reference();
        let args = if self.choose("with_args?", 2) == 0 {
            String::new()
        } else {
            self.mark("with_args", 1, 0);
            let count = self.random(3) + 1;
            // `with` の引数は括弧が閉じるので、束縛の有無を本体へ数えない。
            let items = self.disregard(|g| g.list(count, |h| h.expression(depth, in_arm), " , "));
            format!("( {items} )")
        };
        let (body, _) = self.capture(|g| g.expression(depth, in_arm));
        format!("with {handler}{args} -> {body}")
    }

    fn result(&mut self, depth: usize) -> String {
        self.mark("result", 1, 0);
        format!("{}{}", self.type_expr(depth), self.effect_row_suffix())
    }

    fn effect_row_suffix(&mut self) -> String {
        if self.choose("effect_row?", 2) == 0 {
            return String::new();
        }
        match self.choose("effect_row", 2) {
            0 => {
                let count = self.random(3) + 1;
                format!(" {{ {} }}", self.list(count, |g| g.type_expr(1), " , "))
            }
            _ => " Pure".into(),
        }
    }

    fn effect_section(&mut self) -> String {
        self.mark("effect_section", 1, 0);
        self.declarations += 1;
        let params = self.type_params();
        let name = self.name(true);
        let count = self.choose("effect_item*", 3);
        let ops = self.list(count, |g| g.effect_item(), " ");
        format!("effect {params} {name} {ops}")
    }

    fn effect_item(&mut self) -> String {
        let kind = self.op_kind();
        let name = self.name(false);
        let params = if self.choose("effect_item", 2) == 0 {
            let count = self.random(3) + 1;
            format!(
                "( {} )",
                self.list(
                    count,
                    |g| {
                        g.mark("contract_param", 1, 0);
                        g.typed_param()
                    },
                    " , "
                )
            )
        } else {
            String::new()
        };
        let ret = if kind == "abort" || self.random(2) == 0 {
            String::new()
        } else {
            format!(" : {}", self.result(1))
        };
        format!("{kind} {name}{params}{ret}")
    }

    fn op_kind(&mut self) -> &'static str {
        ["once", "abort", "again"][self.choose("op_kind", 3)]
    }

    fn handler_section(&mut self) -> String {
        self.mark("handler_section", 1, 0);
        self.declarations += 1;
        let params = self.type_params();
        self.mark("contract_ref", 1, 0);
        let effect = self.upper_reference();
        let name = self.name(true);
        let handler_params = if self.choose("handler_params?", 2) == 0 {
            String::new()
        } else {
            self.mark("handler_params", 1, 0);
            let count = self.random(3) + 1;
            format!("( {} )", self.list(count, |g| g.typed_param(), " , "))
        };
        let count = self.choose("handler_item*", 3);
        let items = self.list(count, |g| g.handler_item(), " ");
        format!("handler {params} {effect} as {name}{handler_params} {items}")
    }

    fn handler_item(&mut self) -> String {
        if self.choose("handler_item", 2) == 0 {
            let kind = self.op_kind();
            let name = self.name(false);
            let count = self.random(3);
            let params = self.list(
                count,
                |g| {
                    g.mark("param", 1, 0);
                    format!(
                        " {}",
                        if g.random(2) == 0 {
                            "_".into()
                        } else {
                            g.name(false)
                        }
                    )
                },
                "",
            );
            let saved = self.allow_resume;
            self.allow_resume = kind == "again";
            let body = self.expression(2, false);
            self.allow_resume = saved;
            format!("{kind} {name}{params} => {body}")
        } else {
            self.mark("param", 1, 0);
            let param = if self.random(2) == 0 {
                "_".into()
            } else {
                self.name(false)
            };
            let saved = self.allow_resume;
            self.allow_resume = false;
            let body = self.expression(2, false);
            self.allow_resume = saved;
            format!("return {param} => {body}")
        }
    }

    fn lambda(&mut self, depth: usize, in_arm: bool) -> String {
        let choice = if in_arm {
            self.mark("lambda", 3, 0);
            0
        } else {
            self.choose("lambda", 3)
        };
        // `fn(...)` の腕と本体には `->` を綴らない。腕に束縛を置く綴りが
        // `fn … /fn` に無く、正準形が往復しないため（§12・§12.1）。
        self.sealed_depth += 1;
        let source = if choice == 0 {
            format!(
                "fn ( {} => {} )",
                self.lambda_params(),
                self.expression(depth, in_arm)
            )
        } else if choice == 1 {
            format!("fn ( {} )", self.arms(depth))
        } else {
            format!(
                "fn ( {} {} )",
                self.lambda_params(),
                self.condition_arms(depth)
            )
        };
        self.sealed_depth -= 1;
        source
    }

    fn lambda_params(&mut self) -> String {
        let count = self.random(3) + 1;
        if self.choose("lambda_params", 2) == 0 {
            self.list(
                count,
                |g| {
                    g.mark("param", 1, 0);
                    g.name(false)
                },
                " , ",
            )
        } else {
            self.list(count, |g| g.typed_param(), " , ")
        }
    }

    fn pipe(&mut self, depth: usize, in_arm: bool) -> String {
        let mut source = self.binary(0, depth, in_arm);
        // 先頭の綴り（`fn` か）は左端の被演算子が決める。
        let head = self.fn_head;
        let count = if depth == 0 {
            0
        } else {
            self.choose("pipe", 3)
        };
        self.mark("pipe", 3, count);
        if count > 0 {
            // `|>` が付くと根はパイプになり Neg ではなくなる。
            self.neg = false;
        }
        for _ in 0..count {
            source.push_str(&format!(" |> {}", self.pipe_target(depth - 1, in_arm)));
        }
        self.fn_head = head;
        source
    }

    fn pipe_target(&mut self, depth: usize, in_arm: bool) -> String {
        let head = if self.choose("pipe_callee", 2) == 0 {
            self.name(false)
        } else {
            format!("{} . {}", self.upper_reference(), self.name(false))
        };
        let count = self.choose("pipe_arguments", 3);
        let target = format!(
            "{head} {}",
            self.list(count, |g| g.primary(depth, in_arm), " ")
        );
        self.neg = false;
        self.fn_head = false;
        target
    }

    fn binary(&mut self, level: usize, depth: usize, in_arm: bool) -> String {
        const RULES: &[(&str, &[&str])] = &[
            ("or", &["||"]),
            ("and", &["&&"]),
            ("compare", &["<", "<=", ">", ">=", "==", "!="]),
            ("additive", &["+", "-"]),
            ("multiplicative", &["*", "/", "%"]),
        ];
        if level == RULES.len() {
            return self.unary(depth, in_arm);
        }
        let (rule, operators) = RULES[level];
        let mut source = self.binary(level + 1, depth, in_arm);
        // 二項演算子の左端は左被演算子なので、先頭の綴りは左が決める。
        let head = self.fn_head;
        let count = if depth == 0 || self.random(4) != 0 {
            0
        } else if rule == "compare" {
            1
        } else {
            self.random(2) + 1
        };
        self.mark(
            ["or*", "and*", "compare?", "additive*", "multiplicative*"][level],
            if rule == "compare" { 2 } else { 3 },
            count,
        );
        if count == 0 {
            self.mark(rule, operators.len() + 1, 0);
        }
        for _ in 0..count {
            let operator = self.random(operators.len());
            self.mark(rule, operators.len() + 1, operator + 1);
            let right = self.binary(level + 1, depth - 1, in_arm);
            // 演算子の直後に `fn` を置けない（E100、§7・§12.1）。`not` を挟む。
            let right = if self.fn_head {
                format!("not {right}")
            } else {
                right
            };
            source.push_str(&format!(" {} {}", operators[operator], right));
        }
        if count > 0 {
            // 二項演算子が付くと根はその演算になり Neg ではなくなる。
            self.neg = false;
        }
        self.fn_head = head;
        source
    }

    fn unary(&mut self, depth: usize, in_arm: bool) -> String {
        let choice = if depth == 0 { 2 } else { self.random(3) };
        self.mark("unary", 3, choice);
        match choice {
            0 => {
                let operand = self.unary_operand(depth - 1, in_arm);
                self.fn_head = false;
                if self.neg {
                    // `-` の被演算子が Neg だと `--` と綴られてコメントの印に
                    // 読まれる（E104）。`not` を挟んで根を Neg から外す。
                    self.neg = false;
                    format!("not {operand}")
                } else {
                    self.neg = true;
                    format!("- {operand}")
                }
            }
            1 => {
                let operand = self.unary(depth - 1, in_arm);
                self.neg = false;
                self.fn_head = false;
                format!("not {operand}")
            }
            _ => self.application(depth, in_arm),
        }
    }

    /// `-` の被演算子。単項 `-` を根に置かず、`--` を避ける。
    fn unary_operand(&mut self, depth: usize, in_arm: bool) -> String {
        let choice = if depth == 0 { 2 } else { self.random(2) + 1 };
        self.mark("unary", 3, choice);
        if choice == 1 {
            let inner = self.unary(depth - 1, in_arm);
            self.neg = false;
            self.fn_head = false;
            format!("not {inner}")
        } else {
            self.application(depth, in_arm)
        }
    }

    fn application(&mut self, depth: usize, in_arm: bool) -> String {
        let count = if depth == 0 {
            0
        } else {
            self.choose("application", 3)
        };
        self.mark("application", 3, count);
        if count == 0 {
            return self.postfix(depth, in_arm);
        }
        let head = self.callee(depth - 1, in_arm);
        // 適用の先頭は callee が決める。
        let fn_head = self.fn_head;
        let arguments = self.list(count, |g| g.postfix(depth - 1, in_arm), " ");
        // 適用は根が Neg にならない。
        self.neg = false;
        self.fn_head = fn_head;
        format!("{head} {arguments}")
    }

    fn callee(&mut self, depth: usize, in_arm: bool) -> String {
        let choice = self.choose("callee", 5);
        let callee = match choice {
            0 => self.name(false),
            1 => {
                self.mark("dunder_name", 1, 0);
                self.mark("primary", 15, 7);
                format!("__{}", self.name(false))
            }
            2 => format!("{} . {}", self.upper_reference(), self.name(false)),
            3 => format!("{} . {}", self.name(false), self.name(false)),
            _ => format!("( {} )", self.lambda(depth, in_arm)),
        };
        self.neg = false;
        // ラムダを頭に置くと正準形が `fn` で始まる。
        self.fn_head = choice == 4;
        callee
    }

    fn postfix(&mut self, depth: usize, in_arm: bool) -> String {
        let mut source = self.primary(depth, in_arm);
        let count = self.choose("postfix", 3);
        // `.name` が付くと根はフィールド参照になり Neg ではなくなる。
        if count > 0 {
            self.neg = false;
        }
        for _ in 0..count {
            source.push_str(&format!(" . {}", self.name(false)));
        }
        source
    }

    fn primary(&mut self, depth: usize, in_arm: bool) -> String {
        if self.allow_resume && self.random(3) == 0 {
            self.mark("primary", 15, 14);
            self.neg = false;
            self.fn_head = false;
            return "resume".into();
        }
        let alternatives = if depth == 0 {
            &[0, 1, 2, 3, 4, 5, 6, 8, 9][..]
        } else {
            &[0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13][..]
        };
        let choice = alternatives[self.random(alternatives.len())];
        self.mark("primary", 15, choice);
        // 括弧だけが Neg と `fn` の頭をそのまま通す。他の綴りは根が Neg や
        // `fn` にならない。
        let source = match choice {
            0..=5 => self.literal(choice),
            6 => self.name(false),
            8 => format!("{} . {}", self.upper_reference(), self.name(false)),
            9 if depth == 0 => {
                self.mark("constructor_expr", 4, 0);
                self.upper_reference()
            }
            9 => self.constructor_expr(depth - 1, in_arm),
            10 => self.record_expr(depth - 1, in_arm),
            11 => self.list_expr(depth - 1, in_arm),
            12 => {
                let lambda = self.lambda(depth - 1, in_arm);
                self.neg = false;
                self.fn_head = true;
                return lambda;
            }
            _ => {
                let inner = self.expression(depth - 1, in_arm);
                return format!("( {inner} )");
            }
        };
        self.neg = false;
        self.fn_head = false;
        source
    }

    fn literal(&mut self, choice: usize) -> String {
        const RULES: &[&str] = &["int", "float", "char", "string", "bool", "void"];
        if choice != 4 {
            self.mark(RULES[choice], 1, 0);
        }
        match choice {
            0 => self.random(100000).to_string(),
            1 => format!("{}.{}", self.random(10000), self.random(10000)),
            2 => {
                const CHARS: &[&str] = &["a", "界", " ", "\\n", "\\t", "\\\\", "\\'"];
                let selected = self.random(CHARS.len());
                if selected >= 3 {
                    self.mark("char_escape", CHARS.len() - 3, selected - 3);
                }
                format!("'{}'", CHARS[selected])
            }
            3 => {
                const PARTS: &[&str] = &["text", "界", " ", "\\n", "\\t", "\\\\", "\\\""];
                let count = self.random(4);
                format!(
                    "\"{}\"",
                    self.list(
                        count,
                        |g| {
                            let selected = g.random(PARTS.len());
                            if selected >= 3 {
                                g.mark("string_escape", PARTS.len() - 3, selected - 3);
                            }
                            PARTS[selected].into()
                        },
                        ""
                    )
                )
            }
            4 => {
                if self.choose("bool", 2) == 0 {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            _ => "()".into(),
        }
    }

    fn constructor_expr(&mut self, depth: usize, in_arm: bool) -> String {
        match self.choose("constructor_expr", 4) {
            0 => return self.upper_reference(),
            1 => return "Ok ()".into(),
            2 => {
                return format!(
                    "{} {}",
                    self.upper_reference(),
                    self.record_expr(depth, in_arm)
                )
            }
            _ => {}
        }
        let count = self.random(3) + 1;
        format!(
            "{} ( {} )",
            self.upper_reference(),
            self.list(count, |g| g.expression(depth, in_arm), " , ")
        )
    }

    fn record_expr(&mut self, depth: usize, in_arm: bool) -> String {
        self.mark("record_expr", 1, 0);
        let count = self.random(3) + 1;
        format!(
            "{{ {} }}",
            self.list(
                count,
                |g| format!("{} = ( {} )", g.name(false), g.expression(depth, in_arm)),
                " ; "
            )
        )
    }

    fn list_expr(&mut self, depth: usize, in_arm: bool) -> String {
        let count = self.choose("list_expr", 3);
        let mut items = self.list(count, |g| g.expression(depth, in_arm), " , ");
        if count > 0 && self.choose("list_tail?", 2) == 1 {
            items.push_str(&format!(", ...{}", self.expression(depth, in_arm)));
        }
        format!("[ {items} ]")
    }

    fn branch(&mut self, depth: usize) -> String {
        let source = match self.choose("branch", 3) {
            0 => {
                self.mark("boolean_branch", 1, 0);
                let condition = self.disregard(|g| g.pipe(depth, true));
                let (then, then_arrow) = self.capture(|g| g.arm_body(depth));
                let (els, els_arrow) = self.capture(|g| g.arm_body(depth));
                format!(
                    "if {condition} => {then} | {els}{}",
                    if then_arrow || els_arrow { " /if" } else { "" }
                )
            }
            1 => {
                self.mark("condition_branch", 1, 0);
                let (arms, arrow) = self.capture(|g| g.condition_arms(depth));
                format!("if {arms}{}", if arrow { " /if" } else { "" })
            }
            _ => {
                self.mark("pattern_branch", 1, 0);
                let columns = if self.choose("scrutinee_column*", 2) == 0 {
                    1
                } else {
                    self.random(2) + 2
                };
                let scrutinees =
                    self.disregard(|g| g.list(columns, |h| h.pipe(depth, true), " , "));
                let (arms, arrow) = self.capture(|g| g.pattern_arms(depth, columns));
                format!("if {scrutinees} {arms}{}", if arrow { " /if" } else { "" })
            }
        };
        // 分岐は根が Neg にならず、`fn` で始まらない。
        self.neg = false;
        self.fn_head = false;
        source
    }

    fn condition_arms(&mut self, depth: usize) -> String {
        self.mark("condition_arms", 1, 0);
        let count = self.random(3) + 1;
        let arms = self.list(
            count,
            |g| {
                g.mark("condition_arm", 1, 0);
                let condition = g.disregard(|h| h.pipe(depth, true));
                let body = g.arm_body(depth);
                format!("| {condition} => {body}")
            },
            " ",
        );
        let fallback = self.arm_body(depth);
        format!("{arms} | {fallback}")
    }

    fn pattern_arms(&mut self, depth: usize, columns: usize) -> String {
        self.mark("pattern_arms", 1, 0);
        let count = self.random(3) + 1;
        let arms = self.list(count, |g| g.pattern_arm(depth, columns), " ");
        if self.choose("pattern_fallback?", 2) == 0 {
            return arms;
        }
        self.mark("pattern_fallback", 1, 0);
        format!("{arms} | {}", self.arm_body(depth))
    }

    fn arms(&mut self, depth: usize) -> String {
        self.pattern_arms(depth, 1)
    }

    fn pattern_arm(&mut self, depth: usize, columns: usize) -> String {
        self.mark("pattern_column*", 2, usize::from(columns > 1));
        let patterns = self.list(columns, |g| g.pattern(2), " , ");
        let arm_conditions = self.choose("pattern_arm", 2) == 1;
        // `fn(| … )` の腕に条件列を置くと、正準形が `fn( | p => if(…))` に
        // なって入れ子の分岐（E101）に読まれる。lambda の中では腕の本体に限る。
        let body = if arm_conditions && self.sealed_depth == 0 {
            self.arm_conditions(depth)
        } else {
            self.arm_body(depth)
        };
        format!("| {patterns} => {body}")
    }

    fn arm_conditions(&mut self, depth: usize) -> String {
        self.mark("arm_conditions", 1, 0);
        // 腕の条件列は素の `if` で終わり、閉じの印を置けない。中に `->` を
        // 綴ると正準形が `/if` を足して往復しないため、ここでは綴らない。
        self.sealed_depth += 1;
        let conditions = self.condition_arms(depth);
        self.sealed_depth -= 1;
        format!("if {conditions}")
    }

    fn arm_body(&mut self, depth: usize) -> String {
        let choice = if depth == 0 || self.sealed_depth > 0 {
            0
        } else {
            self.random(3)
        };
        self.mark("arm_body", 3, choice);
        if choice == 0 {
            self.pipe(depth, true)
        } else {
            self.prefix_variant(depth - 1, true, choice - 1)
        }
    }

    fn pattern(&mut self, depth: usize) -> String {
        let choice = if depth == 0 {
            self.random(2)
        } else {
            self.random(6)
        };
        self.mark("pattern", 6, choice);
        match choice {
            0 => "_".into(),
            1 => self.name(false),
            2 => {
                let name = self.upper_reference();
                match self.choose("constructor_pattern_form", 3) {
                    0 => name,
                    1 => {
                        let count = self.random(2) + 1;
                        format!(
                            "{name} ( {} )",
                            self.list(count, |g| g.pattern(depth - 1), " , ")
                        )
                    }
                    _ => format!("{name} {}", self.record_pattern(depth - 1)),
                }
            }
            3 => self.record_pattern(depth - 1),
            5 => self.list_pattern(depth - 1),
            _ => self.literal_pattern(),
        }
    }

    fn list_pattern(&mut self, depth: usize) -> String {
        let count = self.choose("list_pattern", 3);
        let mut items = self.list(count, |g| g.pattern(depth), " , ");
        if count > 0 && self.choose("pattern_tail?", 2) == 1 {
            let rest = if self.random(2) == 0 {
                "_".into()
            } else {
                self.name(false)
            };
            items.push_str(&format!(", ...{rest}"));
        }
        format!("[ {items} ]")
    }

    fn record_pattern(&mut self, depth: usize) -> String {
        self.mark("record_pattern", 1, 0);
        let count = self.random(3) + 1;
        format!(
            "{{ {} }}",
            self.list(
                count,
                |g| {
                    let name = g.name(false);
                    if g.choose("field_pattern", 2) == 0 {
                        name
                    } else {
                        format!("{name} = {}", g.pattern(depth))
                    }
                },
                " ; "
            )
        )
    }

    fn literal_pattern(&mut self) -> String {
        let choice = self.choose("literal_pattern", 7);
        if choice < 5 {
            self.literal(choice)
        } else {
            format!("- {}", self.literal(choice - 5))
        }
    }

    fn perturb(&mut self, source: &str) -> String {
        tokens(source)
            .into_iter()
            .map(|token| {
                let separator = match self.random(3) {
                    0 => " ".into(),
                    1 => "\n\t".into(),
                    _ => {
                        self.mark("comment", 1, 0);
                        format!(" // generated {}\n", self.random(10000))
                    }
                };
                format!("{token}{separator}")
            })
            .collect()
    }
}

fn tokens(source: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote.is_some() && character == '\\' {
            escaped = true;
            continue;
        }
        if quote == Some(character) {
            quote = None;
            continue;
        }
        if quote.is_some() {
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
        }
        if character.is_whitespace() {
            if start < index {
                tokens.push(&source[start..index]);
            }
            start = index + character.len_utf8();
        }
    }
    if start < source.len() {
        tokens.push(&source[start..]);
    }
    tokens
}

#[derive(Debug, PartialEq)]
enum Shape {
    Definition(String, bool, Option<Vec<Param>>, Option<TypeExpr>, Node),
    SignatureDefinition(String, Vec<String>, Vec<Param>, Option<TypeExpr>),
    Type(String, Vec<String>, Option<TypeExpr>, bool),
    Signature(String, Vec<Shape>),
    Module(String, String, ImplementationShape, Vec<Shape>),
    Effect(
        String,
        Vec<String>,
        Vec<(String, String, Vec<Param>, Option<TypeExpr>)>,
    ),
    Handler(Vec<String>, String, String, Vec<Param>, Vec<HandlerShape>),
}

#[derive(Debug, PartialEq)]
enum HandlerShape {
    Operation(String, String, Vec<String>, Node),
    Return(String, Node),
}

#[derive(Debug, PartialEq)]
enum ImplementationShape {
    Ordinary,
    Functor(Vec<(String, String)>),
    Application(String, Vec<String>),
}

fn implementation_shape(form: &ImplementationForm) -> ImplementationShape {
    match form {
        ImplementationForm::Ordinary => ImplementationShape::Ordinary,
        ImplementationForm::Functor(params) => ImplementationShape::Functor(
            params
                .iter()
                .map(|param| (param.name.clone(), param.contract.clone()))
                .collect(),
        ),
        ImplementationForm::Application { functor, arguments } => ImplementationShape::Application(
            functor.name.clone(),
            arguments
                .iter()
                .map(|argument| argument.name.clone())
                .collect(),
        ),
    }
}

fn shapes(items: &[Item]) -> Vec<Shape> {
    items
        .iter()
        .map(|item| match item {
            Item::Def(def) => match &def.kind {
                DefKind::Value(body) => {
                    Shape::Definition(def.name.clone(), def.rec, None, None, body.clone())
                }
                DefKind::Function { params, ret, body } => Shape::Definition(
                    def.name.clone(),
                    def.rec,
                    Some(params.clone()),
                    ret.clone(),
                    body.clone(),
                ),
            },
            Item::SigDef(def) => Shape::SignatureDefinition(
                def.name.clone(),
                def.type_params
                    .iter()
                    .map(|name| name.name.clone())
                    .collect(),
                def.params.clone(),
                def.ret.clone(),
            ),
            Item::TypeDef(def) => Shape::Type(
                def.name.clone(),
                def.params.iter().map(|name| name.name.clone()).collect(),
                def.ty.clone(),
                def.shared,
            ),
            Item::ModuleSig(sig) => Shape::Signature(sig.name.clone(), shapes(&sig.items)),
            Item::ModuleImpl(module) => Shape::Module(
                module.name.clone(),
                module.sig.clone(),
                implementation_shape(&module.form),
                shapes(&module.items),
            ),
            Item::Effect(effect) => Shape::Effect(
                effect.name.clone(),
                effect
                    .type_params
                    .iter()
                    .map(|name| name.name.clone())
                    .collect(),
                effect
                    .ops
                    .iter()
                    .map(|op| {
                        (
                            op.kind.as_str().into(),
                            op.name.clone(),
                            op.params.clone(),
                            op.ret.clone(),
                        )
                    })
                    .collect(),
            ),
            Item::Handler(handler) => Shape::Handler(
                handler
                    .type_params
                    .iter()
                    .map(|name| name.name.clone())
                    .collect(),
                handler.effect.clone(),
                handler.name.clone(),
                handler.params.clone(),
                handler
                    .arms
                    .iter()
                    .map(|arm| match arm {
                        parser::HandlerArm::Operation {
                            kind,
                            name,
                            params,
                            body,
                            ..
                        } => HandlerShape::Operation(
                            kind.as_str().into(),
                            name.clone(),
                            params.iter().map(|param| param.name.clone()).collect(),
                            body.clone(),
                        ),
                        parser::HandlerArm::Return { param, body, .. } => {
                            HandlerShape::Return(param.name.clone(), body.clone())
                        }
                    })
                    .collect(),
            ),
        })
        .collect()
}

fn declaration_count(items: &[Item]) -> usize {
    items
        .iter()
        .map(|item| {
            1 + match item {
                Item::ModuleSig(sig) => declaration_count(&sig.items),
                Item::ModuleImpl(module) => declaration_count(&module.items),
                _ => 0,
            }
        })
        .sum()
}

fn shared_counts(items: &[Item]) -> (usize, usize) {
    let mut ordinary = 0;
    let mut functor = 0;
    for item in items {
        match item {
            Item::ModuleSig(sig) => {
                let (nested_ordinary, nested_functor) = shared_counts(&sig.items);
                ordinary += nested_ordinary;
                functor += nested_functor;
            }
            Item::ModuleImpl(module) => {
                let direct = module
                    .items
                    .iter()
                    .filter(|item| matches!(item, Item::TypeDef(def) if def.shared))
                    .count();
                match module.form {
                    ImplementationForm::Ordinary => ordinary += direct,
                    ImplementationForm::Functor(_) => functor += direct,
                    ImplementationForm::Application { .. } => {}
                }
                let (nested_ordinary, nested_functor) = shared_counts(&module.items);
                ordinary += nested_ordinary;
                functor += nested_functor;
            }
            _ => {}
        }
    }
    (ordinary, functor)
}

#[test]
fn grammar_generated_programs_preserve_every_declaration() {
    let mut generator = Generator {
        state: 0x4152_395f_4752_414d,
        ..Generator::default()
    };
    let mut library_seen = false;
    let mut ordinary_shared_seen = false;
    let mut functor_shared_seen = false;
    for sample in 0..1200 {
        let source = generator.program();
        let ast = parser::parse_source(&source, "generated-grammar.hd")
            .unwrap_or_else(|error| panic!("sample {sample}: {error}\n{source}"));
        library_seen |= ast.library.is_some();
        let (ordinary_shared, functor_shared) = shared_counts(&ast.items);
        ordinary_shared_seen |= ordinary_shared > 0;
        functor_shared_seen |= functor_shared > 0;
        assert_eq!(
            declaration_count(&ast.items),
            generator.declarations,
            "sample {sample}: {source}"
        );
        let spaced = generator.perturb(&source);
        let spaced_ast = parser::parse_source(&spaced, "spaced-grammar.hd")
            .unwrap_or_else(|error| panic!("sample {sample}: {error}\n{spaced}"));
        assert_eq!(
            shapes(&ast.items),
            shapes(&spaced_ast.items),
            "sample {sample}: {source}\n{spaced}"
        );
        let canonical = parser::canonical_program(&ast);
        let restored = parser::parse_source(&canonical, "canonical-grammar.hd")
            .unwrap_or_else(|error| panic!("sample {sample}: {error}\n{source}\n{canonical}"));
        assert_eq!(
            shapes(&ast.items),
            shapes(&restored.items),
            "sample {sample}: {source}\n{canonical}"
        );
        assert_eq!(canonical, parser::canonical_program(&restored));
    }
    assert!(library_seen, "generator never emitted library");
    assert!(
        ordinary_shared_seen,
        "generator never emitted shared in ordinary implementation"
    );
    assert!(
        functor_shared_seen,
        "generator never emitted shared in functor body"
    );
    let spec = include_str!("../../../docs/spec.md");
    let grammar = spec.split("## 付録C ").nth(1).unwrap();
    let rules: BTreeSet<_> = grammar
        .lines()
        .filter_map(|line| line.split_once(":=").map(|(name, _)| name.trim()))
        .collect();
    let visited: BTreeSet<_> = generator.choices.keys().copied().collect();
    assert!(
        rules.difference(&visited).next().is_none(),
        "unvisited rules: {:?}",
        rules.difference(&visited).collect::<Vec<_>>()
    );
    let missing: Vec<_> = generator
        .choices
        .iter()
        .filter(|(_, choices)| choices.contains(&false))
        .collect();
    assert!(missing.is_empty(), "unvisited alternatives: {missing:?}");
}
