//! Hand-built CLIF: `tail`-convention mutual recursion across arities.
//!
//! `f2` (2 params) and `g9` (9 params) call each other with `return_call`
//! while counting down `depth` and counting up `count`. The base case
//! returns the reached count, `hado_entry` prints it, and this test asserts
//! stdout equals the depth constant. A dead constant with a real depth of 1
//! prints `1` and fails here.

use cranelift_codegen::ir::{
    condcodes::IntCC, types, AbiParam, InstBuilder, Signature, UserFuncName,
};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{default_libcall_names, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::path::PathBuf;
use std::process::Command;

const DEPTH: i64 = 100_000_000;

fn make_isa() -> cranelift_codegen::isa::OwnedTargetIsa {
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "true").unwrap();
    flag_builder.set("preserve_frame_pointers", "true").unwrap();
    flag_builder.set("opt_level", "none").unwrap();
    let flags = settings::Flags::new(flag_builder);
    cranelift_native::builder().unwrap().finish(flags).unwrap()
}

fn workdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hado-tail-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn tail_sig(n_params: usize) -> Signature {
    let mut sig = Signature::new(CallConv::Tail);
    for _ in 0..n_params {
        sig.params.push(AbiParam::new(types::I64));
    }
    sig.returns.push(AbiParam::new(types::I64));
    sig
}

/// Define `f2`: `(depth, count) -> count reached at depth 0`, else
/// `return_call g9(depth - 1, count + 1, 0 * 7)`.
fn define_f2(module: &mut ObjectModule, f2: FuncIdAlias, g9: FuncIdAlias) {
    let mut ctx = module.make_context();
    ctx.func = cranelift_codegen::ir::Function::with_name_signature(
        UserFuncName::user(0, f2.as_u32()),
        tail_sig(2),
    );
    let mut func_ctx = FunctionBuilderContext::new();
    let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
    let entry = bcx.create_block();
    let base = bcx.create_block();
    let cont = bcx.create_block();
    bcx.append_block_params_for_function_params(entry);
    bcx.switch_to_block(entry);
    let params = bcx.block_params(entry).to_vec();
    let (depth, count) = (params[0], params[1]);
    let zero = bcx.ins().iconst(types::I64, 0);
    let done = bcx.ins().icmp(IntCC::Equal, depth, zero);
    bcx.ins()
        .brif(done, base, std::iter::empty(), cont, std::iter::empty());
    bcx.switch_to_block(base);
    bcx.ins().return_(&[count]);
    bcx.switch_to_block(cont);
    let one = bcx.ins().iconst(types::I64, 1);
    let depth1 = bcx.ins().isub(depth, one);
    let count1 = bcx.ins().iadd(count, one);
    let mut args = vec![depth1, count1];
    for _ in 0..7 {
        args.push(zero);
    }
    let callee = module.declare_func_in_func(g9.inner(), &mut bcx.func);
    bcx.ins().return_call(callee, &args);
    bcx.seal_all_blocks();
    bcx.finalize(module.target_config());
    module.define_function(f2.inner(), &mut ctx).unwrap();
}

