use hado::{
    mono::Type,
    repr::{machine_type, MachineType},
};

#[test]
fn scalar_widths_follow_the_plan() {
    let checked = hado::check_source("def main = 0", "t").unwrap();
    for (ty, expected) in [
        (Type::Int, MachineType::I64),
        (Type::Float, MachineType::F64),
        (Type::Bool, MachineType::I8),
        (Type::Char, MachineType::I32),
        (Type::Unit, MachineType::I64),
        (Type::String, MachineType::Pointer),
    ] {
        assert_eq!(machine_type(&ty, &checked).unwrap(), expected);
    }
}

#[test]
fn payload_variants_share_a_pointer_representation() {
    let checked = hado::check_source(
        "type Flag = | On | Off\ntype Record = { count: Int }\ndef main = On == Off && { count = 0 }.count == 0",
        "t",
    )
    .unwrap();
    assert_eq!(
        machine_type(&Type::Data("Flag".into(), vec![]), &checked).unwrap(),
        MachineType::I64
    );
    assert_eq!(
        machine_type(&Type::Data("Option".into(), vec![Type::Int]), &checked).unwrap(),
        MachineType::Pointer
    );
    assert_eq!(
        machine_type(&Type::Data("Record".into(), vec![]), &checked).unwrap(),
        MachineType::Pointer
    );
}

#[test]
fn aggregate_layout_aligns_fields_and_total_size() {
    use hado::repr::aggregate_layout;
    let layout = aggregate_layout(
        &[MachineType::I8, MachineType::I64, MachineType::I32],
        8,
        false,
    )
    .unwrap();
    assert_eq!(layout.offsets, vec![0, 8, 16]);
    assert_eq!(layout.size, 24);
    assert_eq!(layout.alignment, 8);
    let tagged = aggregate_layout(&[MachineType::I8, MachineType::Pointer], 8, true).unwrap();
    assert_eq!(tagged.offsets, vec![8, 16]);
    assert_eq!(tagged.size, 24);
    let narrow = aggregate_layout(&[MachineType::I8, MachineType::Pointer], 4, false).unwrap();
    assert_eq!(narrow.offsets, vec![0, 4]);
    assert_eq!(narrow.size, 8);
    assert!(aggregate_layout(&[], 3, false).is_err());
}

#[test]
fn record_layout_specializes_fields_in_declaration_order() {
    let checked = hado::check_source(
        "type T Row = { flag: Bool; value: T; marker: Char }\ndef main = { flag = true; value = 0; marker = 'a' }",
        "t",
    )
    .unwrap();
    let layout =
        hado::repr::record_layout(&Type::Data("Row".into(), vec![Type::Int]), &checked, 8).unwrap();
    assert_eq!(layout.offsets, vec![0, 8, 16]);
    assert_eq!(layout.size, 24);
    let layout =
        hado::repr::record_layout(&Type::Data("Row".into(), vec![Type::Bool]), &checked, 8)
            .unwrap();
    assert_eq!(layout.offsets, vec![0, 1, 4]);
    assert_eq!(layout.size, 8);
    assert!(hado::repr::record_layout(&Type::Int, &checked, 8).is_err());
}

#[test]
fn constructor_layout_keeps_nullary_variants_boxed_when_required() {
    let checked = hado::check_source("type Flag = | On | Off\ndef main = On", "t").unwrap();
    let option = Type::Data("Option".into(), vec![Type::Bool]);
    let some = hado::repr::constructor_layout("Some", &option, &checked, 8)
        .unwrap()
        .unwrap();
    assert_eq!(some.offsets, vec![8]);
    assert_eq!(some.size, 16);
    let none = hado::repr::constructor_layout("None", &option, &checked, 8)
        .unwrap()
        .unwrap();
    assert!(none.offsets.is_empty());
    assert_eq!(none.size, 8);
    assert!(
        hado::repr::constructor_layout("On", &Type::Data("Flag".into(), vec![]), &checked, 8)
            .unwrap()
            .is_none()
    );
    assert!(hado::repr::constructor_layout("On", &option, &checked, 8).is_err());
}

#[test]
fn closure_layout_places_code_before_aligned_captures() {
    let checked = hado::check_source("def main = 0", "t").unwrap();
    let layout = hado::repr::closure_layout(&[Type::Bool, Type::Int], &checked, 8).unwrap();
    assert_eq!(layout.offsets, vec![0, 8, 16]);
    assert_eq!(layout.size, 24);
    let empty = hado::repr::closure_layout(&[], &checked, 8).unwrap();
    assert_eq!(empty.offsets, vec![0]);
    assert_eq!(empty.size, 8);
    let narrow = hado::repr::closure_layout(&[Type::String], &checked, 4).unwrap();
    assert_eq!(narrow.offsets, vec![0, 4]);
    assert_eq!(narrow.size, 8);
}

#[test]
fn function_abi_always_has_an_environment_parameter() {
    let checked = hado::check_source("def main = 0", "t").unwrap();
    let signature = hado::mono::Signature {
        parameters: vec![Type::Bool, Type::Float],
        result: Type::Char,
    };
    let abi = hado::repr::function_signature(&signature, &checked).unwrap();
    assert_eq!(
        abi.parameters,
        vec![MachineType::Pointer, MachineType::I8, MachineType::F64]
    );
    assert_eq!(abi.result, MachineType::I32);
}

#[test]
fn constructor_tags_follow_declaration_order_within_each_type() {
    let checked = hado::check_source("type First = | Zebra | Apple\ntype Second = | Yes | No\ndef main = Zebra == Apple && Yes == No", "t").unwrap();
    let first = Type::Data("First".into(), vec![]);
    let second = Type::Data("Second".into(), vec![]);
    assert_eq!(hado::repr::constructor_tag("Zebra", &first, &checked).unwrap(), 0);
    assert_eq!(hado::repr::constructor_tag("Apple", &first, &checked).unwrap(), 1);
    assert_eq!(hado::repr::constructor_tag("Yes", &second, &checked).unwrap(), 0);
    assert!(hado::repr::constructor_tag("Yes", &first, &checked).is_err());
}

#[test]
fn string_literals_encode_byte_lengths_in_target_order() {
    use hado::repr::{string_literal_data, ByteOrder};
    for (width, order, prefix) in [
        (4, ByteOrder::Little, vec![5, 0, 0, 0]),
        (4, ByteOrder::Big, vec![0, 0, 0, 5]),
        (8, ByteOrder::Little, vec![5, 0, 0, 0, 0, 0, 0, 0]),
        (8, ByteOrder::Big, vec![0, 0, 0, 0, 0, 0, 0, 5]),
    ] {
        let data = string_literal_data("a\0あ", width, order).unwrap();
        assert_eq!(&data[..width as usize], prefix);
        assert_eq!(&data[width as usize..], "a\0あ".as_bytes());
    }
    assert_eq!(string_literal_data("", 8, ByteOrder::Little).unwrap(), vec![0; 8]);
    assert!(string_literal_data("x", 2, ByteOrder::Little).is_err());
}
