use hado::parser::{self, DefKind, HandlerArm, Item, Node, NodeKind};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DirectApplication {
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

fn collect_item(item: &Item, path: &str, applications: &mut Vec<DirectApplication>) {
    match item {
        Item::Def(definition) => match &definition.kind {
            DefKind::Value(body) | DefKind::Function { body, .. } => {
                collect_node(body, path, false, applications)
            }
        },
        Item::ModuleSig(module) => {
            for item in &module.items {
                collect_item(item, path, applications);
            }
        }
        Item::ModuleImpl(module) => {
            for item in &module.items {
                collect_item(item, path, applications);
            }
        }
        Item::Handler(handler) => {
            for arm in &handler.arms {
                let body = match arm {
                    HandlerArm::Operation { body, .. } | HandlerArm::Return { body, .. } => body,
                };
                collect_node(body, path, false, applications);
            }
        }
        Item::SigDef(_) | Item::TypeDef(_) | Item::Effect(_) => {}
    }
}

fn collect_node(
    node: &Node,
    path: &str,
    pipe_target: bool,
    applications: &mut Vec<DirectApplication>,
) {
    match &node.kind {
        NodeKind::Apply(callee, arguments) => {
            if !pipe_target {
                applications.push(DirectApplication {
                    path: path.to_string(),
                    line: node.span.line,
                    column: node.span.col,
                    expression: parser::canonical(node),
                });
            }
            collect_node(callee, path, false, applications);
            for argument in arguments {
                collect_node(argument, path, false, applications);
            }
        }
        NodeKind::Call(callee, arguments) => {
            collect_node(callee, path, false, applications);
            for argument in arguments {
                collect_node(argument, path, false, applications);
            }
        }
        NodeKind::Pipe(value, target) => {
            collect_node(value, path, false, applications);
            collect_node(target, path, true, applications);
        }
        NodeKind::Neg(value) | NodeKind::Not(value) => {
            collect_node(value, path, false, applications)
        }
        NodeKind::Bin(_, left, right) => {
            collect_node(left, path, false, applications);
            collect_node(right, path, false, applications);
        }
        NodeKind::Field(value, _) => collect_node(value, path, false, applications),
        NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) => {
            for (_, value) in fields {
                collect_node(value, path, false, applications);
            }
        }
        NodeKind::List(values, tail) => {
            for value in values {
                collect_node(value, path, false, applications);
            }
            if let Some(tail) = tail {
                collect_node(tail, path, false, applications);
            }
        }
        NodeKind::Let { value, body, .. } | NodeKind::LetQ { value, body, .. } => {
            collect_node(value, path, false, applications);
            collect_node(body, path, false, applications);
        }
        NodeKind::DoQ { value, body } => {
            collect_node(value, path, false, applications);
            collect_node(body, path, false, applications);
        }
        NodeKind::Fn { body, .. } => collect_node(body, path, false, applications),
        NodeKind::BoolBranch { cond, then, els } => {
            collect_node(cond, path, false, applications);
            collect_node(then, path, false, applications);
            collect_node(els, path, false, applications);
        }
        NodeKind::CondBranch { arms, fallback } => {
            for (condition, body) in arms {
                collect_node(condition, path, false, applications);
                collect_node(body, path, false, applications);
            }
            if let Some(fallback) = fallback {
                collect_node(fallback, path, false, applications);
            }
        }
        NodeKind::PatternBranch {
            scrutinees,
            arms,
            fallback,
        } => {
            for scrutinee in scrutinees {
                collect_node(scrutinee, path, false, applications);
            }
            for (_, body) in arms {
                collect_node(body, path, false, applications);
            }
            if let Some(fallback) = fallback {
                collect_node(fallback, path, false, applications);
            }
        }
        NodeKind::With { args, body, .. } => {
            for argument in args {
                collect_node(argument, path, false, applications);
            }
            collect_node(body, path, false, applications);
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

#[test]
fn distributed_hado_sources_prefer_pipelines() {
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
    let mut applications = Vec::new();
    for root in roots {
        for file in source_files(&repository.join(root)) {
            let relative = file.strip_prefix(&repository).unwrap().to_string_lossy();
            let source = fs::read_to_string(&file).unwrap();
            let ast = parser::parse_source(&source, &relative).unwrap();
            for item in &ast.items {
                collect_item(item, &relative, &mut applications);
            }
        }
    }
    for relative in appendix_b_examples {
        let file = repository.join(relative);
        let source = fs::read_to_string(&file).unwrap();
        let ast = parser::parse_source(&source, relative).unwrap();
        for item in &ast.items {
            collect_item(item, relative, &mut applications);
        }
    }
    applications.sort();

    assert!(
        applications.is_empty(),
        "direct applications remain outside pipeline targets:\n{}",
        applications
            .iter()
            .map(|application| format!(
                "{}:{}:{} {}",
                application.path, application.line, application.column, application.expression
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
