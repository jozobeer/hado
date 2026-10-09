//! `hado repl [<dir>]` — セッションのプロセスを 1 つ保ち、入力ごとに新しい
//! 関数を JIT で足して呼ぶ（`docs/plans/repl.md` の案 A）。
//!
//! セッションは `main` の本体の続きとして扱う（§11）。`e as x ->` で終えた
//! 入力は値だけを評価して束縛として持ち越し、以後の入力はその束縛を仮引数と
//! して受け取る。入力ごとに評価は 1 回なので、前の入力の効果は再実行されない。
//!
//! 値の印字は §54 の `main` の規則に従う（Int は 10 進、Void は何も出さない、
//! String は引用符なし）。`main` の型が Int なら終了コードにする規則だけは
//! 当てない。対話では値を見せる必要があるためである。

use std::collections::HashMap;
use std::io::{BufRead, IsTerminal};
use std::path::{Path, PathBuf};

use cranelift_codegen::ir::{types as clif_types, AbiParam, InstBuilder, Signature};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};

use crate::lexer::{lex_full, Tok, TokKind};
use crate::mono::{self, FunctionId};
use crate::project::Project;
use crate::runtime_abi::RuntimeFunction;
use crate::types::{Scheme, Ty};

const RTLD_NOW: i32 = 2;
const RTLD_LOCAL: i32 = 0;

extern "C" {
    fn dlopen(filename: *const i8, flag: i32) -> *mut core::ffi::c_void;
    fn dlsym(handle: *mut core::ffi::c_void, symbol: *const i8) -> *mut core::ffi::c_void;
    fn dlerror() -> *const i8;
    /// 入力ごとに C の標準出力を流す。パイプのときは塊で溜まるので、診断や
    /// 次の入力との順序を保つためにここで吐く。
    fn fflush(stream: *mut core::ffi::c_void) -> i32;
}

pub fn run(dir: Option<&Path>) -> Result<(), String> {
    let interactive = std::io::stdin().is_terminal();
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    run_with(dir, &mut input, interactive)
}

/// セッションを回す。`interactive` は前口上を標準エラーへ出すかどうかだけを
/// 変える（テストはここへ入力の列を直接渡す）。
pub fn run_with(
    dir: Option<&Path>,
    input: &mut dyn BufRead,
    interactive: bool,
) -> Result<(), String> {
    let mut session = Session::open(dir)?;
    let mut pending = String::new();
    loop {
        if interactive {
            eprint!("> ");
        }
        let mut line = String::new();
        let read = input
            .read_line(&mut line)
            .map_err(|error| format!("read stdin: {error}"))?;
        if read == 0 {
            break;
        }
        pending.push_str(&line);
        if !input_complete(&pending) {
            continue;
        }
        let text = std::mem::take(&mut pending);
        session.step(&text);
    }
    // EOF で未完の入力も、そのまま投げて診断を見せる。
    if !pending.trim().is_empty() {
        session.step(&pending);
    }
    session.close();
    Ok(())
}

/// C ランタイムを `dlopen` したハンドル。住所は `JITBuilder::symbol` に渡す。
struct Runtime {
    handle: *mut core::ffi::c_void,
    path: PathBuf,
}

impl Runtime {
    fn load() -> Result<Self, String> {
        let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libhado_repl_runtime.so"));
        let path = std::env::temp_dir().join(format!("hado-repl-{}.so", std::process::id()));
        std::fs::write(&path, bytes)
            .map_err(|error| format!("write {}: {error}", path.display()))?;
        let name = std::ffi::CString::new(path.to_str().ok_or("temp path is not UTF-8")?)
            .map_err(|error| error.to_string())?;
        let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
        if handle.is_null() {
            let message = unsafe { std::ffi::CStr::from_ptr(dlerror()) };
            return Err(format!(
                "dlopen {}: {}",
                path.display(),
                message.to_string_lossy()
            ));
        }
        Ok(Self { handle, path })
    }

    fn symbol(&self, name: &str) -> Result<*const u8, String> {
        let name = std::ffi::CString::new(name).map_err(|error| error.to_string())?;
        let address = unsafe { dlsym(self.handle, name.as_ptr()) };
        if address.is_null() {
            return Err(format!("runtime symbol not found: {name:?}"));
        }
        Ok(address as *const u8)
    }

