use crate::{mono::Type, types::Ty, Checked};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineType {
    I8,
    I32,
    I64,
    F64,
    Pointer,
}

pub fn machine_type(ty: &Type, checked: &Checked) -> Result<MachineType, String> {
    Ok(match ty {
        Type::Int | Type::Void => MachineType::I64,
        Type::Float => MachineType::F64,
        Type::Bool => MachineType::I8,
        Type::Char => MachineType::I32,
        Type::String | Type::Bytes | Type::Closure(..) => MachineType::Pointer,
        Type::Abstract(..) => return Err("abstract type needs its concrete representation".into()),
        Type::Data(name, _) if name == crate::mono::OUTCOME => MachineType::Pointer,
        Type::Data(name, _) if name == crate::mono::ARRAY => MachineType::Pointer,
        Type::Data(name, _) if checked.records.contains_key(name) => MachineType::Pointer,
        Type::Data(name, _) => {
            let mut found = false;
            let mut has_payload = false;
            for scheme in checked.constructors.values() {
                let (result, payload) = match scheme.body() {
                    Ty::Fn(fields, result, _) => (result.as_ref(), !fields.is_empty()),
                    result => (result, false),
                };
                if matches!(result, Ty::Adt(constructor_type, _) if constructor_type == name) {
                    found = true;
                    has_payload |= payload;
                }
            }
            if !found {
                return Err(format!("unknown data type {name}"));
            }
            if has_payload {
                MachineType::Pointer
            } else {
                MachineType::I64
            }
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateLayout {
    pub offsets: Vec<u32>,
    pub size: u32,
    pub alignment: u32,
}

pub fn aggregate_layout(
    fields: &[MachineType],
    pointer_bytes: u32,
    tagged: bool,
) -> Result<AggregateLayout, String> {
    if !matches!(pointer_bytes, 4 | 8) {
        return Err("unsupported pointer width".into());
    }
    let mut size: u32 = if tagged { 8 } else { 0 };
    let mut alignment = if tagged { 8 } else { 1 };
    let mut offsets = Vec::with_capacity(fields.len());
    for field in fields {
        let width = match field {
            MachineType::I8 => 1,
            MachineType::I32 => 4,
            MachineType::I64 | MachineType::F64 => 8,
            MachineType::Pointer => pointer_bytes,
        };
        alignment = alignment.max(width);
        size = align_up(size, width)?;
        offsets.push(size);
        size = size.checked_add(width).ok_or("aggregate size overflow")?;
    }
    Ok(AggregateLayout {
        offsets,
        size: align_up(size, alignment)?,
        alignment,
    })
}

fn align_up(size: u32, alignment: u32) -> Result<u32, String> {
    size.checked_add(alignment - 1)
        .map(|size| size & !(alignment - 1))
        .ok_or_else(|| "aggregate size overflow".into())
}

pub fn record_layout(
    ty: &Type,
    checked: &Checked,
    pointer_bytes: u32,
) -> Result<AggregateLayout, String> {
    let Type::Data(name, _) = ty else {
        return Err("record layout requires a nominal record type".into());
    };
    let (_, scheme) = checked.records.get(name).ok_or("unknown record type")?;
    let mut inference = crate::types::Inference::default();
    let Ty::Fn(fields, result, _) = inference.instantiate(scheme) else {
        return Err("invalid record schema".into());
    };
    inference
        .unify(&result, &Ty::from(ty))
        .map_err(|error| format!("record specialization failed: {error:?}"))?;
    let fields = fields
        .iter()
        .map(|field| {
            let ty = Type::try_from(&inference.resolve(field))
                .map_err(|_| "unresolved record field type")?;
            machine_type(&ty, checked)
        })
        .collect::<Result<Vec<_>, String>>()?;
    aggregate_layout(&fields, pointer_bytes, false)
}

pub fn constructor_layout(
    constructor: &str,
    ty: &Type,
    checked: &Checked,
    pointer_bytes: u32,
) -> Result<Option<AggregateLayout>, String> {
    if matches!(ty, Type::Data(name, _) if name == crate::mono::OUTCOME) {
        // Payloads occupy a full slot: concrete store/load types live in mono's
        // Construct/Project nodes, while every effect shares these offsets.
        let fields = match constructor {
            "$Done" => {
                let Type::Data(_, args) = ty else {
                    unreachable!()
                };
                vec![machine_type(
                    args.first().ok_or("outcome missing success type")?,
                    checked,
                )?]
            }
            _ if constructor.starts_with("$Yield#") => vec![MachineType::I64, MachineType::Pointer],
            _ if constructor.starts_with("$Fail#") => vec![MachineType::I64],
            _ => return Err("unknown outcome constructor".into()),
        };
        return Ok(Some(aggregate_layout(&fields, pointer_bytes, true)?));
    }
    let scheme = checked
        .constructors
        .get(constructor)
        .ok_or("unknown constructor")?;
    let mut inference = crate::types::Inference::default();
    let (fields, result) = match inference.instantiate(scheme) {
        Ty::Fn(fields, result, _) => (fields, *result),
        result => (vec![], result),
    };
    inference
        .unify(&result, &Ty::from(ty))
        .map_err(|error| format!("constructor specialization failed: {error:?}"))?;
    let fields = fields
        .iter()
        .map(|field| {
            let ty = Type::try_from(&inference.resolve(field))
                .map_err(|_| "unresolved constructor field type")?;
            machine_type(&ty, checked)
        })
        .collect::<Result<Vec<_>, String>>()?;
    if machine_type(ty, checked)? == MachineType::Pointer {
        Ok(Some(aggregate_layout(&fields, pointer_bytes, true)?))
    } else {
        Ok(None)
    }
}

pub fn closure_layout(
    captures: &[Type],
    checked: &Checked,
    pointer_bytes: u32,
) -> Result<AggregateLayout, String> {
    let mut fields = vec![MachineType::Pointer];
    for capture in captures {
        fields.push(machine_type(capture, checked)?);
    }
    aggregate_layout(&fields, pointer_bytes, false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    pub parameters: Vec<MachineType>,
    pub result: MachineType,
}

pub fn function_signature(
    signature: &crate::mono::Signature,
    checked: &Checked,
) -> Result<FunctionSignature, String> {
    let mut parameters = vec![MachineType::Pointer];
    for parameter in &signature.parameters {
        parameters.push(machine_type(parameter, checked)?);
    }
    Ok(FunctionSignature {
        parameters,
        result: machine_type(&signature.result, checked)?,
    })
}

pub fn constructor_tag(constructor: &str, ty: &Type, checked: &Checked) -> Result<u32, String> {
    if let Some(tag) = crate::mono::outcome_tag(constructor) {
        return Ok(tag);
    }
    let scheme = checked
        .constructors
        .get(constructor)
        .ok_or("unknown constructor")?;
    let mut inference = crate::types::Inference::default();
    let instantiated = inference.instantiate(scheme);
    let result = match &instantiated {
        Ty::Fn(_, result, _) => result.as_ref(),
        ty => ty,
    };
    inference
        .unify(result, &Ty::from(ty))
        .map_err(|error| format!("constructor type mismatch: {error:?}"))?;
    checked
        .constructor_tags
        .get(constructor)
        .copied()
        .ok_or_else(|| "missing constructor tag".into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    Little,
    Big,
}

pub fn string_literal_data(
    value: &str,
    pointer_bytes: u32,
    order: ByteOrder,
) -> Result<Vec<u8>, String> {
    let mut bytes = match pointer_bytes {
        4 => {
            let length =
                u32::try_from(value.len()).map_err(|_| "string exceeds target address space")?;
            match order {
                ByteOrder::Little => length.to_le_bytes().to_vec(),
                ByteOrder::Big => length.to_be_bytes().to_vec(),
            }
        }
        8 => {
            let length =
                u64::try_from(value.len()).map_err(|_| "string exceeds target address space")?;
            match order {
                ByteOrder::Little => length.to_le_bytes().to_vec(),
                ByteOrder::Big => length.to_be_bytes().to_vec(),
            }
        }
        _ => return Err("unsupported pointer width".into()),
    };
    bytes.extend_from_slice(value.as_bytes());
    Ok(bytes)
}
