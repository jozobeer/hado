use std::path::Path;

use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, Signature, UserFuncName, types as clif_types,
};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::parser::{Expr, Program};

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

/// Compile a lowered M0 `Program` (`def main = <int + int ...>`) to an
/// object file exposing `hado_entry` (SystemV, Export). `hado_entry` calls
/// the `tail` convention `hado_main`, prints the `i64` result, and returns 0.
pub fn compile_to_object(prog: &Program, obj_path: &Path) -> Result<(), String> {
    let isa = make_isa()?;
    let builder =
        ObjectBuilder::new(isa, "hado_walk".to_string(), default_libcall_names())
            .map_err(|e| format!("{e:?}"))?;
    let mut module = ObjectModule::new(builder);

    let print = crate::runtime_abi::RuntimeFunction::PrintInt.declaration();
    let print_sig = runtime_signature(crate::runtime_abi::RuntimeFunction::PrintInt, module.target_config().pointer_type());
    let print_id = module
        .declare_function(print.symbol, Linkage::Import, &print_sig)
        .map_err(|e| format!("{e:?}"))?;

    let mut main_sig = Signature::new(CallConv::Tail);
    main_sig.returns.push(AbiParam::new(clif_types::I64));
    let main_id = module
        .declare_function("hado_main", Linkage::Local, &main_sig)
        .map_err(|e| format!("{e:?}"))?;

    let mut entry_sig = Signature::new(CallConv::SystemV);
    entry_sig.returns.push(AbiParam::new(clif_types::I32));
    let entry_id = module
        .declare_function("hado_entry", Linkage::Export, &entry_sig)
        .map_err(|e| format!("{e:?}"))?;

    // `hado_main`: evaluate the body expression.
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

    // `hado_entry`: call `hado_main`, print, return 0.
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

pub fn mono_signature(
    source: &crate::mono::Signature,
    checked: &crate::Checked,
    pointer_type: cranelift_codegen::ir::Type,
) -> Result<Signature, String> {
    let source = crate::repr::function_signature(source, checked)?;
    let machine_type = |ty| match ty {
        crate::repr::MachineType::I8 => clif_types::I8,
        crate::repr::MachineType::I32 => clif_types::I32,
        crate::repr::MachineType::I64 => clif_types::I64,
        crate::repr::MachineType::F64 => clif_types::F64,
        crate::repr::MachineType::Pointer => pointer_type,
    };
    let mut signature = Signature::new(CallConv::Tail);
    signature.params = source.parameters.into_iter().map(|ty| AbiParam::new(machine_type(ty))).collect();
    signature.returns.push(AbiParam::new(machine_type(source.result)));
    Ok(signature)
}

pub fn lower_mono_function(
    source: &crate::mono::Function,
    checked: &crate::Checked,
) -> Result<Function, String> {
    let checked = &checked.for_codegen();
    let isa = make_isa()?;
    let signature = mono_signature(&source.signature, checked, isa.pointer_type())?;
    let mut function = Function::with_name_signature(UserFuncName::user(0, source.id.0 as u32), signature);
    let mut context = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut function, &mut context);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    let mut variables: std::collections::HashMap<_, _> = source.parameters.iter().copied()
        .zip(builder.block_params(entry).iter().skip(1).copied()).collect();
    let capture_types: Vec<_> = source.captures.iter().map(|(_, ty)| ty.clone()).collect();
    let layout = crate::repr::closure_layout(&capture_types, checked, isa.pointer_type().bytes())?;
    let environment = builder.block_params(entry)[0];
    for ((id, ty), offset) in source.captures.iter().zip(layout.offsets.iter().skip(1)) {
        let signature = mono_signature(&crate::mono::Signature { parameters: vec![], result: ty.clone() }, checked, isa.pointer_type())?;
        let value = builder.ins().load(signature.returns[0].value_type, cranelift_codegen::ir::MemFlagsData::new(), environment, i32::try_from(*offset).map_err(|_| "capture offset overflow")?);
        variables.insert(*id, value);
    }
    build_mono_return(&mut builder, &source.body, &mut variables, checked)?;
    builder.seal_all_blocks();
    builder.finalize(isa.frontend_config());
    Ok(function)
}