/// Define `g9`: `(depth, count, _ * 7) -> count reached at depth 0`, else
/// `return_call f2(depth - 1, count + 1)`.
fn define_g9(module: &mut ObjectModule, f2: FuncIdAlias, g9: FuncIdAlias) {
    let mut ctx = module.make_context();
    ctx.func = cranelift_codegen::ir::Function::with_name_signature(
        UserFuncName::user(0, g9.as_u32()),
        tail_sig(9),
    );
    let mut func_ctx = FunctionBuilderContext::new();
    let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
    let entry = bcx.create_block();
    let base = bcx.create_block();
    let cont = bcx.create_block();
    bcx.append_block_params_for_function_params(entry);
    bcx.switch_to_block(entry);
    let params = bcx.block_params(entry).to_vec();
    let (depth, count) = (params[0], params[1]);
    let zero = bcx.ins().iconst(types::I64, 0);
    let done = bcx.ins().icmp(IntCC::Equal, depth, zero);
    bcx.ins()
        .brif(done, base, std::iter::empty(), cont, std::iter::empty());
    bcx.switch_to_block(base);
    bcx.ins().return_(&[count]);
    bcx.switch_to_block(cont);
    let one = bcx.ins().iconst(types::I64, 1);
    let depth1 = bcx.ins().isub(depth, one);
    let count1 = bcx.ins().iadd(count, one);
    let callee = module.declare_func_in_func(f2.inner(), &mut bcx.func);
    bcx.ins().return_call(callee, &[depth1, count1]);
    bcx.seal_all_blocks();
    bcx.finalize(module.target_config());
    module.define_function(g9.inner(), &mut ctx).unwrap();
}

#[derive(Clone, Copy)]
struct FuncIdAlias(cranelift_module::FuncId);

impl FuncIdAlias {
    fn inner(self) -> cranelift_module::FuncId {
        self.0
    }
    fn as_u32(self) -> u32 {
        self.0.as_u32()
    }
}

#[test]
fn tail() {
    let isa = make_isa();
    let builder =
        ObjectBuilder::new(isa, "hado_tail".to_string(), default_libcall_names()).unwrap();
    let mut module = ObjectModule::new(builder);

    let mut print_sig = Signature::new(CallConv::SystemV);
    print_sig.params.push(AbiParam::new(types::I64));
    let print_id = module
        .declare_function("hado_print_int", Linkage::Import, &print_sig)
        .unwrap();

    let f2_id = FuncIdAlias(
        module
            .declare_function("tail_f2", Linkage::Local, &tail_sig(2))
            .unwrap(),
    );
    let g9_id = FuncIdAlias(
        module
            .declare_function("tail_g9", Linkage::Local, &tail_sig(9))
            .unwrap(),
    );

    let mut entry_sig = Signature::new(CallConv::SystemV);
    entry_sig.returns.push(AbiParam::new(types::I32));
    let entry_id = module
        .declare_function("hado_entry", Linkage::Export, &entry_sig)
        .unwrap();

    define_f2(&mut module, f2_id, g9_id);
    define_g9(&mut module, f2_id, g9_id);

    // `hado_entry`: call into the tail side, print the reached count, return 0.
    {
        let mut ctx = module.make_context();
        ctx.func = cranelift_codegen::ir::Function::with_name_signature(
            UserFuncName::user(0, entry_id.as_u32()),
            entry_sig,
        );
        let mut func_ctx = FunctionBuilderContext::new();
        let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = bcx.create_block();
        bcx.switch_to_block(block);
        let f2_ref = module.declare_func_in_func(f2_id.inner(), &mut bcx.func);
        let print_ref = module.declare_func_in_func(print_id, &mut bcx.func);
        let depth = bcx.ins().iconst(types::I64, DEPTH);
        let zero = bcx.ins().iconst(types::I64, 0);
        let call = bcx.ins().call(f2_ref, &[depth, zero]);
        let reached = bcx.inst_results(call)[0];
        bcx.ins().call(print_ref, &[reached]);
        let code = bcx.ins().iconst(types::I32, 0);
        bcx.ins().return_(&[code]);
        bcx.seal_all_blocks();
        bcx.finalize(module.target_config());
        module.define_function(entry_id, &mut ctx).unwrap();
    }

    let product = module.finish();
    let bytes = product.emit().unwrap();
    let dir = workdir();
    let obj = dir.join("tail.o");
    let exe = dir.join("tail");
    std::fs::write(&obj, bytes).unwrap();
    hado::link_executable(&obj, &exe).expect("link tail");
    let out = Command::new(&exe).output().expect("run tail");
    assert!(out.status.success(), "guest exit status: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), DEPTH.to_string());
}
