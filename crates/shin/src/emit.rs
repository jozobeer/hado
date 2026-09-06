use std::path::Path;

use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, Signature, UserFuncName, types as clif_types,
};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::lexer;
use crate::parser::{self, Expr};
use crate::types;

fn make_isa() -> Result<cranelift_codegen::isa::OwnedTargetIsa, String> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("is_pic", "true")
        .map_err(|e| format!("{e:?}"))?;
    flag_builder
        .set("preserve_frame_pointers", "true")
        .map_err(|e| format!("{e:?}"))?;
    flag_builder
        .set("opt_level", "none")
        .map_err(|e| format!("{e:?}"))?;
    let flags = settings::Flags::new(flag_builder);
    let isa = cranelift_native::builder()
        .map_err(|e| e.to_string())?
        .finish(flags)
        .map_err(|e| format!("{e:?}"))?;
    Ok(isa)
}

fn build_expr(bcx: &mut FunctionBuilder<'_>, expr: &Expr) -> cranelift_codegen::ir::Value {
    match expr {
        Expr::Int(n) => bcx.ins().iconst(clif_types::I64, *n),
        Expr::Add(l, r) => {
            let a = build_expr(bcx, l);
            let b = build_expr(bcx, r);
            bcx.ins().iadd(a, b)
        }
    }
}

/// Compile `def main = <int + int ...>` to an object file exposing
/// `shin_entry` (SystemV, Export). `shin_entry` calls the `tail`
/// convention `shin_main`, prints the `i64` result, and returns 0.
pub fn compile_to_object(source: &str, obj_path: &Path) -> Result<(), String> {
    let toks = lexer::lex(source)?;
    let prog = parser::parse(&toks)?;
    let ty = types::check(&prog)?;
    if ty != types::Ty::Int {
        return Err(format!("main must be Int, found {ty:?}"));
    }

    let isa = make_isa()?;
    let builder =
        ObjectBuilder::new(isa, "shin_walk".to_string(), default_libcall_names())
            .map_err(|e| format!("{e:?}"))?;
    let mut module = ObjectModule::new(builder);

    let mut print_sig = Signature::new(CallConv::SystemV);
    print_sig.params.push(AbiParam::new(clif_types::I64));
    let print_id = module
        .declare_function("shin_print_int", Linkage::Import, &print_sig)
        .map_err(|e| format!("{e:?}"))?;

    let mut main_sig = Signature::new(CallConv::Tail);
    main_sig.returns.push(AbiParam::new(clif_types::I64));
    let main_id = module
        .declare_function("shin_main", Linkage::Local, &main_sig)
        .map_err(|e| format!("{e:?}"))?;

    let mut entry_sig = Signature::new(CallConv::SystemV);
    entry_sig.returns.push(AbiParam::new(clif_types::I32));
    let entry_id = module
        .declare_function("shin_entry", Linkage::Export, &entry_sig)
        .map_err(|e| format!("{e:?}"))?;

    // `shin_main`: evaluate the body expression.
    {
        let mut ctx = module.make_context();
        ctx.func = Function::with_name_signature(
            UserFuncName::user(0, main_id.as_u32()),
            main_sig,
        );
        let mut func_ctx = FunctionBuilderContext::new();
        let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = bcx.create_block();
        bcx.append_block_params_for_function_params(block);
        bcx.switch_to_block(block);
        let v = build_expr(&mut bcx, &prog.body);
        bcx.ins().return_(&[v]);
        bcx.seal_all_blocks();
        bcx.finalize(module.target_config());
        module
            .define_function(main_id, &mut ctx)
            .map_err(|e| format!("{e:?}"))?;
    }

    // `shin_entry`: call `shin_main`, print, return 0.
    {
        let mut ctx = module.make_context();
        ctx.func = Function::with_name_signature(
            UserFuncName::user(0, entry_id.as_u32()),
            entry_sig,
        );
        let mut func_ctx = FunctionBuilderContext::new();
        let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = bcx.create_block();
        bcx.switch_to_block(block);
        let main_ref = module.declare_func_in_func(main_id, &mut bcx.func);
        let print_ref = module.declare_func_in_func(print_id, &mut bcx.func);
        let call = bcx.ins().call(main_ref, &[]);
        let result = bcx.inst_results(call)[0];
        bcx.ins().call(print_ref, &[result]);
        let code = bcx.ins().iconst(clif_types::I32, 0);
        bcx.ins().return_(&[code]);
        bcx.seal_all_blocks();
        bcx.finalize(module.target_config());
        module
            .define_function(entry_id, &mut ctx)
            .map_err(|e| format!("{e:?}"))?;
    }

    let product = module.finish();
    let bytes = product.emit().map_err(|e| format!("{e:?}"))?;
    std::fs::write(obj_path, bytes).map_err(|e| e.to_string())?;
    Ok(())
}