fn prepare_direct_call(
    builder: &mut FunctionBuilder<'_>,
    function: crate::mono::FunctionId,
    arguments: &[crate::mono::Expression],
    result: &crate::mono::Type,
    variables: &mut std::collections::HashMap<usize, cranelift_codegen::ir::Value>,
    checked: &crate::Checked,
) -> Result<(cranelift_codegen::ir::FuncRef, Vec<cranelift_codegen::ir::Value>), String> {
    use crate::mono::Expression;
            let pointer_type = builder.func.signature.params[0].value_type;
            let source = crate::mono::Signature { parameters: arguments.iter().map(Expression::ty).collect(), result: result.clone() };
            let signature = mono_signature(&source, checked, pointer_type)?;
            let callee = import_function(builder, 0, u32::try_from(function.0).map_err(|_| "function ID overflow")?, signature);
            let mut values = vec![builder.ins().iconst(pointer_type, 0)];
            for argument in arguments { values.push(build_mono_expr(builder, argument, variables, checked)?); }
    Ok((callee, values))
}

fn build_mono_return(
    builder: &mut FunctionBuilder<'_>,
    expression: &crate::mono::Expression,
    variables: &mut std::collections::HashMap<usize, cranelift_codegen::ir::Value>,
    checked: &crate::Checked,
) -> Result<(), String> {
    use crate::mono::Expression;
    match expression {
        Expression::Switch { value, arms, fallback, .. } => {
            let ty = value.ty();
            let value = build_mono_expr(builder, value, variables, checked)?;
            let tag = if crate::repr::machine_type(&ty, checked)? == crate::repr::MachineType::Pointer {
                builder.ins().load(clif_types::I64, cranelift_codegen::ir::MemFlagsData::new(), value, 0)
            } else { value };
            for (constructor, body) in arms {
                let expected = crate::repr::constructor_tag(constructor, &ty, checked)?;
                let matches = builder.ins().icmp_imm_u(cranelift_codegen::ir::condcodes::IntCC::Equal, tag, i64::from(expected));
                let yes = builder.create_block();
                let next = builder.create_block();
                builder.ins().brif(matches, yes, &[], next, &[]);
                builder.switch_to_block(yes);
                build_mono_return(builder, body, variables, checked)?;
                builder.switch_to_block(next);
            }
            if let Some(fallback) = fallback {
                build_mono_return(builder, fallback, variables, checked)?;
            } else {
                builder.ins().trap(cranelift_codegen::ir::TrapCode::user(1).unwrap());
            }
        }
        Expression::Unreachable { .. } => {
            builder.ins().trap(cranelift_codegen::ir::TrapCode::user(1).unwrap());
        }
        Expression::TailCallClosure { closure, arguments, result } => {
            let (signature, code, values) = prepare_closure_call(builder, closure, arguments, result, variables, checked)?;
            builder.ins().return_call_indirect(signature, code, &values);
        }
        Expression::TailCall { function, arguments, result } => {
            let (callee, values) = prepare_direct_call(builder, *function, arguments, result, variables, checked)?;
            builder.ins().return_call(callee, &values);
        }
        Expression::Let { id, value, body } => {
            let value = build_mono_expr(builder, value, variables, checked)?;
            let saved = variables.insert(*id, value);
            let result = build_mono_return(builder, body, variables, checked);
            if let Some(value) = saved { variables.insert(*id, value); } else { variables.remove(id); }
            result?;
        }
        Expression::Branch { condition, then, otherwise } => {
            let condition = build_mono_expr(builder, condition, variables, checked)?;
            let yes = builder.create_block();
            let no = builder.create_block();
            builder.ins().brif(condition, yes, &[], no, &[]);
            builder.switch_to_block(yes);
            build_mono_return(builder, then, variables, checked)?;
            builder.switch_to_block(no);
            build_mono_return(builder, otherwise, variables, checked)?;
        }
        _ => {
            let value = build_mono_expr(builder, expression, variables, checked)?;
            builder.ins().return_(&[value]);
        }
    }
    Ok(())
}

