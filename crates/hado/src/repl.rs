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

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{BufRead, IsTerminal};
use std::path::{Path, PathBuf};

use cranelift_codegen::ir::{types as clif_types, AbiParam, InstBuilder, Signature};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};

use crate::lexer::{lex_full, Tok, TokKind};
use crate::mono::{self, FunctionId};
use crate::parser::{parse_source, DefKind, Item};
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

    /// `.so` が公開する関数の住所。`dlopen` した側の関数は直接は呼べないので、
    /// `dlsym` の住所を関数の型へ読み替える。`hado_runtime_init`・`hado_repl_run`・
    /// `hado_gc_add_root` はこの経路で取る。
    fn function<T: Copy>(&self, name: &str) -> Result<T, String> {
        let address = self.symbol(name)?;
        debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<*const u8>());
        Ok(unsafe { std::mem::transmute_copy(&address) })
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

/// `e as x ->` で持ち越した束縛。値は機械語のビット列のまま、Boehm GC の根と
/// して登録した 1 語に持つ。Rust のヒープは GC が走査しないので、根にしないと
/// 次の入力までに回収されうる（`docs/rounds/repl.md` の欠陥 1）。
struct Binder {
    name: String,
    /// 次の入力の仮引数の型注釈に使う、ソースの綴りの型。
    ty: Ty,
    slot: Box<u64>,
}

