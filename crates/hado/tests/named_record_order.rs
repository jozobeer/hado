use hado::mono::{Constant, Expression, FunctionId, Program};
use hado::parser::BinOp;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Int(i64),
    Construct(String, Vec<Value>),
}
struct Interpreter<'a> {
    program: &'a Program,
    calls: Vec<String>,
}
impl Interpreter<'_> {
    fn eval(&mut self, expression: &Expression, bindings: &BTreeMap<usize, Value>) -> Value {
        match expression {
            Expression::Constant(Constant::Int(value)) => Value::Int(*value),
            Expression::Variable { id, .. } => bindings.get(id).unwrap_or_else(|| panic!("unbound IR variable {id}")).clone(),
            Expression::Let { id, value, body } => {
                let value = self.eval(value, bindings);
                let mut local = bindings.clone();
                local.insert(*id, value);
                self.eval(body, &local)
            }
            Expression::Construct { constructor, fields, .. } => {
                let values = fields.iter().map(|field| self.eval(field, bindings)).collect();
                Value::Construct(constructor.clone(), values)
            }
            Expression::CallDirect { function, arguments, .. } | Expression::TailCall { function, arguments, .. } => {
                let values = arguments.iter().map(|argument| self.eval(argument, bindings)).collect();
                self.call(*function, values)
            }
            Expression::Binary { op: BinOp::Add, left, right } => {
                let Value::Int(left) = self.eval(left, bindings) else { panic!("non-integer left operand"); };
                let Value::Int(right) = self.eval(right, bindings) else { panic!("non-integer right operand"); };
                Value::Int(left.wrapping_add(right))
            }
            unsupported => panic!("unsupported IR in order oracle: {unsupported:?}"),
        }
    }
    fn call(&mut self, id: FunctionId, values: Vec<Value>) -> Value {
        let function = self.program.functions.iter().find(|function| function.id == id).expect("unknown IR function");
        assert!(function.captures.is_empty(), "generated functions have no captures");
        assert_eq!(function.parameters.len(), values.len());
        self.calls.push(function.name.clone());
        let bindings = function.parameters.iter().copied().zip(values).collect();
        self.eval(&function.body, &bindings)
    }
}
fn random(state: &mut u64, limit: usize) -> usize {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*state >> 32) as usize) % limit
}
struct Field {
    name: String,
    function: String,
    argument: i64,
    increment: i64,
}
impl Field {
    fn call(&self) -> String { format!("{} {}", self.function, self.argument) }
    fn result(&self) -> Value { Value::Int(self.argument + self.increment) }
}
struct Sample {
    fields: Vec<Field>,
    order: Vec<usize>,
    constructor: String,
}
impl Sample {
    fn generate(state: &mut u64) -> Self {
        let count = 3 + random(state, 3);
        let fields = (0..count).map(|i| Field {
            name: format!("field{i}_{}", random(state, 10000)),
            function: format!("compute{i}_{}", random(state, 10000)),
            argument: (i * 1000 + random(state, 100)) as i64,
            increment: (1 + random(state, 100)) as i64,
        }).collect();
        let mut order: Vec<_> = (0..count).collect();
        for i in (1..count).rev() { let j = random(state, i + 1); order.swap(i, j); }
        if order.iter().copied().eq(0..count) { order.rotate_left(1); }
        Self { fields, order, constructor: format!("Payload{}", random(state, 10000)) }
    }
    fn source(&self, named: bool) -> String {
        let definitions: Vec<_> = self.fields.iter().map(|field| format!("def {} x => x + {}", field.function, field.increment)).collect();
        let declaration = if named {
            format!("{} {{ {} }}", self.constructor, self.fields.iter().map(|field| format!("{}: Int", field.name)).collect::<Vec<_>>().join("; "))
        } else {
            format!("| {}({})", self.constructor, vec!["Int"; self.fields.len()].join(", "))
        };
        let construction = if named {
            format!("{} {{ {} }}", self.constructor, self.order.iter().map(|i| format!("{} = {}", self.fields[*i].name, self.fields[*i].call())).collect::<Vec<_>>().join("; "))
        } else {
            format!("{}({})", self.constructor, self.order.iter().map(|i| self.fields[*i].call()).collect::<Vec<_>>().join(", "))
        };
        format!("type Data = {declaration}\n{}\ndef main = {construction}", definitions.join("\n"))
    }
    fn verify(&self, named: bool) {
        let source = self.source(named);
        let ast = hado::parser::parse_source(&source, "field-order.hd").unwrap_or_else(|error| panic!("{error}\n{source}"));
        let checked = hado::check_source(&source, "field-order.hd").unwrap_or_else(|error| panic!("{error}\n{source}"));
        let program = hado::mono::lower_program(&ast, &checked).unwrap();
        let mut interpreter = Interpreter { program: &program, calls: Vec::new() };
        let actual = interpreter.eval(&program.entry, &BTreeMap::new());
        let expected_calls: Vec<_> = self.order.iter().map(|i| self.fields[*i].function.clone()).collect();
        assert_eq!(interpreter.calls, expected_calls, "call order: {source}");
        let payload = if named { self.fields.iter().map(Field::result).collect() }
            else { self.order.iter().map(|i| self.fields[*i].result()).collect() };
        assert_eq!(actual, Value::Construct(self.constructor.clone(), payload), "payload order: {source}");
    }
}
#[test]
fn named_fields_evaluate_in_written_order_and_construct_in_declaration_order() {
    let mut state = 0x120051;
    for _ in 0..12 { Sample::generate(&mut state).verify(true); }
}
#[test]
fn positional_constructor_control_uses_the_same_ir_interpreter() {
    let mut state = 0x120051;
    for _ in 0..12 { Sample::generate(&mut state).verify(false); }
}