fn build_mono_expr(
    builder: &mut FunctionBuilder<'_>,
    expression: &crate::mono::Expression,
    variables: &mut std::collections::HashMap<usize, cranelift_codegen::ir::Value>,
    checked: &crate::Checked,
) -> Result<cranelift_codegen::ir::Value, String> {
    use crate::mono::{Constant, Expression};
    use crate::parser::BinOp;
    Ok(match expression {
        Expression::CallRuntime { function, arguments, result } if function.is_polymorphic() => {
            build_array_intrinsic(builder, *function, arguments, result, variables, checked)?
        }
        Expression::CallRuntime { function, arguments, .. } => {
            let pointer = builder.func.signature.params[0].value_type;
            let callee = import_function(builder, 1, function.id(), runtime_signature(*function, pointer));
            let mut values = Vec::new();
            for argument in arguments { values.push(build_mono_expr(builder, argument, variables, checked)?); }
            let call = builder.ins().call(callee, &values);
            if function.declaration().result.is_some() {
                builder.inst_results(call)[0]
            } else {
                builder.ins().iconst(clif_types::I64, 0)
            }
        }
        Expression::Unreachable { result } => {
            let pointer = builder.func.signature.params[0].value_type;
            let signature = mono_signature(&crate::mono::Signature { parameters: vec![], result: result.clone() }, checked, pointer)?;
            builder.ins().trap(cranelift_codegen::ir::TrapCode::user(1).unwrap());
            // A predecessor-free continuation supplies a typed value without executing it.
            let continuation = builder.create_block();
            builder.append_block_param(continuation, signature.returns[0].value_type);
            builder.switch_to_block(continuation);
            builder.block_params(continuation)[0]
        }
        Expression::Switch { value, arms, fallback, result } => {
            let ty = value.ty();
            let value = build_mono_expr(builder, value, variables, checked)?;
            let tag = if crate::repr::machine_type(&ty, checked)? == crate::repr::MachineType::Pointer {
                builder.ins().load(clif_types::I64, cranelift_codegen::ir::MemFlagsData::new(), value, 0)
            } else { value };
            let pointer = builder.func.signature.params[0].value_type;
            let signature = mono_signature(&crate::mono::Signature { parameters: vec![], result: result.clone() }, checked, pointer)?;
            let merge = builder.create_block();
            builder.append_block_param(merge, signature.returns[0].value_type);
            for (constructor, body) in arms {
                let expected = crate::repr::constructor_tag(constructor, &ty, checked)?;
                let matches = builder.ins().icmp_imm_u(cranelift_codegen::ir::condcodes::IntCC::Equal, tag, i64::from(expected));
                let yes = builder.create_block();
                let next = builder.create_block();
                builder.ins().brif(matches, yes, &[], next, &[]);
                builder.switch_to_block(yes);
                let value = build_mono_expr(builder, body, variables, checked)?;
                builder.ins().jump(merge, &[value.into()]);
                builder.switch_to_block(next);
            }
            if let Some(fallback) = fallback {
                let value = build_mono_expr(builder, fallback, variables, checked)?;
                builder.ins().jump(merge, &[value.into()]);
            } else {
                builder.ins().trap(cranelift_codegen::ir::TrapCode::user(1).unwrap());
            }
            builder.switch_to_block(merge);
            builder.block_params(merge)[0]
        }
        Expression::Constant(Constant::String(value)) => {
            let constant = builder.func.dfg.constants.insert(value.as_bytes().to_vec().into());
            let reference = builder.func.declare_imported_user_function(cranelift_codegen::ir::UserExternalName { namespace: 2, index: constant.as_u32() });
            let global = builder.func.create_global_value(cranelift_codegen::ir::GlobalValueData::Symbol {
                name: cranelift_codegen::ir::ExternalName::user(reference), offset: 0.into(), colocated: true, tls: false,
            });
            let pointer = builder.func.signature.params[0].value_type;
            builder.ins().symbol_value(pointer, global)
        }
        Expression::Constant(Constant::Int(value)) => builder.ins().iconst(clif_types::I64, *value),
        Expression::Constant(Constant::Bool(value)) => builder.ins().iconst(clif_types::I8, i64::from(*value)),
        Expression::Constant(Constant::Char(value)) => builder.ins().iconst(clif_types::I32, i64::from(*value as u32)),
        Expression::Constant(Constant::Unit) => builder.ins().iconst(clif_types::I64, 0),
        Expression::Constant(Constant::Float(value)) => builder.ins().f64const(*value),
        Expression::Variable { id, .. } => *variables.get(id).ok_or("unknown IR variable")?,
        Expression::Let { id, value, body } => {
            let value = build_mono_expr(builder, value, variables, checked)?;
            let saved = variables.insert(*id, value);
            let result = build_mono_expr(builder, body, variables, checked);
            if let Some(value) = saved { variables.insert(*id, value); } else { variables.remove(id); }
            result?
        }
        Expression::Construct { constructor, fields, result } => {
            let pointer = builder.func.signature.params[0].value_type;
            let tag = crate::repr::constructor_tag(constructor, result, checked)?;
            let layout = crate::repr::constructor_layout(constructor, result, checked, pointer.bytes())?;
            if let Some(layout) = layout {
                if fields.len() != layout.offsets.len() { return Err("constructor field count mismatch".into()); }
                let mut values = Vec::new();
                for field in fields { values.push(build_mono_expr(builder, field, variables, checked)?); }
                let object = allocate(builder, layout.size)?;
                let tag = builder.ins().iconst(clif_types::I64, i64::from(tag));
                builder.ins().store(cranelift_codegen::ir::MemFlagsData::new(), tag, object, 0);
                for (value, offset) in values.into_iter().zip(layout.offsets) {
                    let offset = i32::try_from(offset).map_err(|_| "constructor offset overflow")?;
                    builder.ins().store(cranelift_codegen::ir::MemFlagsData::new(), value, object, offset);
                }
                object
            } else {
                if !fields.is_empty() { return Err("immediate constructor has fields".into()); }
                builder.ins().iconst(clif_types::I64, i64::from(tag))
            }
        }
        Expression::Project { value, constructor, index, result } => {
            let pointer = builder.func.signature.params[0].value_type;
            let layout = crate::repr::constructor_layout(constructor, &value.ty(), checked, pointer.bytes())?
                .ok_or("cannot project an immediate constructor")?;
            let offset = *layout.offsets.get(*index).ok_or("constructor field index out of range")?;
            let offset = i32::try_from(offset).map_err(|_| "constructor offset overflow")?;
            let signature = mono_signature(&crate::mono::Signature { parameters: vec![], result: result.clone() }, checked, pointer)?;
            let object = build_mono_expr(builder, value, variables, checked)?;
            builder.ins().load(signature.returns[0].value_type, cranelift_codegen::ir::MemFlagsData::new(), object, offset)
        }
        Expression::Record { fields, result } => {
            let pointer = builder.func.signature.params[0].value_type;
            let layout = crate::repr::record_layout(result, checked, pointer.bytes())?;
            let mut values = Vec::new();
            for (name, expression) in fields {
                let index = record_field_index(result, name, checked)?;
                let value = build_mono_expr(builder, expression, variables, checked)?;
                values.push((index, value));
            }
            let record = allocate(builder, layout.size)?;
            for (index, value) in values {
                let offset = i32::try_from(layout.offsets[index]).map_err(|_| "record offset overflow")?;
                builder.ins().store(cranelift_codegen::ir::MemFlagsData::new(), value, record, offset);
            }
            record
        }
        Expression::RecordGet { record, field, result } => {
            let pointer = builder.func.signature.params[0].value_type;
            let layout = crate::repr::record_layout(&record.ty(), checked, pointer.bytes())?;
            let index = record_field_index(&record.ty(), field, checked)?;
            let offset = i32::try_from(layout.offsets[index]).map_err(|_| "record offset overflow")?;
            let signature = mono_signature(&crate::mono::Signature { parameters: vec![], result: result.clone() }, checked, pointer)?;
            let record = build_mono_expr(builder, record, variables, checked)?;
            builder.ins().load(signature.returns[0].value_type, cranelift_codegen::ir::MemFlagsData::new(), record, offset)
        }
        Expression::MakeClosure { captures, function, signature } => {
            let pointer = builder.func.signature.params[0].value_type;
            let types: Vec<_> = captures.iter().map(Expression::ty).collect();
            let layout = crate::repr::closure_layout(&types, checked, pointer.bytes())?;
            let mut values = Vec::new();
            for capture in captures { values.push(build_mono_expr(builder, capture, variables, checked)?); }
            let allocation = allocate(builder, layout.size)?;
            let signature = mono_signature(signature, checked, pointer)?;
            let callee = import_function(builder, 0, u32::try_from(function.0).map_err(|_| "function ID overflow")?, signature);
            let code = builder.ins().func_addr(pointer, callee);
            builder.ins().store(cranelift_codegen::ir::MemFlagsData::new(), code, allocation, 0);
            for (value, offset) in values.into_iter().zip(layout.offsets.iter().skip(1)) {
                builder.ins().store(cranelift_codegen::ir::MemFlagsData::new(), value, allocation, i32::try_from(*offset).map_err(|_| "capture offset overflow")?);
            }
            allocation
        }
        Expression::CallClosure { closure, arguments, result } => {
            let (signature, code, values) = prepare_closure_call(builder, closure, arguments, result, variables, checked)?;
            let call = builder.ins().call_indirect(signature, code, &values);
            builder.inst_results(call)[0]
        }
        Expression::CallDirect { function, arguments, result } => {
            let (callee, values) = prepare_direct_call(builder, *function, arguments, result, variables, checked)?;
            let call = builder.ins().call(callee, &values);
            builder.inst_results(call)[0]
        }
        Expression::Unary { op, value } => {
            let float = value.ty() == crate::mono::Type::Float;
            let value = build_mono_expr(builder, value, variables, checked)?;
            match op {
                crate::mono::UnaryOp::Not => builder.ins().icmp_imm_u(cranelift_codegen::ir::condcodes::IntCC::Equal, value, 0),
                crate::mono::UnaryOp::Negate if float => builder.ins().fneg(value),
                crate::mono::UnaryOp::Negate => builder.ins().ineg(value),
            }
        }
        Expression::Branch { condition, then, otherwise } => {
            let condition = build_mono_expr(builder, condition, variables, checked)?;
            let yes = builder.create_block();
            let no = builder.create_block();
            let merge = builder.create_block();
            builder.ins().brif(condition, yes, &[], no, &[]);
            builder.switch_to_block(yes);
            let then = build_mono_expr(builder, then, variables, checked)?;
            let ty = builder.func.dfg.value_type(then);
            builder.append_block_param(merge, ty);
            builder.ins().jump(merge, &[then.into()]);
            builder.switch_to_block(no);
            let otherwise = build_mono_expr(builder, otherwise, variables, checked)?;
            builder.ins().jump(merge, &[otherwise.into()]);
            builder.switch_to_block(merge);
            builder.block_params(merge)[0]
        }
        Expression::Binary { op, left, right } => {
            let float = left.ty() == crate::mono::Type::Float;
            let signed = left.ty() == crate::mono::Type::Int;
            let string = left.ty() == crate::mono::Type::String || left.ty() == crate::mono::Type::Bytes;
            let left = build_mono_expr(builder, left, variables, checked)?;
            let right = build_mono_expr(builder, right, variables, checked)?;
            if string {
                return emit_string_comparison(builder, *op, left, right);
            }
            match (op, float) {
                (BinOp::Add, false) => builder.ins().iadd(left, right),
                (BinOp::Sub, false) => builder.ins().isub(left, right),
                (BinOp::Mul, false) => builder.ins().imul(left, right),
                (BinOp::Add, true) => builder.ins().fadd(left, right),
                (BinOp::Sub, true) => builder.ins().fsub(left, right),
                (BinOp::Mul, true) => builder.ins().fmul(left, right),
                (BinOp::Div, true) => builder.ins().fdiv(left, right),
                (BinOp::Div | BinOp::Mod, false) => emit_integer_division(builder, left, right, *op == BinOp::Mod, checked)?,
                (op, false) => {
                    use cranelift_codegen::ir::condcodes::IntCC;
                    let code = match op {
                        BinOp::Eq => IntCC::Equal, BinOp::Ne => IntCC::NotEqual,
                        BinOp::Lt if signed => IntCC::SignedLessThan,
                        BinOp::Le if signed => IntCC::SignedLessThanOrEqual,
                        BinOp::Gt if signed => IntCC::SignedGreaterThan,
                        BinOp::Ge if signed => IntCC::SignedGreaterThanOrEqual,
                        BinOp::Lt => IntCC::UnsignedLessThan, BinOp::Le => IntCC::UnsignedLessThanOrEqual,
                        BinOp::Gt => IntCC::UnsignedGreaterThan, BinOp::Ge => IntCC::UnsignedGreaterThanOrEqual,
                        _ => return Err("operator code generation is not implemented".into()),
                    };
                    builder.ins().icmp(code, left, right)
                }
                (op, true) => {
                    use cranelift_codegen::ir::condcodes::FloatCC;
                    let code = match op {
                        BinOp::Eq => FloatCC::Equal, BinOp::Ne => FloatCC::NotEqual,
                        BinOp::Lt => FloatCC::LessThan, BinOp::Le => FloatCC::LessThanOrEqual,
                        BinOp::Gt => FloatCC::GreaterThan, BinOp::Ge => FloatCC::GreaterThanOrEqual,
                        _ => return Err("operator code generation is not implemented".into()),
                    };
                    builder.ins().fcmp(code, left, right)
                }
            }
        }
        _ => return Err("IR expression code generation is not implemented".into()),
    })
}