struct Session {
    root: PathBuf,
    runtime: Runtime,
    binders: Vec<Binder>,
    /// JIT したモジュールは閉包のコードを指したままにするため、セッションの
    /// 間は落とさない。
    modules: Vec<JITModule>,
    /// JIT した 1 歩を raise の捕獲つきで走らせる（`hado_repl_run`）。
    run: unsafe extern "C" fn(*const u8, *mut u8, *mut i64) -> i32,
    /// 束縛の 1 語を GC の根として登録する（`hado_gc_add_root`）。
    add_root: unsafe extern "C" fn(*mut core::ffi::c_void),
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
        // ゲストの `main` がやることを先に済ませる。`GC_INIT` とスタック枯渇の
        // 番人は `main` の中にしか無く、JIT の経路では誰も呼ばない（欠陥 1・5）。
        let init: unsafe extern "C" fn() = runtime.function("hado_runtime_init")?;
        let run = runtime.function("hado_repl_run")?;
        let add_root = runtime.function("hado_gc_add_root")?;
        unsafe { init() };
        Ok(Self {
            root,
            runtime,
            binders: Vec::new(),
            modules: Vec::new(),
            run,
            add_root,
        })
    }

    fn close(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }

    /// 入力 1 つを評価する。診断は標準エラーへ出し、セッションは続ける。
    fn step(&mut self, text: &str) {
        if self.reject_input(text) {
            return;
        }
        let step = split_step(text);
        let used = self.used_binders(&step.body);
        let Some((program, checked, function, result)) = self.lower(&step.body, &used) else {
            return;
        };
        match self.evaluate(&program, &checked, &function, &used, step.display) {
            Ok(Some(bits)) => {
                if let Some(name) = step.binder {
                    self.bind(name, result, bits);
                }
            }
            // raise が最上位に届いた入力は捨てる（メッセージはランタイムが出す）。
            Ok(None) => {}
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

    /// 何も評価しない入力を断る。断ったら true。定義の入力は段階 1 では受けない。
    fn reject_input(&self, text: &str) -> bool {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return true;
        }
        if starts_with_definition(trimmed) {
            eprintln!("repl: 定義の入力はまだ使えない（式と `e as x ->` だけを受ける）");
            return true;
        }
        false
    }

    /// 本体が値として使う束縛の位置。使う束縛だけを仮引数にする（未使用の
    /// 仮引数は E505 になる）。同じ綴りがラムダの仮引数や field に出ても、
    /// それは値の参照ではないので仮引数にしない。同じ名前を打ち直したときは
    /// 新しい方だけを残す。
    fn used_binders(&self, body: &str) -> Vec<usize> {
        let names = expression_free_names(body).unwrap_or_default();
        let mut seen = HashSet::new();
        let mut used = Vec::new();
        for (index, binder) in self.binders.iter().enumerate().rev() {
            if !seen.insert(binder.name.clone()) {
                continue;
            }
            if names.contains(&binder.name) {
                used.push(index);
            }
        }
        used.reverse();
        used
    }

    /// 合成したソースを書いて検査・単相化し、1 歩の関数と値の型を取る。断る
    /// 場合は診断を出して `None`。
    fn lower(
        &self,
        body: &str,
        used: &[usize],
    ) -> Option<(mono::Program, crate::Checked, mono::Function, Ty)> {
        let source = compose(body, &self.binders, used);
        let entry = self.root.join("main.hd");
        if let Err(error) = std::fs::write(&entry, source) {
            eprintln!("repl: write {}: {error}", entry.display());
            return None;
        }
        let project = match Project::load(&entry) {
            Ok(project) => project,
            Err(error) => {
                eprintln!("{error}");
                return None;
            }
        };
        let (program, checked) = match project.lower_repl_step() {
            Ok(lowered) => lowered,
            Err(errors) => {
                for error in errors {
                    eprintln!("{}", project.report(&error));
                }
                return None;
            }
        };
        let Some((function, result)) = resolve_step(&program, &checked, !used.is_empty()) else {
            eprintln!("repl: 1 歩の関数が読めない");
            return None;
        };
        if !result.is_monomorphic() {
            eprintln!("repl: 値の型が単相でない（§10 と同じ規則）");
            return None;
        }
        Some((program, checked, function, result))
    }

    /// 束縛を 1 つ足す。値は GC の根として登録した 1 語に置く。
    fn bind(&mut self, name: String, ty: Ty, bits: u64) {
        let mut slot = Box::new(bits);
        unsafe { (self.add_root)((&mut *slot as *mut u64).cast()) };
        self.binders.push(Binder { name, ty, slot });
    }

    /// 合成した 1 歩を JIT して 1 回呼ぶ。`display` ならプリンタを呼ぶ。
    /// `None` は raise が最上位に届いてこの入力を捨てたことを表す。
    fn evaluate(
        &mut self,
        program: &mono::Program,
        checked: &crate::Checked,
        step: &mono::Function,
        used: &[usize],
        display: bool,
    ) -> Result<Option<u64>, Vec<String>> {
        let mut module = self.new_module()?;
        let functions = step_functions(program, step);
        let printers = if display {
            mono::derive_printers(
                FunctionId(max_id(&functions) + 2),
                step.signature.result.clone(),
                checked,
            )
            .map_err(|error| vec![format!("この入力の値は印字できない: {error}")])?
        } else {
            Vec::new()
        };
        let ids = declare_all(&mut module, &functions, &printers, checked)?;
        define_all(&mut module, &functions, &printers, checked, &ids)?;
        let bridge = define_bridge(
            &mut module,
            ids[&step.id.0],
            printers.first().map(|printer| ids[&printer.id.0]),
            step,
            checked,
        )
        .map_err(|error| vec![error])?;
        module
            .finalize_definitions()
            .map_err(|error| vec![error.to_string()])?;
        let code = module.get_finalized_function(bridge);
        // 閉包のコードを指したままにするので、モジュールはセッションが持つ。
        self.modules.push(module);
        self.call(code, used)
    }

    fn new_module(&self) -> Result<JITModule, Vec<String>> {
        let mut builder = JITBuilder::with_flags(
            &[("opt_level", "none"), ("preserve_frame_pointers", "true")],
            default_libcall_names(),
        )
        .map_err(|error| vec![format!("{error:?}")])?;
        self.runtime
            .register(&mut builder)
            .map_err(|error| vec![error])?;
        Ok(JITModule::new(builder))
    }

    /// 橋を呼ぶ。束縛は GC の根に置いた 1 語から読む。
    fn call(&self, code: *const u8, used: &[usize]) -> Result<Option<u64>, Vec<String>> {
        let mut environment: Vec<u64> = used
            .iter()
            .map(|index| *self.binders[*index].slot)
            .collect();
        let mut value: i64 = 0;
        let raised = unsafe { (self.run)(code, environment.as_mut_ptr().cast::<u8>(), &mut value) };
        if raised != 0 {
            return Ok(None);
        }
        Ok(Some(value as u64))
    }
}

/// 1 歩で JIT する関数の列。合成した `hado_step` が単相化の結果に無いときだけ
/// 足す（引数の無い形では入口の式を包んだものを別に作る）。
fn step_functions(program: &mono::Program, step: &mono::Function) -> Vec<mono::Function> {
    let mut functions = program.functions.clone();
    if !functions.iter().any(|function| function.id == step.id) {
        functions.push(step.clone());
    }
    functions
}

fn max_id(functions: &[mono::Function]) -> usize {
    functions
        .iter()
        .map(|function| function.id.0)
        .max()
        .unwrap_or(0)
}

/// 関数とプリンタの名前を宣言して id を引く。id は `emit` が内部参照に使う。
fn declare_all(
    module: &mut JITModule,
    functions: &[mono::Function],
    printers: &[mono::Function],
    checked: &crate::Checked,
) -> Result<HashMap<usize, FuncId>, Vec<String>> {
    let mut ids = HashMap::new();
    for (prefix, list) in [("hado_fn_", functions), ("hado_printer_", printers)] {
        for function in list {
            let signature = crate::emit::mono_signature(
                &function.signature,
                checked,
                module.target_config().pointer_type(),
            )
            .map_err(|error| vec![error])?;
            let id = module
                .declare_function(
                    &format!("{prefix}{}", function.id.0),
                    Linkage::Local,
                    &signature,
                )
                .map_err(|error| vec![error.to_string()])?;
            ids.insert(function.id.0, id);
        }
    }
    Ok(ids)
}

