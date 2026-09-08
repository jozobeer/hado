use std::collections::{BTreeMap, BTreeSet};

use hado::parser::{self, DefKind, ImplementationForm, Item, Node, Param, TypeExpr};

#[derive(Default)]
struct Generator {
    state: u64,
    serial: usize,
    choices: BTreeMap<&'static str, Vec<bool>>,
    declarations: usize,
}

impl Generator {
    fn random(&mut self, limit: usize) -> usize {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.state >> 32) as usize) % limit
    }

    fn choose(&mut self, rule: &'static str, count: usize) -> usize {
        let choice = self.random(count);
        self.mark(rule, count, choice);
        choice
    }

    fn mark(&mut self, rule: &'static str, count: usize, choice: usize) {
        let visited = self.choices.entry(rule).or_insert_with(|| vec![false; count]);
        assert_eq!(visited.len(), count, "inconsistent alternatives for {rule}");
        visited[choice] = true;
    }

    fn name(&mut self, upper: bool) -> String {
        self.mark(if upper { "upper_name" } else { "name" }, 1, 0);
        self.serial += 1;
        let suffix = self.random(10000);
        format!("{}{:_>3}_{suffix}", if upper { "Type" } else { "value" }, self.serial)
    }

    fn list(&mut self, count: usize, mut element: impl FnMut(&mut Self) -> String, separator: &str) -> String {
        (0..count).map(|_| element(self)).collect::<Vec<_>>().join(separator)
    }

    fn program(&mut self) -> String {
        self.declarations = 0;
        self.mark("program", 1, 0);
        let outer_count = self.choose("outer_item*", 3);
        let section_count = self.choose("section*", 3);
        let outer = self.list(outer_count, |g| g.outer_item(), "\n");
        let sections = self.list(section_count, |g| g.section(), "\n");
        format!("{outer}\n{sections}")
    }

    fn outer_item(&mut self) -> String {
        match self.choose("outer_item", 2) {
            0 => self.type_definition(),
            _ => self.definition(),
        }
    }

    fn section(&mut self) -> String {
        match self.choose("section", 4) {
            0 => self.module_section(false).0,
            1 => self.implement_section(),
            2 => self.functor_section().0,
            _ => self.apply_section(),
        }
    }

    fn definition(&mut self) -> String {
        self.declarations += 1;
        let name = self.name(false);
        match self.choose("definition", 4) {
            0 => format!("def {name} = {}", self.expression(3, false)),
            1 => format!("def {}{name} {} {} => {}", self.rec(), self.parameters(), self.return_type(), self.expression(3, false)),
            2 => format!("def {}{name} {}", self.rec(), self.arms(2)),
            _ => format!("def {}{name} {} {} {}", self.rec(), self.parameters(), self.return_type(), self.condition_arms(2)),
        }
    }

    fn rec(&mut self) -> &'static str {
        if self.choose("rec?", 2) == 0 { "" } else { "rec " }
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
        if self.choose("return_type?", 2) == 0 { return String::new(); }
        self.mark("return_type", 1, 0);
        format!(": {}", self.type_expr(2))
    }

    fn type_params(&mut self) -> String {
        if self.choose("type_vars?", 2) == 0 { return String::new(); }
        let count = self.random(3) + 1;
        self.mark("type_vars", 2, usize::from(count > 1));
        if count == 1 { self.name(true) } else { format!("( {} )", self.list(count, |g| g.name(true), " , ")) }
    }

    fn type_definition(&mut self) -> String {
        self.declarations += 1;
        let name = self.name(true);
        let params = self.type_params();
        let body = if self.choose("type_definition", 2) == 0 {
            let count = self.random(3) + 1;
            self.list(count, |g| format!("| {}", g.constructor()), " ")
        } else { self.record_type() };
        format!("type {params} {name} = {body}")
    }

    fn constructor(&mut self) -> String {
        let name = self.name(true);
        if self.choose("constructor", 2) == 0 { return name; }
        let count = self.random(3) + 1;
        format!("{name} ( {} )", self.list(count, |g| g.type_expr(2), " , "))
    }

    fn record_type(&mut self) -> String {
        self.mark("record_type", 1, 0);
        let count = self.random(3) + 1;
        format!("{{ {} }}", self.list(count, |g| {
            g.mark("field_type", 1, 0);
            format!("{} : {}", g.name(false), g.type_expr(2))
        }, " ; "))
    }

    fn type_expr(&mut self, depth: usize) -> String {
        self.mark("type_expr", 1, 0);
        let choice = if depth == 0 { 0 } else { self.random(3) };
        self.mark("type_atom", 3, choice);
        let mut source = match choice {
            0 => self.type_ref(),
            1 => {
                let count = self.random(3) + 2;
                format!("( {} ) {}", self.list(count, |g| g.type_expr(depth - 1), " , "), self.type_ref())
            }
            _ => format!("Fn ( {} => {} )", self.fn_params(depth - 1), self.type_expr(depth - 1)),
        };
        let count = self.choose("type_ref*", 3);
        for _ in 0..count { source.push_str(&format!(" {}", self.type_ref())); }
        source
    }

    fn type_ref(&mut self) -> String {
        if self.choose("type_ref", 2) == 0 { self.name(true) }
        else { format!("{} . {}", self.name(true), self.name(true)) }
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
        (format!("module {name} {items}"), name, abstract_name)
    }

    fn module_item(&mut self) -> String {
        self.declarations += 1;
        if self.choose("module_item", 2) == 0 { return format!("type {}", self.name(true)); }
        let name = self.name(false);
        let params = self.type_params();
        let count = self.random(3) + 1;
        format!("def {params} {name} ( {} ) : {}", self.list(count, |g| { g.mark("contract_param", 1, 0); g.typed_param() }, " , "), self.type_expr(2))
    }

    fn implement_section(&mut self) -> String {
        self.mark("implement_section", 1, 0);
        let (signature, sig_name, abstract_name) = self.module_section(true);
        self.mark("contract_ref", 1, 0);
        self.declarations += 1;
        let count = self.choose("implement_item*", 3);
        let items = self.list(count, |g| g.implement_item(&abstract_name), " ");
        format!("{signature}\nimplement {sig_name} as {} {items}", self.name(true))
    }

    fn functor_section(&mut self) -> (String, String, String) {
        self.mark("functor_section", 1, 0);
        let (signature, sig_name, abstract_name) = self.module_section(true);
        self.mark("contract_ref", 1, 0);
        let name = self.name(true);
        let count = self.choose("functor_param+", 3) + 1;
        let params = self.list(count, |g| {
            g.mark("functor_param", 1, 0);
            g.mark("contract_ref", 1, 0);
            format!("{} : {sig_name}", g.name(true))
        }, " , ");
        self.declarations += 2;
        (format!("{signature}\nimplement {sig_name} as {name} ( {params} ) type {abstract_name} = Int"), sig_name, name)
    }

    fn apply_section(&mut self) -> String {
        self.mark("apply_section", 1, 0);
        let (definition, signature, functor) = self.functor_section();
        self.mark("contract_ref", 1, 0);
        self.mark("impl_ref", 1, 0);
        let count = self.choose("impl_ref+", 3) + 1;
        let arguments = self.list(count, |g| { g.mark("impl_ref", 1, 0); g.name(true) }, " , ");
        self.declarations += 1;
        format!("{definition}\nimplement {signature} as {} = {functor} ( {arguments} )", self.name(true))
    }

    fn implement_item(&mut self, abstract_name: &str) -> String {
        match self.choose("implement_item", 3) {
            0 => self.definition(),
            1 => self.type_definition(),
            _ => {
                self.declarations += 1;
                format!("type {abstract_name} = {}", self.type_expr(2))
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
        let choice = self.random(if in_arm { 3 } else { 4 });
        self.prefix_variant(depth, in_arm, choice)
    }

    fn prefix_variant(&mut self, depth: usize, in_arm: bool, choice: usize) -> String {
        self.mark("prefix_form", 4, choice);
        match choice {
            0 | 1 => {
                self.mark(if choice == 0 { "binding" } else { "rail_binding" }, 1, 0);
                format!("let {}{} = ( {} ) -> {}", self.pattern(2), if choice == 0 { "" } else { "?" }, self.expression(depth, in_arm), self.expression(depth, in_arm))
            }
            2 => {
                self.mark("rail_discard", 1, 0);
                format!("do ( {} ) -> {}", self.expression(depth, in_arm), self.expression(depth, in_arm))
            }
            _ => self.branch(depth),
        }
    }

    fn lambda(&mut self, depth: usize, in_arm: bool) -> String {
        let choice = if in_arm { self.mark("lambda", 3, 0); 0 } else { self.choose("lambda", 3) };
        if choice == 0 {
            format!("fn ( {} => {} )", self.lambda_params(), self.expression(depth, in_arm))
        } else if choice == 1 {
            format!("fn ( {} )", self.arms(depth))
        } else {
            format!("fn ( {} {} )", self.lambda_params(), self.condition_arms(depth))
        }
    }

    fn lambda_params(&mut self) -> String {
        let count = self.random(3) + 1;
        if self.choose("lambda_params", 2) == 0 {
            self.list(count, |g| { g.mark("param", 1, 0); g.name(false) }, " , ")
        } else { self.list(count, |g| g.typed_param(), " , ") }
    }

    fn pipe(&mut self, depth: usize, in_arm: bool) -> String {
        let mut source = self.binary(0, depth, in_arm);
        let count = if depth == 0 { 0 } else { self.choose("pipe", 3) };
        self.mark("pipe", 3, count);
        for _ in 0..count {
            source.push_str(&format!(" |> {}", self.pipe_target(depth - 1, in_arm)));
        }
        source
    }

    fn pipe_target(&mut self, depth: usize, in_arm: bool) -> String {
        let head = if self.choose("pipe_callee", 2) == 0 { self.name(false) }
            else { format!("{} . {}", self.name(true), self.name(false)) };
        let count = self.choose("pipe_arguments", 3);
        format!("{head} {}", self.list(count, |g| g.primary(depth, in_arm), " "))
    }

    fn binary(&mut self, level: usize, depth: usize, in_arm: bool) -> String {
        const RULES: &[(&str, &[&str])] = &[
            ("or", &["||"]), ("and", &["&&"]), ("compare", &["<", "<=", ">", ">=", "==", "!="]),
            ("additive", &["+", "-"]), ("multiplicative", &["*", "/", "%"]),
        ];
        if level == RULES.len() { return self.unary(depth, in_arm); }
        let (rule, operators) = RULES[level];
        let mut source = self.binary(level + 1, depth, in_arm);
        let count = if depth == 0 || self.random(4) != 0 { 0 } else if rule == "compare" { 1 } else { self.random(2) + 1 };
        self.mark(["or*", "and*", "compare?", "additive*", "multiplicative*"][level], if rule == "compare" { 2 } else { 3 }, count);
        if count == 0 { self.mark(rule, operators.len() + 1, 0); }
        for _ in 0..count {
            let operator = self.random(operators.len());
            self.mark(rule, operators.len() + 1, operator + 1);
            source.push_str(&format!(" {} {}", operators[operator], self.binary(level + 1, depth - 1, in_arm)));
        }
        source
    }

    fn unary(&mut self, depth: usize, in_arm: bool) -> String {
        let choice = if depth == 0 { 2 } else { self.random(3) };
        self.mark("unary", 3, choice);
        match choice {
            0 => format!("- {}", self.unary(depth - 1, in_arm)),
            1 => format!("not {}", self.unary(depth - 1, in_arm)),
            _ => self.application(depth, in_arm),
        }
    }

    fn application(&mut self, depth: usize, in_arm: bool) -> String {
        let count = if depth == 0 { 0 } else { self.choose("application", 3) };
        self.mark("application", 3, count);
        if count == 0 { return self.postfix(depth, in_arm); }
        let head = self.callee(depth - 1, in_arm);
        format!("{head} {}", self.list(count, |g| g.postfix(depth - 1, in_arm), " "))
    }

    fn callee(&mut self, depth: usize, in_arm: bool) -> String {
        match self.choose("callee", 5) {
            0 => self.name(false),
            1 => {
                self.mark("dunder_name", 1, 0);
                self.mark("primary", 14, 7);
                format!("__{}", self.name(false))
            }
            2 => format!("{} . {}", self.name(true), self.name(false)),
            3 => format!("{} . {}", self.name(false), self.name(false)),
            _ => format!("( {} )", self.lambda(depth, in_arm)),
        }
    }

    fn postfix(&mut self, depth: usize, in_arm: bool) -> String {
        let mut source = self.primary(depth, in_arm);
        let count = self.choose("postfix", 3);
        for _ in 0..count { source.push_str(&format!(" . {}", self.name(false))); }
        source
    }

    fn primary(&mut self, depth: usize, in_arm: bool) -> String {
        let alternatives = if depth == 0 { &[0, 1, 2, 3, 4, 5, 6, 8, 9][..] }
            else { &[0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13][..] };
        let choice = alternatives[self.random(alternatives.len())];
        self.mark("primary", 14, choice);
        match choice {
            0..=5 => self.literal(choice),
            6 => self.name(false),
            8 => format!("{} . {}", self.name(true), self.name(false)),
            9 if depth == 0 => { self.mark("constructor_expr", 3, 0); self.name(true) }
            9 => self.constructor_expr(depth - 1, in_arm),
            10 => self.record_expr(depth - 1, in_arm),
            11 => self.list_expr(depth - 1, in_arm),
            12 => self.lambda(depth - 1, in_arm),
            _ => format!("( {} )", self.expression(depth - 1, in_arm)),
        }
    }

    fn literal(&mut self, choice: usize) -> String {
        const RULES: &[&str] = &["int", "float", "char", "string", "bool", "unit"];
        if choice != 4 { self.mark(RULES[choice], 1, 0); }
        match choice {
            0 => self.random(100000).to_string(),
            1 => format!("{}.{}", self.random(10000), self.random(10000)),
            2 => {
                const CHARS: &[&str] = &["a", "界", " ", "\\n", "\\t", "\\\\", "\\'"];
                let selected = self.random(CHARS.len());
                if selected >= 3 { self.mark("char_escape", CHARS.len() - 3, selected - 3); }
                format!("'{}'", CHARS[selected])
            }
            3 => {
                const PARTS: &[&str] = &["text", "界", " ", "\\n", "\\t", "\\\\", "\\\""];
                let count = self.random(4);
                format!("\"{}\"", self.list(count, |g| {
                    let selected = g.random(PARTS.len());
                    if selected >= 3 { g.mark("string_escape", PARTS.len() - 3, selected - 3); }
                    PARTS[selected].into()
                }, ""))
            }
            4 => if self.choose("bool", 2) == 0 { "true".into() } else { "false".into() },
            _ => "()".into(),
        }
    }

    fn constructor_expr(&mut self, depth: usize, in_arm: bool) -> String {
        match self.choose("constructor_expr", 3) {
            0 => return self.name(true),
            1 => return "Ok ()".into(),
            _ => {}
        }
        let count = self.random(3) + 1;
        format!("{} ( {} )", self.name(true), self.list(count, |g| g.expression(depth, in_arm), " , "))
    }

    fn record_expr(&mut self, depth: usize, in_arm: bool) -> String {
        self.mark("record_expr", 1, 0);
        let count = self.random(3) + 1;
        format!("{{ {} }}", self.list(count, |g| format!("{} = ( {} )", g.name(false), g.expression(depth, in_arm)), " ; "))
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
        match self.choose("branch", 3) {
            0 => {
                self.mark("boolean_branch", 1, 0);
                format!("if {} => {} | {}", self.pipe(depth, true), self.arm_body(depth), self.arm_body(depth))
            }
            1 => {
                self.mark("condition_branch", 1, 0);
                format!("if {}", self.condition_arms(depth))
            }
            _ => {
                self.mark("pattern_branch", 1, 0);
                format!("if {} {}", self.pipe(depth, true), self.arms(depth))
            }
        }
    }

    fn condition_arms(&mut self, depth: usize) -> String {
        self.mark("condition_arms", 1, 0);
        let count = self.random(3) + 1;
        let arms = self.list(count, |g| {
            g.mark("condition_arm", 1, 0);
            format!("| {} => {}", g.pipe(depth, true), g.arm_body(depth))
        }, " ");
        format!("{arms} | {}", self.arm_body(depth))
    }

    fn arms(&mut self, depth: usize) -> String {
        self.mark("pattern_arms", 1, 0);
        let count = self.random(3) + 1;
        let arms = self.list(count, |g| {
            g.mark("pattern_arm", 1, 0);
            format!("| {} => {}", g.pattern(2), g.arm_body(depth))
        }, " ");
        if self.choose("pattern_fallback?", 2) == 0 { return arms; }
        self.mark("pattern_fallback", 1, 0);
        format!("{arms} | {}", self.arm_body(depth))
    }

    fn arm_body(&mut self, depth: usize) -> String {
        let choice = if depth == 0 { 0 } else { self.random(4) };
        self.mark("arm_body", 4, choice);
        if choice == 0 { self.pipe(depth, true) } else { self.prefix_variant(depth - 1, true, choice - 1) }
    }

    fn pattern(&mut self, depth: usize) -> String {
        let choice = if depth == 0 { self.random(2) } else { self.random(6) };
        self.mark("pattern", 6, choice);
        match choice {
            0 => "_".into(),
            1 => self.name(false),
            2 => {
                let name = self.name(true);
                let count = self.random(3);
                if count == 0 { name } else { format!("{name} ( {} )", self.list(count, |g| g.pattern(depth - 1), " , ")) }
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
            let rest = if self.random(2) == 0 { "_".into() } else { self.name(false) };
            items.push_str(&format!(", ...{rest}"));
        }
        format!("[ {items} ]")
    }

    fn record_pattern(&mut self, depth: usize) -> String {
        self.mark("record_pattern", 1, 0);
        let count = self.random(3) + 1;
        format!("{{ {} }}", self.list(count, |g| {
            let name = g.name(false);
            if g.choose("field_pattern", 2) == 0 { name } else { format!("{name} = {}", g.pattern(depth)) }
        }, " ; "))
    }

    fn literal_pattern(&mut self) -> String {
        let choice = self.choose("literal_pattern", 7);
        if choice < 5 { self.literal(choice) } else { format!("- {}", self.literal(choice - 5)) }
    }

    fn perturb(&mut self, source: &str) -> String {
        tokens(source).into_iter().map(|token| {
            let separator = match self.random(3) {
                0 => " ".into(),
                1 => "\n\t".into(),
                _ => {
                    self.mark("comment", 1, 0);
                    format!(" // generated {}\n", self.random(10000))
                }
            };
            format!("{token}{separator}")
        }).collect()
    }
}

fn tokens(source: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in source.char_indices() {
        if escaped { escaped = false; continue; }
        if quote.is_some() && character == '\\' { escaped = true; continue; }
        if quote == Some(character) { quote = None; continue; }
        if quote.is_some() { continue; }
        if character == '\'' || character == '"' { quote = Some(character); }
        if character.is_whitespace() {
            if start < index { tokens.push(&source[start..index]); }
            start = index + character.len_utf8();
        }
    }
    if start < source.len() { tokens.push(&source[start..]); }
    tokens
}

#[derive(Debug, PartialEq)]
enum Shape {
    Definition(String, bool, Option<Vec<Param>>, Option<TypeExpr>, Node),
    SignatureDefinition(String, Vec<String>, Vec<Param>, Option<TypeExpr>),
    Type(String, Vec<String>, Option<TypeExpr>),
    Signature(String, Vec<Shape>),
    Module(String, String, ImplementationShape, Vec<Shape>),
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
        ImplementationForm::Functor(params) => ImplementationShape::Functor(params.iter().map(|param| (param.name.clone(), param.contract.clone())).collect()),
        ImplementationForm::Application { functor, arguments } => ImplementationShape::Application(functor.name.clone(), arguments.iter().map(|argument| argument.name.clone()).collect()),
    }
}

fn shapes(items: &[Item]) -> Vec<Shape> {
    items.iter().map(|item| match item {
        Item::Def(def) => match &def.kind {
            DefKind::Value(body) => Shape::Definition(def.name.clone(), def.rec, None, None, body.clone()),
            DefKind::Function { params, ret, body } => Shape::Definition(def.name.clone(), def.rec, Some(params.clone()), ret.clone(), body.clone()),
        },
        Item::SigDef(def) => Shape::SignatureDefinition(def.name.clone(), def.type_params.iter().map(|name| name.name.clone()).collect(), def.params.clone(), def.ret.clone()),
        Item::TypeDef(def) => Shape::Type(def.name.clone(), def.params.iter().map(|name| name.name.clone()).collect(), def.ty.clone()),
        Item::ModuleSig(sig) => Shape::Signature(sig.name.clone(), shapes(&sig.items)),
        Item::ModuleImpl(module) => Shape::Module(module.name.clone(), module.sig.clone(), implementation_shape(&module.form), shapes(&module.items)),
    }).collect()
}

fn declaration_count(items: &[Item]) -> usize {
    items.iter().map(|item| 1 + match item {
        Item::ModuleSig(sig) => declaration_count(&sig.items),
        Item::ModuleImpl(module) => declaration_count(&module.items),
        _ => 0,
    }).sum()
}

#[test]
fn grammar_generated_programs_preserve_every_declaration() {
    let mut generator = Generator { state: 0x4152_395f_4752_414d, ..Generator::default() };
    for sample in 0..1200 {
        let source = generator.program();
        let ast = parser::parse_source(&source, "generated-grammar.hd")
            .unwrap_or_else(|error| panic!("sample {sample}: {error}\n{source}"));
        assert_eq!(declaration_count(&ast.items), generator.declarations, "sample {sample}: {source}");
        let spaced = generator.perturb(&source);
        let spaced_ast = parser::parse_source(&spaced, "spaced-grammar.hd")
            .unwrap_or_else(|error| panic!("sample {sample}: {error}\n{spaced}"));
        assert_eq!(shapes(&ast.items), shapes(&spaced_ast.items), "sample {sample}: {source}\n{spaced}");
        let canonical = parser::canonical_program(&ast);
        let restored = parser::parse_source(&canonical, "canonical-grammar.hd")
            .unwrap_or_else(|error| panic!("sample {sample}: {error}\n{source}\n{canonical}"));
        assert_eq!(shapes(&ast.items), shapes(&restored.items), "sample {sample}: {source}\n{canonical}");
        assert_eq!(canonical, parser::canonical_program(&restored));
    }
    let spec = include_str!("../../../docs/spec.md");
    let grammar = spec.split("# 41.").nth(1).unwrap().split("# 42.").next().unwrap();
    let rules: BTreeSet<_> = grammar.lines().filter_map(|line| line.split_once(":=").map(|(name, _)| name.trim())).collect();
    let visited: BTreeSet<_> = generator.choices.keys().copied().collect();
    assert!(rules.difference(&visited).next().is_none(), "unvisited rules: {:?}", rules.difference(&visited).collect::<Vec<_>>());
    let missing: Vec<_> = generator.choices.iter().filter(|(_, choices)| choices.contains(&false)).collect();
    assert!(missing.is_empty(), "unvisited alternatives: {missing:?}");
}