pub fn compile_mono_to_object(
    program: &crate::mono::Program,
    checked: &crate::Checked,
    path: &Path,
) -> Result<(), String> {
    let checked = &checked.for_codegen();
    use crate::mono::{FunctionId, Signature as MonoSignature};
    let mut module = ObjectModule::new(
        ObjectBuilder::new(make_isa()?, "hado", default_libcall_names())
            .map_err(|e| e.to_string())?,
    );
    let mut ids = std::collections::HashMap::new();
    for function in &program.functions {
        let signature = mono_signature(&function.signature, checked, module.target_config().pointer_type())?;
        let id = module.declare_function(&format!("hado_fn_{}", function.id.0), Linkage::Local, &signature)
            .map_err(|e| e.to_string())?;
        if ids.insert(function.id.0, id).is_some() {
            return Err("duplicate mono function ID".into());
        }
    }
    for function in &program.functions {
        define_mono_function(&mut module, function, checked, &ids)?;
    }
    let main = crate::mono::Function {
        id: FunctionId(program.functions.iter().map(|f| f.id.0).max().map_or(0, |id| id + 1)),
        name: "hado_main".into(), captures: vec![], parameters: vec![],
        signature: MonoSignature { parameters: vec![], result: program.entry.ty() },
        body: program.entry.clone(),
    };
    let signature = mono_signature(&main.signature, checked, module.target_config().pointer_type())?;
    let main_id = module.declare_function("hado_main", Linkage::Local, &signature).map_err(|e| e.to_string())?;
    ids.insert(main.id.0, main_id);
    define_mono_function(&mut module, &main, checked, &ids)?;
    let printers = crate::mono::derive_printers(FunctionId(main.id.0 + 1), program.entry.ty(), checked)?;
    for printer in &printers {
        let signature = mono_signature(&printer.signature, checked, module.target_config().pointer_type())?;
        let id = module.declare_function(&format!("hado_printer_{}", printer.id.0), Linkage::Local, &signature).map_err(|e| e.to_string())?;
        ids.insert(printer.id.0, id);
    }
    for printer in &printers { define_mono_function(&mut module, printer, checked, &ids)?; }
    let print_id = ids[&printers[0].id.0];
    define_int_entry(&mut module, main_id, print_id)?;
    let bytes = module.finish().emit().map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

fn define_mono_function(
    module: &mut ObjectModule,
    source: &crate::mono::Function,
    checked: &crate::Checked,
    ids: &std::collections::HashMap<usize, cranelift_module::FuncId>,
) -> Result<(), String> {
    let mut context = module.make_context();
    context.func = lower_mono_function(source, checked)?;
    let names: Vec<_> = context.func.params.user_named_funcs().iter()
        .map(|(reference, name)| (reference, name.clone())).collect();
    for (reference, name) in names {
        if name.namespace == 2 {
            let constant = cranelift_codegen::ir::Constant::from_u32(name.index);
            let value = std::str::from_utf8(context.func.dfg.constants.get(constant).as_slice()).map_err(|e| e.to_string())?;
            let order = if cfg!(target_endian = "little") { crate::repr::ByteOrder::Little } else { crate::repr::ByteOrder::Big };
            let bytes = crate::repr::string_literal_data(value, module.target_config().pointer_type().bytes(), order)?;
            let id = module.declare_anonymous_data(false, false).map_err(|e| e.to_string())?;
            let mut data = cranelift_module::DataDescription::new();
            data.define(bytes.into_boxed_slice());
            data.set_align(u64::from(module.target_config().pointer_type().bytes()));
            module.define_data(id, &data).map_err(|e| e.to_string())?;
            context.func.params.reset_user_func_name(reference, cranelift_codegen::ir::UserExternalName { namespace: 1, index: id.as_u32() });
            continue;
        }
        let id = match name.namespace {
            0 => *ids.get(&(name.index as usize)).ok_or("missing mono callee")?,
            1 => {
                let function = crate::runtime_abi::RuntimeFunction::from_id(name.index).ok_or("unknown runtime function")?;
                let declaration = function.declaration();
                let signature = runtime_signature(function, module.target_config().pointer_type());
                module.declare_function(declaration.symbol, Linkage::Import, &signature).map_err(|e| e.to_string())?
            }
            _ => return Err("unknown external function".into()),
        };
        context.func.params.reset_user_func_name(reference, cranelift_codegen::ir::UserExternalName {
            namespace: 0, index: id.as_u32(),
        });
    }
    let id = *ids.get(&source.id.0).ok_or("missing mono function")?;
    context.func.name = UserFuncName::user(0, id.as_u32());
    module.define_function(id, &mut context).map_err(|e| e.to_string())
}

fn define_int_entry(module: &mut ObjectModule, main: cranelift_module::FuncId, print: cranelift_module::FuncId) -> Result<(), String> {
    let mut context = module.make_context();
    context.func.signature = Signature::new(CallConv::SystemV);
    context.func.signature.returns.push(AbiParam::new(clif_types::I32));
    let entry = module.declare_function("hado_entry", Linkage::Export, &context.func.signature).map_err(|e| e.to_string())?;
    let mut frontend = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut context.func, &mut frontend);
    let block = builder.create_block();
    builder.switch_to_block(block);
    let main = module.declare_func_in_func(main, builder.func);
    let print = module.declare_func_in_func(print, builder.func);
    let environment = builder.ins().iconst(module.target_config().pointer_type(), 0);
    let call = builder.ins().call(main, &[environment]);
    let result = builder.inst_results(call)[0];
    let printed = builder.ins().call(print, &[environment, result]);
    let status = builder.inst_results(printed)[0];
    let code = builder.ins().ireduce(clif_types::I32, status);
    builder.ins().return_(&[code]);
    builder.seal_all_blocks();
    builder.finalize(module.target_config());
    module.define_function(entry, &mut context).map_err(|e| e.to_string())
}