/// 宣言した関数の本体を定義する。1 つの失敗で残りを止めない。
fn define_all(
    module: &mut JITModule,
    functions: &[mono::Function],
    printers: &[mono::Function],
    checked: &crate::Checked,
    ids: &HashMap<usize, FuncId>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for function in functions.iter().chain(printers) {
        if let Err(error) = crate::emit::define_mono_function(module, function, checked, ids) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        return Ok(());
    }
    Err(errors)
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

/// 本体の自由な名前。字句に同じ綴りがあっても、ラムダの仮引数や field・
/// module member は値の参照ではない。構文として読めない入力は `None` で、
/// 仮引数を足さず本検査の診断に任せる。
fn expression_free_names(body: &str) -> Option<HashSet<String>> {
    let ast = parse_source(&format!("def main = {body}\n"), "repl").ok()?;
    let node = ast.items.iter().find_map(|item| match item {
        Item::Def(def) if def.name == "main" => match &def.kind {
            DefKind::Value(node) | DefKind::Function { body: node, .. } => Some(node),
        },
        _ => None,
    })?;
    let mut names = BTreeSet::new();
    crate::names::free_names(node, &BTreeSet::new(), &mut names);
    Some(names.into_iter().collect())
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
    if !dir.is_dir() {
        return Err(format!("library directory not found: {}", dir.display()));
    }
    let name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("library directory has no usable name: {}", dir.display()))?;
    if !is_namespace(name) {
        return Err(format!(
            "library directory name is not a namespace: {name}（§51、小文字で始まる字句）"
        ));
    }
    if !declares_library(dir)? {
        return Err(format!(
            "{} is not a §52 library: no `library` declaration（宣言の無いディレクトリは根の宣言として prelude と衝突する）",
            dir.display()
        ));
    }
    copy_tree(dir, &root.join(name))
}

/// §52: `<dir>` のどれかのファイルが `library` を宣言しているか。
fn declares_library(dir: &Path) -> Result<bool, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name.starts_with("__") {
            continue;
        }
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            if declares_library(&path)? {
                return Ok(true);
            }
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("hd" | "hdi")
        ) && std::fs::read_to_string(&path).is_ok_and(|text| {
            tokens(&text)
                .iter()
                .any(|token| matches!(token.kind, TokKind::KwLibrary))
        }) {
            return Ok(true);
        }
    }
    Ok(false)
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
    let balance = block_balance(&tokens);
    if balance.depth > 0 || balance.open > balance.close {
        return false;
    }
    !opens_next_input(&last.kind)
}

/// 括弧と、`fn`・`module`・`implement`・`effect`・`handler` の開き／閉じの数。
#[derive(Default)]
struct Balance {
    depth: i64,
    open: i64,
    close: i64,
}

fn block_balance(tokens: &[Tok]) -> Balance {
    let mut balance = Balance::default();
    for (index, token) in tokens.iter().enumerate() {
        let next = tokens.get(index + 1).map(|next| &next.kind);
        match &token.kind {
            TokKind::LParen | TokKind::LBracket | TokKind::LBrace => balance.depth += 1,
            TokKind::RParen | TokKind::RBracket | TokKind::RBrace => balance.depth -= 1,
            TokKind::KwFn if !matches!(next, Some(TokKind::LParen)) => balance.open += 1,
            TokKind::KwModule | TokKind::KwImplement | TokKind::KwEffect | TokKind::KwHandler => {
                balance.open += 1;
            }
            TokKind::Slash
                if matches!(
                    next,
                    Some(TokKind::KwFn | TokKind::KwModule | TokKind::KwImplement | TokKind::KwIf)
                ) =>
            {
                balance.close += 1;
            }
            _ => {}
        }
    }
    balance
}

/// その字句で終わる入力を、続きを待つ未完として扱うか（§14 の行頭の `|>` を
/// 含め、`=` は `def` の本体の前で切れるため開きに数える）。
fn opens_next_input(kind: &TokKind) -> bool {
    matches!(
        kind,
        TokKind::Arrow | TokKind::PipeArrow | TokKind::Pipe | TokKind::Comma | TokKind::Eq
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_binding_accepts_a_nbsp_before_the_name() {
        let (body, name) = trailing_binding("1 as\u{a0}x").expect("binding");
        assert_eq!(body, "1");
        assert_eq!(name, "x");
    }

    #[test]
    fn trailing_binding_accepts_an_ideographic_space_before_the_name() {
        let (body, name) = trailing_binding("1 as\u{3000}x").expect("binding");
        assert_eq!(body, "1");
        assert_eq!(name, "x");
    }
}