    fn register(&self, builder: &mut JITBuilder) -> Result<(), String> {
        for function in RuntimeFunction::ALL {
            // この 2 つは `emit` がインラインで展開し、`declaration` を持たない。
            if matches!(
                function,
                RuntimeFunction::ArrayGet | RuntimeFunction::ArrayFillUnchecked
            ) {
                continue;
            }
            let symbol = function.declaration().symbol;
            builder.symbol(symbol, self.symbol(symbol)?);
        }
        Ok(())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// `e as x ->` で持ち越した束縛。値は機械語のビット列のまま持つ。
struct Binder {
    name: String,
    /// 次の入力の仮引数の型注釈に使う、ソースの綴りの型。
    ty: Ty,
    value: u64,
}

struct Session {
    root: PathBuf,
    runtime: Runtime,
    binders: Vec<Binder>,
    /// JIT したモジュールは閉包のコードを指したままにするため、セッションの
    /// 間は落とさない。
    modules: Vec<JITModule>,
}

impl Session {
    fn open(dir: Option<&Path>) -> Result<Self, String> {
        let root = std::env::temp_dir().join(format!("hado-repl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("create {}: {error}", root.display()))?;
        if let Some(dir) = dir {
            embed_library(dir, &root)?;
        }
        let runtime = Runtime::load()?;
        Ok(Self {
            root,
            runtime,
            binders: Vec::new(),
            modules: Vec::new(),
        })
    }

    fn close(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }

    /// 入力 1 つを評価する。診断は標準エラーへ出し、セッションは続ける。
    fn step(&mut self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        if starts_with_definition(trimmed) {
            eprintln!("repl: 定義の入力はまだ使えない（式と `e as x ->` だけを受ける）");
            return;
        }
        let step = split_step(text);
        let used: Vec<usize> = self
            .binders
            .iter()
            .enumerate()
            .filter(|(_, binder)| occurs(&step.body, &binder.name))
            .map(|(index, _)| index)
            .collect();
        let source = compose(&step.body, &self.binders, &used);
        let entry = self.root.join("main.hd");
        if let Err(error) = std::fs::write(&entry, source) {
            eprintln!("repl: write {}: {error}", entry.display());
            return;
        }
        let project = match Project::load(&entry) {
            Ok(project) => project,
            Err(error) => {
                eprintln!("{error}");
                return;
            }
        };
        let (program, checked) = match project.lower_repl_step() {
            Ok(lowered) => lowered,
            Err(errors) => {
                for error in errors {
                    eprintln!("{}", project.report(&error));
                }
                return;
            }
        };
        let Some((function, result)) = resolve_step(&program, &checked, !used.is_empty()) else {
            eprintln!("repl: 1 歩の関数が読めない");
            return;
        };
        if !result.is_monomorphic() {
            eprintln!("repl: 値の型が単相でない（§10 と同じ規則）");
            return;
        }
        match self.evaluate(&program, &checked, &function, &used, step.display) {
            Ok(bits) => {
                if let Some(name) = step.binder {
                    self.binders.push(Binder {
                        name,
                        ty: result,
                        value: bits,
                    });
                }
            }
            Err(errors) => {
                for error in errors {
                    eprintln!("{error}");
                }
            }
        }
        unsafe {
            fflush(std::ptr::null_mut());
        }
    }

    /// 合成した 1 歩を JIT して 1 回呼ぶ。`display` ならプリンタを呼ぶ。
    fn evaluate(
        &mut self,
        program: &mono::Program,
        checked: &crate::Checked,
        step: &mono::Function,
        used: &[usize],
        display: bool,
    ) -> Result<u64, Vec<String>> {
        let mut builder = JITBuilder::with_flags(
            &[("opt_level", "none"), ("preserve_frame_pointers", "true")],
            default_libcall_names(),
        )
        .map_err(|error| vec![format!("{error:?}")])?;
        self.runtime
            .register(&mut builder)
            .map_err(|error| vec![error])?;
        let mut module = JITModule::new(builder);

        let mut functions = program.functions.clone();
        if !functions.iter().any(|function| function.id == step.id) {
            functions.push(step.clone());
        }
        let max = functions
            .iter()
            .map(|function| function.id.0)
            .max()
            .unwrap_or(0);
        let printers = if display {
            mono::derive_printers(FunctionId(max + 2), step.signature.result.clone(), checked)
                .map_err(|error| vec![format!("この入力の値は印字できない: {error}")])?
        } else {
            Vec::new()
        };

        let mut ids: HashMap<usize, FuncId> = HashMap::new();
        for function in &functions {
            let signature = crate::emit::mono_signature(
                &function.signature,
                checked,
                module.target_config().pointer_type(),
            )
            .map_err(|error| vec![error])?;
            let id = module
                .declare_function(
                    &format!("hado_fn_{}", function.id.0),
                    Linkage::Local,
                    &signature,
                )
                .map_err(|error| vec![error.to_string()])?;
            ids.insert(function.id.0, id);
        }
        for printer in &printers {
            let signature = crate::emit::mono_signature(
                &printer.signature,
                checked,
                module.target_config().pointer_type(),
            )
            .map_err(|error| vec![error])?;
            let id = module
                .declare_function(
                    &format!("hado_printer_{}", printer.id.0),
                    Linkage::Local,
                    &signature,
                )
                .map_err(|error| vec![error.to_string()])?;
            ids.insert(printer.id.0, id);
        }
        let mut errors = Vec::new();
        for function in &functions {
            if let Err(error) =
                crate::emit::define_mono_function(&mut module, function, checked, &ids)
            {
                errors.push(error);
            }
        }
        for printer in &printers {
            if let Err(error) =
                crate::emit::define_mono_function(&mut module, printer, checked, &ids)
            {
                errors.push(error);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let step_id = ids[&step.id.0];
        let printer_id = printers.first().map(|printer| ids[&printer.id.0]);
        let bridge = define_bridge(&mut module, step_id, printer_id, step, checked)
            .map_err(|error| vec![error])?;
        module
            .finalize_definitions()
            .map_err(|error| vec![error.to_string()])?;

        let mut environment: Vec<u64> = used
            .iter()
            .map(|index| self.binders[*index].value)
            .collect();
        let code = module.get_finalized_function(bridge);
        let call: extern "C" fn(*mut u8) -> i64 = unsafe { std::mem::transmute(code) };
        let bits = call(environment.as_mut_ptr().cast::<u8>()) as u64;
        self.modules.push(module);
        Ok(bits)
    }
}

/// `e as x ->` を剥がした 1 歩。`display` は値を見せるか（`->` で終えなければ見せる）。
struct Step {
    body: String,
    binder: Option<String>,
    display: bool,
}

/// 入力の末尾が `->` なら継続の印として剥がす。直前が `as x` なら束縛。
fn split_step(text: &str) -> Step {
    let without = text.trim_end();
    let Some(head) = without.strip_suffix("->") else {
        return Step {
            body: text.to_string(),
            binder: None,
            display: true,
        };
    };
    let head = head.trim_end();
    match trailing_binding(head) {
        Some((body, name)) => Step {
            body,
            binder: Some(name),
            display: false,
        },
        None => Step {
            body: head.to_string(),
            binder: None,
            display: false,
        },
    }
}

/// `… as name` の末尾を `(本体, 名前)` に割る。
fn trailing_binding(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim_end();
    let start = trimmed
        .rfind(|character: char| !(character.is_alphanumeric() || character == '_'))
        .map_or(0, |index| index + 1);
    let name = &trimmed[start..];
    if !is_name(name) {
        return None;
    }
    let before = trimmed[..start].trim_end();
    let head = before.strip_suffix("as")?;
    if head.ends_with(|character: char| character.is_alphanumeric() || character == '_') {
        return None;
    }
    Some((head.trim_end().to_string(), name.to_string()))
}

fn is_name(name: &str) -> bool {
    let mut characters = name.chars();
    match characters.next() {
        Some(first) if first.is_ascii_lowercase() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// `body` が名前 `name` を字句として含むか。使う束縛だけを仮引数にする
/// （未使用の仮引数は E505 になる）。
fn occurs(body: &str, name: &str) -> bool {
    tokens(body)
        .iter()
        .any(|token| matches!(&token.kind, TokKind::Name(found) if found == name))
}

fn tokens(text: &str) -> Vec<Tok> {
    lex_full(text).unwrap_or_default()
}

fn starts_with_definition(text: &str) -> bool {
    matches!(
        tokens(text).first().map(|token| &token.kind),
        Some(
            TokKind::Def
                | TokKind::KwType
                | TokKind::KwModule
                | TokKind::KwImplement
                | TokKind::KwEffect
                | TokKind::KwHandler
                | TokKind::KwLibrary
        )
    )
}

/// 合成した入口のソース。使う束縛が無ければ `def main = <本体>` にして、
/// §10 の型・効果の規則をそのまま受ける。使う束縛があればそれを仮引数にした
/// `hado_step` を作り、`main` から参照して単相化させる。
fn compose(body: &str, binders: &[Binder], used: &[usize]) -> String {
    if used.is_empty() {
        return format!("def main = {body}\n");
    }
    let mut source = String::from("def hado_step (");
    for (position, index) in used.iter().enumerate() {
        if position > 0 {
            source.push_str(", ");
        }
        let binder = &binders[*index];
        source.push_str(&binder.name);
        source.push_str(": ");
        source.push_str(&Scheme::monomorphic(binder.ty.clone()).canonical());
    }
    source.push_str(") => ");
    source.push_str(body);
    source.push_str("\ndef main = hado_step\n");
    source
}

/// 合成した入口から 1 歩の関数と、その値のソースの型を取る。仮引数が無い形では
/// 入口の式そのものを引数なしの関数に包む（`emit` が `hado_main` を作るのと同じ形）。
/// 形は入口の式からではなく、合成したソースの形（`parameterized`）で決める。
/// 入口の式は単相化の都合で `Let` などにもなり、形の判定には使えない。
fn resolve_step(
    program: &mono::Program,
    checked: &crate::Checked,
    parameterized: bool,
) -> Option<(mono::Function, Ty)> {
    if parameterized {
        let mono::Expression::MakeClosure { function, .. } = &program.entry else {
            return None;
        };
        let found = program.functions.iter().find(|f| f.id == *function)?;
        // `hado_step` の scheme ではなく `main` の型から取る。scheme の効果行は
        // 一般化された開いた行なので、値を束縛するときの単相性（§10）に合わない。
        let Ty::Fn(_, result, _) = &checked.main else {
            return None;
        };
        return Some((found.clone(), result.as_ref().clone()));
    }
    let step = mono::Function {
        captures: Vec::new(),
        id: FunctionId(
            program
                .functions
                .iter()
                .map(|function| function.id.0)
                .max()
                .map_or(0, |id| id + 1),
        ),
        name: "hado_step".to_string(),
        parameters: Vec::new(),
        signature: mono::Signature {
            parameters: Vec::new(),
            result: program.entry.ty(),
        },
        body: program.entry.clone(),
    };
    Some((step, checked.main.clone()))
}

/// JIT した 1 歩を呼ぶ橋。`env` は束縛の値を 8 バイトずつ並べた配列で、
///
/// ```text
/// extern "C" fn bridge(env: *mut u8) -> i64
/// ```
///
/// の戻りは、表示する歩ではプリンタの状態、束縛する歩では値のビット列である。
fn define_bridge(
    module: &mut JITModule,
    step: FuncId,
    printer: Option<FuncId>,
    step_function: &mono::Function,
    checked: &crate::Checked,
) -> Result<FuncId, String> {
    let pointer = module.target_config().pointer_type();
    let signature = crate::emit::mono_signature(&step_function.signature, checked, pointer)?;
    let mut context = module.make_context();
    context.func.signature = Signature::new(CallConv::SystemV);
    context.func.signature.params.push(AbiParam::new(pointer));
    context
        .func
        .signature
        .returns
        .push(AbiParam::new(clif_types::I64));
    let bridge = module
        .declare_function("hado_repl_step", Linkage::Export, &context.func.signature)
        .map_err(|error| error.to_string())?;

    let mut frontend = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut context.func, &mut frontend);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    let environment = builder.block_params(block)[0];
    let zero = builder.ins().iconst(pointer, 0);
    let mut arguments = vec![zero];
    // `signature.params[0]` は mono の関数が必ず取る環境である（`repr::function_signature`）。
    for (index, parameter) in signature.params.iter().skip(1).enumerate() {
        let offset = i32::try_from(index * 8).map_err(|_| "environment offset overflow")?;
        let value = builder.ins().load(
            parameter.value_type,
            cranelift_codegen::ir::MemFlagsData::new(),
            environment,
            offset,
        );
        arguments.push(value);
    }
    let callee = module.declare_func_in_func(step, builder.func);
    let call = builder.ins().call(callee, &arguments);
    let result = builder.inst_results(call)[0];
    let result_type = signature.returns[0].value_type;
    match printer {
        Some(printer) => {
            let print = module.declare_func_in_func(printer, builder.func);
            let printed = builder.ins().call(print, &[zero, result]);
            let status = builder.inst_results(printed)[0];
            builder.ins().return_(&[status]);
        }
        None => {
            let boxed = to_i64(&mut builder, result, result_type);
            builder.ins().return_(&[boxed]);
        }
    }
    builder.seal_all_blocks();
    builder.finalize(module.target_config());
    module
        .define_function(bridge, &mut context)
        .map_err(|error| error.to_string())?;
    Ok(bridge)
}

/// 機械語の値を 8 バイトのビット列に揃える。束縛はこのビット列のまま持ち、
/// 次の歩の橋が元の型で読み戻す。
fn to_i64(
    builder: &mut FunctionBuilder<'_>,
    value: cranelift_codegen::ir::Value,
    ty: cranelift_codegen::ir::Type,
) -> cranelift_codegen::ir::Value {
    if ty == clif_types::F64 {
        return builder.ins().bitcast(
            clif_types::I64,
            cranelift_codegen::ir::MemFlagsData::new(),
            value,
        );
    }
    if ty == clif_types::I64 {
        return value;
    }
    builder.ins().uextend(clif_types::I64, value)
}

/// `hado repl <dir>` の `<dir>` をセッションの根の下へ名前空間として置く（§51・§52）。
/// ライブラリは `<dir>` の名前で `name::実装.member` として見える。
fn embed_library(dir: &Path, root: &Path) -> Result<(), String> {
    let name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("library directory has no usable name: {}", dir.display()))?;
    if !is_namespace(name) {
        return Err(format!(
            "library directory name is not a namespace: {name}（§51、小文字で始まる字句）"
        ));
    }
    copy_tree(dir, &root.join(name))
}

fn is_namespace(name: &str) -> bool {
    let mut characters = name.chars();
    match characters.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target)
        .map_err(|error| format!("create {}: {error}", target.display()))?;
    let entries =
        std::fs::read_dir(source).map_err(|error| format!("read {}: {error}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // §50: `.` と `__` で始まる名前は読まない。
        if name.starts_with('.') || name.starts_with("__") {
            continue;
        }
        let path = entry.path();
        let destination = target.join(name.as_ref());
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            copy_tree(&path, &destination)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("hd" | "hdi")
        ) {
            std::fs::copy(&path, &destination)
                .map_err(|error| format!("copy {}: {error}", path.display()))?;
        }
    }
    Ok(())
}

/// 入力がここで完結したか（`docs/issues/repl.md` の段階 1）。
///
/// - 括弧（`(` `[` `{`）が釣り合っている
/// - `fn`・`module`・`implement`・`effect`・`handler` の開きと閉じ（`/…`）が釣り合う
/// - 行末が開いた字句（`=>`・`|>`・`|`・`,`・`=`）でない
///
/// 字句が読めない入力は未完として続きを待つ。`->` で終わる入力は束縛の印なので
/// 完結として送る。
fn input_complete(text: &str) -> bool {
    let tokens = tokens(text);
    let Some(last) = tokens
        .iter()
        .rfind(|token| !matches!(token.kind, TokKind::Eof))
    else {
        return false;
    };
    let mut depth: i64 = 0;
    let mut open_blocks: i64 = 0;
    let mut close_blocks: i64 = 0;
    for (index, token) in tokens.iter().enumerate() {
        let next = tokens.get(index + 1).map(|next| &next.kind);
        match &token.kind {
            TokKind::LParen | TokKind::LBracket | TokKind::LBrace => depth += 1,
            TokKind::RParen | TokKind::RBracket | TokKind::RBrace => depth -= 1,
            TokKind::KwFn if !matches!(next, Some(TokKind::LParen)) => open_blocks += 1,
            TokKind::KwModule | TokKind::KwImplement | TokKind::KwEffect | TokKind::KwHandler => {
                open_blocks += 1;
            }
            TokKind::Slash
                if matches!(
                    next,
                    Some(TokKind::KwFn | TokKind::KwModule | TokKind::KwImplement | TokKind::KwIf)
                ) =>
            {
                close_blocks += 1;
            }
            _ => {}
        }
    }
    if depth > 0 || open_blocks > close_blocks {
        return false;
    }
    !matches!(
        last.kind,
        TokKind::Arrow | TokKind::PipeArrow | TokKind::Pipe | TokKind::Comma | TokKind::Eq
    )
}