fn import_function(builder: &mut FunctionBuilder<'_>, namespace: u32, index: u32, signature: Signature) -> cranelift_codegen::ir::FuncRef {
    let signature = builder.import_signature(signature);
    let reference = builder.func.declare_imported_user_function(cranelift_codegen::ir::UserExternalName { namespace, index });
    builder.import_function(cranelift_codegen::ir::ExtFuncData {
        name: cranelift_codegen::ir::ExternalName::user(reference), signature, colocated: namespace == 0, patchable: false,
    })
}

pub fn runtime_signature(function: crate::runtime_abi::RuntimeFunction, pointer: cranelift_codegen::ir::Type) -> Signature {
    let declaration = function.declaration();
    let machine_type = |ty| match ty {
        crate::runtime_abi::AbiType::F64 => clif_types::F64,
        crate::runtime_abi::AbiType::I8 => clif_types::I8,
        crate::runtime_abi::AbiType::I32 => clif_types::I32,
        crate::runtime_abi::AbiType::I64 => clif_types::I64,
        crate::runtime_abi::AbiType::Size | crate::runtime_abi::AbiType::Pointer => pointer,
    };
    let mut signature = Signature::new(CallConv::SystemV);
    signature.params.extend(declaration.parameters.iter().map(|ty| AbiParam::new(machine_type(*ty))));
    signature.returns.extend(declaration.result.map(|ty| AbiParam::new(machine_type(ty))));
    signature
}

