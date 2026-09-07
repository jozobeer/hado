#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiType {
    F64,
    I32,
    I64,
    Size,
    Pointer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFunction {
    Alloc,
    PrintInt,
    PrintString,
    PrintErrorString,
    PrintChar,
    PrintFloat,
    IntToString,
    FloatToString,
    CharToString,
    QuoteString,
    QuoteChar,
    StringLength,
    StringConcat,
    StringCompare,
}

pub struct Declaration {
    pub symbol: &'static str,
    pub parameters: &'static [AbiType],
    pub result: Option<AbiType>,
    pub may_allocate: bool,
}

impl RuntimeFunction {
    pub fn intrinsic_name(self) -> Option<&'static str> {
        match self {
            Self::CharToString => Some("__char_to_string"),
            Self::FloatToString => Some("__float_to_string"),
            Self::IntToString => Some("__int_to_string"),
            Self::StringLength => Some("__string_length"),
            Self::StringConcat => Some("__string_concat"),
            Self::StringCompare => Some("__string_compare"),
            Self::QuoteString | Self::QuoteChar | Self::Alloc | Self::PrintInt | Self::PrintString | Self::PrintErrorString | Self::PrintChar | Self::PrintFloat => None,
        }
    }

    pub fn from_intrinsic(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|function| function.intrinsic_name() == Some(name))
    }

    pub fn value_signature(self) -> Option<crate::mono::Signature> {
        use crate::mono::{Signature, Type};
        let (parameters, result) = match self {
            Self::Alloc => return None,
            Self::CharToString | Self::QuoteChar => (vec![Type::Char], Type::String),
            Self::QuoteString => (vec![Type::String], Type::String),
            Self::FloatToString => (vec![Type::Float], Type::String),
            Self::IntToString => (vec![Type::Int], Type::String),
            Self::PrintInt => (vec![Type::Int], Type::Unit),
            Self::PrintString | Self::PrintErrorString => (vec![Type::String], Type::Unit),
            Self::PrintChar => (vec![Type::Char], Type::Unit),
            Self::PrintFloat => (vec![Type::Float], Type::Unit),
            Self::StringLength => (vec![Type::String], Type::Int),
            Self::StringConcat => (vec![Type::String, Type::String], Type::String),
            Self::StringCompare => (vec![Type::String, Type::String], Type::Int),
        };
        Some(Signature { parameters, result })
    }

    pub const ALL: &'static [Self] = &[Self::Alloc, Self::PrintInt, Self::StringLength, Self::StringConcat, Self::StringCompare, Self::PrintString, Self::PrintChar, Self::PrintFloat, Self::IntToString, Self::FloatToString, Self::CharToString, Self::PrintErrorString, Self::QuoteString, Self::QuoteChar];

    pub fn id(self) -> u32 {
        Self::ALL.iter().position(|function| *function == self).unwrap() as u32
    }

    pub fn from_id(id: u32) -> Option<Self> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn declaration(self) -> &'static Declaration {
        match self {
            Self::QuoteString => &Declaration { symbol: "shin_quote_string", parameters: &[AbiType::Pointer], result: Some(AbiType::Pointer), may_allocate: true },
            Self::QuoteChar => &Declaration { symbol: "shin_quote_char", parameters: &[AbiType::I32], result: Some(AbiType::Pointer), may_allocate: true },
            Self::CharToString => &Declaration { symbol: "shin_char_to_string", parameters: &[AbiType::I32], result: Some(AbiType::Pointer), may_allocate: true },
            Self::FloatToString => &Declaration { symbol: "shin_float_to_string", parameters: &[AbiType::F64], result: Some(AbiType::Pointer), may_allocate: true },
            Self::IntToString => &Declaration { symbol: "shin_int_to_string", parameters: &[AbiType::I64], result: Some(AbiType::Pointer), may_allocate: true },
            Self::PrintFloat => &Declaration { symbol: "shin_print_float", parameters: &[AbiType::F64], result: None, may_allocate: false },
            Self::PrintChar => &Declaration { symbol: "shin_print_char", parameters: &[AbiType::I32], result: None, may_allocate: false },
            Self::StringLength => &Declaration {
                symbol: "shin_string_length",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::I64),
                may_allocate: false,
            },
            Self::StringConcat => &Declaration {
                symbol: "shin_string_concat",
                parameters: &[AbiType::Pointer, AbiType::Pointer],
                result: Some(AbiType::Pointer),
                may_allocate: true,
            },
            Self::StringCompare => &Declaration {
                symbol: "shin_string_compare",
                parameters: &[AbiType::Pointer, AbiType::Pointer],
                result: Some(AbiType::I64),
                may_allocate: false,
            },
            Self::PrintErrorString => &Declaration { symbol: "shin_print_error_string", parameters: &[AbiType::Pointer], result: None, may_allocate: false },
            Self::PrintString => &Declaration { symbol: "shin_print_string", parameters: &[AbiType::Pointer], result: None, may_allocate: false },
            Self::Alloc => &Declaration {
                symbol: "shin_alloc",
                parameters: &[AbiType::Size],
                result: Some(AbiType::Pointer),
                may_allocate: true,
            },
            Self::PrintInt => &Declaration {
                symbol: "shin_print_int",
                parameters: &[AbiType::I64],
                result: None,
                may_allocate: false,
            },
        }
    }
}
