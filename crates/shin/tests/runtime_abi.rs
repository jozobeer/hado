use shin::runtime_abi::{AbiType, RuntimeFunction};

#[test]
fn allocation_and_print_have_distinct_effects_and_signatures() {
    let alloc = RuntimeFunction::Alloc.declaration();
    assert_eq!(alloc.symbol, "shin_alloc");
    assert_eq!(alloc.parameters, &[AbiType::Size]);
    assert_eq!(alloc.result, Some(AbiType::Pointer));
    assert!(alloc.may_allocate);
    let print = RuntimeFunction::PrintInt.declaration();
    assert_eq!(print.symbol, "shin_print_int");
    assert_eq!(print.parameters, &[AbiType::I64]);
    assert_eq!(print.result, None);
    assert!(!print.may_allocate);
}

#[test]
fn mono_signatures_use_tail_convention_and_target_pointer_width() {
    let checked = shin::check_source("def main = 0", "t").unwrap();
    let source = shin::mono::Signature {
        parameters: vec![shin::mono::Type::Bool],
        result: shin::mono::Type::Int,
    };
    let signature =
        shin::emit::mono_signature(&source, &checked, cranelift_codegen::ir::types::I64).unwrap();
    assert_eq!(signature.call_conv, cranelift_codegen::isa::CallConv::Tail);
    assert_eq!(
        signature.params[0].value_type,
        cranelift_codegen::ir::types::I64
    );
    assert_eq!(
        signature.params[1].value_type,
        cranelift_codegen::ir::types::I8
    );
    assert_eq!(
        signature.returns[0].value_type,
        cranelift_codegen::ir::types::I64
    );
}

#[test]
fn runtime_signatures_use_system_v_and_target_sized_allocations() {
    use cranelift_codegen::ir::types::{I32, I64};
    for pointer in [I32, I64] {
        let alloc = shin::emit::runtime_signature(RuntimeFunction::Alloc, pointer);
        assert_eq!(alloc.call_conv, cranelift_codegen::isa::CallConv::SystemV);
        assert_eq!(alloc.params.len(), 1);
        assert_eq!(alloc.params[0].value_type, pointer);
        assert_eq!(alloc.returns[0].value_type, pointer);
        let print = shin::emit::runtime_signature(RuntimeFunction::PrintInt, pointer);
        assert_eq!(print.params.len(), 1);
        assert_eq!(print.params[0].value_type, I64);
        assert!(print.returns.is_empty());
    }
}