fn build_array_intrinsic(
    builder: &mut FunctionBuilder<'_>,
    function: crate::runtime_abi::RuntimeFunction,
    arguments: &[crate::mono::Expression],
    result: &crate::mono::Type,
    variables: &mut std::collections::HashMap<usize, cranelift_codegen::ir::Value>,
    checked: &crate::Checked,
) -> Result<cranelift_codegen::ir::Value, String> {
    let mut values = Vec::new();
    for argument in arguments {
        values.push(build_mono_expr(builder, argument, variables, checked)?);
    }
    use crate::runtime_abi::RuntimeFunction;
    match function {
        RuntimeFunction::ArrayLength | RuntimeFunction::ArrayAlloc => {
            let pointer = builder.func.signature.params[0].value_type;
            let callee = import_function(builder, 1, function.id(), runtime_signature(function, pointer));
            let call = builder.ins().call(callee, &values);
            Ok(builder.inst_results(call)[0])
        }
        RuntimeFunction::ArrayGet | RuntimeFunction::ArrayFillUnchecked => {
            let stride = builder.ins().iconst(clif_types::I64, 8);
            let offset = builder.ins().imul(values[1], stride);
            let address = builder.ins().iadd(values[0], offset);
            if function == RuntimeFunction::ArrayFillUnchecked {
                builder.ins().store(cranelift_codegen::ir::MemFlagsData::new(), values[2], address, 8);
                Ok(values[0])
            } else {
                let pointer = builder.func.signature.params[0].value_type;
                let signature = mono_signature(&crate::mono::Signature { parameters: vec![], result: result.clone() }, checked, pointer)?;
                Ok(builder.ins().load(signature.returns[0].value_type, cranelift_codegen::ir::MemFlagsData::new(), address, 8))
            }
        }
        _ => Err("expected array intrinsic".into()),
    }
}

