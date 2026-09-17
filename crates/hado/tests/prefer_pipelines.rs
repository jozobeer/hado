use hado::parser::{self, DefKind, HandlerArm, Item, Node, NodeKind};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ValueFirstPipeline {
    path: String,
    line: usize,
    column: usize,
    expression: String,
}

fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if matches!(path.extension().and_then(|extension| extension.to_str()), Some("hd" | "hdi")) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn collect_item(item: &Item, path: &str, pipelines: &mut Vec<ValueFirstPipeline>) {
    match item {
        Item::Def(definition) => match &definition.kind {
            DefKind::Value(body) | DefKind::Function { body, .. } => {
                collect_node(body, path, false, pipelines)
            }
        },
        Item::ModuleSig(module) => {
            for item in &module.items {
                collect_item(item, path, pipelines);
            }
        }
        Item::ModuleImpl(module) => {
            for item in &module.items {
                collect_item(item, path, pipelines);
            }
        }
        Item::Handler(handler) => {
            for arm in &handler.arms {
                let body = match arm {
                    HandlerArm::Operation { body, .. } | HandlerArm::Return { body, .. } => body,
                };
                collect_node(body, path, false, pipelines);
            }
        }
        Item::SigDef(_) | Item::TypeDef(_) | Item::Effect(_) => {}
    }
}

fn collect_node(
    node: &Node,
    path: &str,
    is_pipe_left_operand: bool,
    pipelines: &mut Vec<ValueFirstPipeline>,
) {
    match &node.kind {
        NodeKind::Apply(callee, arguments) => {
            collect_node(callee, path, false, pipelines);
            for argument in arguments {
                collect_node(argument, path, false, pipelines);
            }
        }
        NodeKind::Call(callee, arguments) => {
            collect_node(callee, path, false, pipelines);
            for argument in arguments {
                collect_node(argument, path, false, pipelines);
            }
        }
        NodeKind::Pipe(value, target) => {
            if !is_pipe_left_operand
                && !matches!(&pipeline_source(node).kind, NodeKind::Apply(_, _))
            {
                pipelines.push(ValueFirstPipeline {
                    path: path.to_string(),
                    line: node.span.line,
                    column: node.span.col,
                    expression: parser::canonical(node),
                });
            }
            collect_node(value, path, true, pipelines);
            collect_node(target, path, false, pipelines);
        }
        NodeKind::Neg(value) | NodeKind::Not(value) => {
            collect_node(value, path, false, pipelines)
        }
        NodeKind::Bin(_, left, right) => {
            collect_node(left, path, false, pipelines);
            collect_node(right, path, false, pipelines);
        }
        NodeKind::Field(value, _) => collect_node(value, path, false, pipelines),
        NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) => {
            for (_, value) in fields {
                collect_node(value, path, false, pipelines);
            }
        }
        NodeKind::List(values, tail) => {
            for value in values {
                collect_node(value, path, false, pipelines);
            }
            if let Some(tail) = tail {
                collect_node(tail, path, false, pipelines);
            }
        }
        NodeKind::Let { value, body, .. } | NodeKind::LetQ { value, body, .. } => {
            collect_node(value, path, false, pipelines);
            collect_node(body, path, false, pipelines);
        }
        NodeKind::DoQ { value, body } => {
            collect_node(value, path, false, pipelines);
            collect_node(body, path, false, pipelines);
        }
        NodeKind::Fn { body, .. } => collect_node(body, path, false, pipelines),
        NodeKind::BoolBranch { cond, then, els } => {
            collect_node(cond, path, false, pipelines);
            collect_node(then, path, false, pipelines);
            collect_node(els, path, false, pipelines);
        }
        NodeKind::CondBranch { arms, fallback } => {
            for (condition, body) in arms {
                collect_node(condition, path, false, pipelines);
                collect_node(body, path, false, pipelines);
            }
            if let Some(fallback) = fallback {
                collect_node(fallback, path, false, pipelines);
            }
        }
        NodeKind::PatternBranch {
            scrutinees,
            arms,
            fallback,
        } => {
            for scrutinee in scrutinees {
                collect_node(scrutinee, path, false, pipelines);
            }
            for (_, body) in arms {
                collect_node(body, path, false, pipelines);
            }
            if let Some(fallback) = fallback {
                collect_node(fallback, path, false, pipelines);
            }
        }
        NodeKind::With { args, body, .. } => {
            for argument in args {
                collect_node(argument, path, false, pipelines);
            }
            collect_node(body, path, false, pipelines);
        }
        NodeKind::Int(_)
        | NodeKind::Float(_)
        | NodeKind::Char(_)
        | NodeKind::Str(_)
        | NodeKind::Bool(_)
        | NodeKind::Unit
        | NodeKind::Name(_)
        | NodeKind::Resume => {}
    }
}

fn pipeline_source(mut node: &Node) -> &Node {
    while let NodeKind::Pipe(value, _) = &node.kind {
        node = value;
    }
    node
}

#[test]
fn distributed_hado_sources_start_pipelines_from_applications() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roots = ["stdlib", "tools"];
    let appendix_b_examples = [
        "tests/accept/active_names/main.hd",
        "tests/accept/classify/main.hd",
        "tests/accept/create_user/main.hd",
        "tests/accept/delete_user/main.hd",
        "tests/accept/display/main.hd",
        "tests/accept/display_name/main.hd",
        "tests/accept/functor/main.hd",
        "tests/accept/name/main.hd",
        "tests/accept/size/main.hd",
        "tests/accept/to_option/main.hd",
        "tests/accept/user_store/main.hd",
    ];
    let mut pipelines = Vec::new();
    for root in roots {
        for file in source_files(&repository.join(root)) {
            let relative = file.strip_prefix(&repository).unwrap().to_string_lossy();
            let source = fs::read_to_string(&file).unwrap();
            let ast = parser::parse_source(&source, &relative).unwrap();
            for item in &ast.items {
                collect_item(item, &relative, &mut pipelines);
            }
        }
    }
    for relative in appendix_b_examples {
        let file = repository.join(relative);
        let source = fs::read_to_string(&file).unwrap();
        let ast = parser::parse_source(&source, relative).unwrap();
        for item in &ast.items {
            collect_item(item, relative, &mut pipelines);
        }
    }
    pipelines.sort();

    assert!(
        pipelines.is_empty(),
        "pipelines start from values instead of application results:\n{}",
        pipelines
            .iter()
            .map(|pipeline| format!(
                "{}:{}:{} {}",
                pipeline.path, pipeline.line, pipeline.column, pipeline.expression
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