fn allocate(builder: &mut FunctionBuilder<'_>, size: u32) -> Result<cranelift_codegen::ir::Value, String> {
    let pointer = builder.func.signature.params[0].value_type;
    let callee = import_function(builder, 1, crate::runtime_abi::RuntimeFunction::Alloc.id(), runtime_signature(crate::runtime_abi::RuntimeFunction::Alloc, pointer));
    let size = builder.ins().iconst(pointer, i64::from(size));
    let call = builder.ins().call(callee, &[size]);
    Ok(builder.inst_results(call)[0])
}

fn prepare_closure_call(
    builder: &mut FunctionBuilder<'_>,
    closure: &crate::mono::Expression,
    arguments: &[crate::mono::Expression],
    result: &crate::mono::Type,
    variables: &mut std::collections::HashMap<usize, cranelift_codegen::ir::Value>,
    checked: &crate::Checked,
) -> Result<(cranelift_codegen::ir::SigRef, cranelift_codegen::ir::Value, Vec<cranelift_codegen::ir::Value>), String> {
    let pointer = builder.func.signature.params[0].value_type;
    let environment = build_mono_expr(builder, closure, variables, checked)?;
    let code = builder.ins().load(pointer, cranelift_codegen::ir::MemFlagsData::new(), environment, 0);
    let source = crate::mono::Signature { parameters: arguments.iter().map(crate::mono::Expression::ty).collect(), result: result.clone() };
    let signature = mono_signature(&source, checked, pointer)?;
    let signature = builder.import_signature(signature);
    let mut values = vec![environment];
    for argument in arguments { values.push(build_mono_expr(builder, argument, variables, checked)?); }
    Ok((signature, code, values))
}

fn record_field_index(ty: &crate::mono::Type, field: &str, checked: &crate::Checked) -> Result<usize, String> {
    let crate::mono::Type::Data(name, _) = ty else { return Err("expected record type".into()); };
    let (fields, _) = checked.records.get(name).ok_or("unknown record type")?;
    fields.iter().position(|name| name == field).ok_or_else(|| format!("unknown record field {field}"))
}

fn emit_integer_division(
    builder: &mut FunctionBuilder<'_>,
    left: cranelift_codegen::ir::Value,
    right: cranelift_codegen::ir::Value,
    remainder: bool,
    checked: &crate::Checked,
) -> Result<cranelift_codegen::ir::Value, String> {
    use cranelift_codegen::ir::condcodes::IntCC;
    let pointer = builder.func.signature.params[0].value_type;
    let error = builder.create_block();
    let nonzero = builder.create_block();
    let wrap = builder.create_block();
    let divide = builder.create_block();
    let merge = builder.create_block();
    builder.append_block_param(merge, clif_types::I64);
    let zero = builder.ins().icmp_imm_u(IntCC::Equal, right, 0);
    builder.ins().brif(zero, error, &[], nonzero, &[]);
    builder.switch_to_block(error);
    let message = build_mono_expr(
        builder,
        &crate::mono::Expression::Constant(crate::mono::Constant::String("DivisionByZero".into())),
        &mut std::collections::HashMap::new(),
        checked,
    )?;
    let abort = import_function(
        builder,
        1,
        crate::runtime_abi::RuntimeFunction::AbortError.id(),
        runtime_signature(crate::runtime_abi::RuntimeFunction::AbortError, pointer),
    );
    builder.ins().call(abort, &[message]);
    builder.ins().trap(cranelift_codegen::ir::TrapCode::user(1).unwrap());
    builder.switch_to_block(nonzero);
    let minimum = builder.ins().icmp_imm_u(IntCC::Equal, left, i64::MIN);
    let negative_one = builder.ins().icmp_imm_u(IntCC::Equal, right, -1);
    let overflow = builder.ins().band(minimum, negative_one);
    builder.ins().brif(overflow, wrap, &[], divide, &[]);
    builder.switch_to_block(wrap);
    let value = builder.ins().iconst(clif_types::I64, if remainder { 0 } else { i64::MIN });
    builder.ins().jump(merge, &[value.into()]);
    builder.switch_to_block(divide);
    let value = if remainder { builder.ins().srem(left, right) } else { builder.ins().sdiv(left, right) };
    builder.ins().jump(merge, &[value.into()]);
    builder.switch_to_block(merge);
    Ok(builder.block_params(merge)[0])
}

fn emit_string_comparison(
    builder: &mut FunctionBuilder<'_>,
    op: crate::parser::BinOp,
    left: cranelift_codegen::ir::Value,
    right: cranelift_codegen::ir::Value,
) -> Result<cranelift_codegen::ir::Value, String> {
    use crate::parser::BinOp;
    use cranelift_codegen::ir::condcodes::IntCC;
    let condition = match op {
        BinOp::Eq => IntCC::Equal,
        BinOp::Ne => IntCC::NotEqual,
        BinOp::Lt => IntCC::SignedLessThan,
        BinOp::Le => IntCC::SignedLessThanOrEqual,
        BinOp::Gt => IntCC::SignedGreaterThan,
        BinOp::Ge => IntCC::SignedGreaterThanOrEqual,
        _ => return Err("invalid String operator".into()),
    };
    let function = crate::runtime_abi::RuntimeFunction::StringCompare;
    let pointer = builder.func.signature.params[0].value_type;
    let callee = import_function(builder, 1, function.id(), runtime_signature(function, pointer));
    let call = builder.ins().call(callee, &[left, right]);
    let order = builder.inst_results(call)[0];
    Ok(builder.ins().icmp_imm_u(condition, order, 0))
}
