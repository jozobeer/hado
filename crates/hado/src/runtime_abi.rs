#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiType {
    F64,
    I8,
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
    AbortError,
    PrintChar,
    PrintFloat,
    IntToString,
    IntParseIsValid,
    IntParseUnchecked,
    FloatToString,
    CharToString,
    QuoteString,
    QuoteChar,
    CanonicalFloat,
    StringLength,
    StringConcat,
    StringCompare,
    StringSlice,
    StringGet,
    StringFind,
    BytesLength,
    BytesGet,
    BytesIsUtf8,
    BytesDisplay,
    BytesOfString,
    BytesToStringUnchecked,
    BytesAlloc,
    BytesFillUnchecked,
    ArrayLength,
    ArrayGet,
    ArrayAlloc,
    ArrayFillUnchecked,
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
            Self::ArrayLength => Some("__array_length"),
            Self::ArrayGet => Some("__array_get"),
            Self::ArrayAlloc => Some("__array_alloc"),
            Self::ArrayFillUnchecked => Some("__array_fill_unchecked"),
            Self::CharToString => Some("__char_to_string"),
            Self::FloatToString => Some("__float_to_string"),
            Self::IntToString => Some("__int_to_string"),
            Self::IntParseIsValid => Some("__int_parse_is_valid"),
            Self::IntParseUnchecked => Some("__int_parse_unchecked"),
            Self::StringLength => Some("__string_length"),
            Self::StringConcat => Some("__string_concat"),
            Self::StringCompare => Some("__string_compare"),
            Self::StringSlice => Some("__string_slice"),
            Self::StringGet => Some("__string_get"),
            Self::StringFind => Some("__string_find"),
            Self::BytesLength => Some("__bytes_length"),
            Self::BytesGet => Some("__bytes_get"),
            Self::BytesIsUtf8 => Some("__bytes_is_utf8"),
            Self::BytesDisplay => Some("__bytes_display"),
            Self::BytesOfString => Some("__bytes_of_string"),
            Self::BytesToStringUnchecked => Some("__bytes_to_string_unchecked"),
            Self::BytesAlloc => Some("__bytes_alloc"),
            Self::BytesFillUnchecked => Some("__bytes_fill_unchecked"),
            Self::CanonicalFloat | Self::QuoteString | Self::QuoteChar | Self::Alloc | Self::PrintInt | Self::PrintString | Self::PrintErrorString | Self::AbortError | Self::PrintChar | Self::PrintFloat => None,
        }
    }

    pub fn from_intrinsic(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|function| function.intrinsic_name() == Some(name))
    }

    pub fn is_polymorphic(self) -> bool {
        matches!(self, Self::ArrayLength | Self::ArrayGet | Self::ArrayAlloc | Self::ArrayFillUnchecked)
    }

    pub fn value_signature(self) -> Option<crate::mono::Signature> {
        use crate::mono::{Signature, Type};
        let (parameters, result) = match self {
            Self::Alloc | Self::ArrayLength | Self::ArrayGet | Self::ArrayAlloc | Self::ArrayFillUnchecked => return None,
            Self::CharToString | Self::QuoteChar => (vec![Type::Char], Type::String),
            Self::QuoteString => (vec![Type::String], Type::String),
            Self::FloatToString | Self::CanonicalFloat => (vec![Type::Float], Type::String),
            Self::IntToString => (vec![Type::Int], Type::String),
            Self::IntParseIsValid => (vec![Type::String], Type::Bool),
            Self::IntParseUnchecked => (vec![Type::String], Type::Int),
            Self::PrintInt => (vec![Type::Int], Type::Unit),
            Self::PrintString | Self::PrintErrorString | Self::AbortError => (vec![Type::String], Type::Unit),
            Self::PrintChar => (vec![Type::Char], Type::Unit),
            Self::PrintFloat => (vec![Type::Float], Type::Unit),
            Self::StringLength => (vec![Type::String], Type::Int),
            Self::StringConcat => (vec![Type::String, Type::String], Type::String),
            Self::StringCompare => (vec![Type::String, Type::String], Type::Int),
            Self::StringSlice => (vec![Type::String, Type::Int, Type::Int], Type::String),
            Self::StringGet => (vec![Type::String, Type::Int], Type::Char),
            Self::StringFind => (vec![Type::String, Type::String, Type::Int], Type::Int),
            Self::BytesLength => (vec![Type::Bytes], Type::Int),
            Self::BytesGet => (vec![Type::Bytes, Type::Int], Type::Int),
            Self::BytesIsUtf8 => (vec![Type::Bytes], Type::Bool),
            Self::BytesDisplay => (vec![Type::Bytes], Type::String),
            Self::BytesOfString => (vec![Type::String], Type::Bytes),
            Self::BytesToStringUnchecked => (vec![Type::Bytes], Type::String),
            Self::BytesAlloc => (vec![Type::Int], Type::Bytes),
            Self::BytesFillUnchecked => (vec![Type::Bytes, Type::Int, Type::Int], Type::Bytes),
        };
        Some(Signature { parameters, result })
    }

    pub const ALL: &'static [Self] = &[Self::Alloc, Self::PrintInt, Self::StringLength, Self::StringConcat, Self::StringCompare, Self::PrintString, Self::PrintChar, Self::PrintFloat, Self::IntToString, Self::IntParseIsValid, Self::IntParseUnchecked, Self::FloatToString, Self::CharToString, Self::PrintErrorString, Self::AbortError, Self::QuoteString, Self::QuoteChar, Self::CanonicalFloat, Self::BytesLength, Self::BytesGet, Self::BytesIsUtf8, Self::BytesDisplay, Self::BytesOfString, Self::BytesToStringUnchecked, Self::BytesAlloc, Self::BytesFillUnchecked, Self::StringSlice, Self::StringGet, Self::StringFind, Self::ArrayLength, Self::ArrayGet, Self::ArrayAlloc, Self::ArrayFillUnchecked];

    pub fn id(self) -> u32 {
        Self::ALL.iter().position(|function| *function == self).unwrap() as u32
    }

    pub fn from_id(id: u32) -> Option<Self> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn declaration(self) -> &'static Declaration {
        match self {
            Self::ArrayGet | Self::ArrayFillUnchecked => unreachable!("array slot operations are emitted inline"),
            Self::ArrayLength => &Declaration {
                symbol: "hado_array_length",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::I64),
                may_allocate: false,
            },
            Self::ArrayAlloc => &Declaration {
                symbol: "hado_array_alloc",
                parameters: &[AbiType::I64],
                result: Some(AbiType::Pointer),
                may_allocate: true,
            },
            Self::CanonicalFloat => &Declaration { symbol: "hado_canonical_float", parameters: &[AbiType::F64], result: Some(AbiType::Pointer), may_allocate: true },
            Self::QuoteString => &Declaration { symbol: "hado_quote_string", parameters: &[AbiType::Pointer], result: Some(AbiType::Pointer), may_allocate: true },
            Self::QuoteChar => &Declaration { symbol: "hado_quote_char", parameters: &[AbiType::I32], result: Some(AbiType::Pointer), may_allocate: true },
            Self::CharToString => &Declaration { symbol: "hado_char_to_string", parameters: &[AbiType::I32], result: Some(AbiType::Pointer), may_allocate: true },
            Self::FloatToString => &Declaration { symbol: "hado_float_to_string", parameters: &[AbiType::F64], result: Some(AbiType::Pointer), may_allocate: true },
            Self::IntToString => &Declaration { symbol: "hado_int_to_string", parameters: &[AbiType::I64], result: Some(AbiType::Pointer), may_allocate: true },
            Self::IntParseIsValid => &Declaration {
                symbol: "hado_int_parse_is_valid",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::I8),
                may_allocate: false,
            },
            Self::IntParseUnchecked => &Declaration {
                symbol: "hado_int_parse_unchecked",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::I64),
                may_allocate: false,
            },
            Self::PrintFloat => &Declaration { symbol: "hado_print_float", parameters: &[AbiType::F64], result: None, may_allocate: false },
            Self::PrintChar => &Declaration { symbol: "hado_print_char", parameters: &[AbiType::I32], result: None, may_allocate: false },
            Self::StringLength => &Declaration {
                symbol: "hado_string_length",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::I64),
                may_allocate: false,
            },
            Self::StringConcat => &Declaration {
                symbol: "hado_string_concat",
                parameters: &[AbiType::Pointer, AbiType::Pointer],
                result: Some(AbiType::Pointer),
                may_allocate: true,
            },
            Self::StringSlice => &Declaration {
                symbol: "hado_string_slice",
                parameters: &[AbiType::Pointer, AbiType::I64, AbiType::I64],
                result: Some(AbiType::Pointer),
                may_allocate: true,
            },
            Self::StringGet => &Declaration {
                symbol: "hado_string_get",
                parameters: &[AbiType::Pointer, AbiType::I64],
                result: Some(AbiType::I32),
                may_allocate: false,
            },
            Self::StringFind => &Declaration {
                symbol: "hado_string_find",
                parameters: &[AbiType::Pointer, AbiType::Pointer, AbiType::I64],
                result: Some(AbiType::I64),
                may_allocate: false,
            },
            Self::StringCompare => &Declaration {
                symbol: "hado_string_compare",
                parameters: &[AbiType::Pointer, AbiType::Pointer],
                result: Some(AbiType::I64),
                may_allocate: false,
            },
            Self::PrintErrorString => &Declaration { symbol: "hado_print_error_string", parameters: &[AbiType::Pointer], result: None, may_allocate: false },
            Self::AbortError => &Declaration { symbol: "hado_abort_error", parameters: &[AbiType::Pointer], result: None, may_allocate: false },
            Self::PrintString => &Declaration { symbol: "hado_print_string", parameters: &[AbiType::Pointer], result: None, may_allocate: false },
            Self::Alloc => &Declaration {
                symbol: "hado_alloc",
                parameters: &[AbiType::Size],
                result: Some(AbiType::Pointer),
                may_allocate: true,
            },
            Self::PrintInt => &Declaration {
                symbol: "hado_print_int",
                parameters: &[AbiType::I64],
                result: None,
                may_allocate: false,
            },
            Self::BytesLength => &Declaration {
                symbol: "hado_bytes_length",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::I64),
                may_allocate: false,
            },
            Self::BytesGet => &Declaration {
                symbol: "hado_bytes_get",
                parameters: &[AbiType::Pointer, AbiType::I64],
                result: Some(AbiType::I64),
                may_allocate: false,
            },
            Self::BytesIsUtf8 => &Declaration {
                symbol: "hado_bytes_is_utf8",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::I8),
                may_allocate: false,
            },
            Self::BytesDisplay => &Declaration {
                symbol: "hado_bytes_display",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::Pointer),
                may_allocate: true,
            },
            Self::BytesOfString => &Declaration {
                symbol: "hado_bytes_of_string",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::Pointer),
                may_allocate: false,
            },
            Self::BytesToStringUnchecked => &Declaration {
                symbol: "hado_bytes_to_string_unchecked",
                parameters: &[AbiType::Pointer],
                result: Some(AbiType::Pointer),
                may_allocate: false,
            },
            Self::BytesAlloc => &Declaration {
                symbol: "hado_bytes_alloc",
                parameters: &[AbiType::I64],
                result: Some(AbiType::Pointer),
                may_allocate: true,
            },
            Self::BytesFillUnchecked => &Declaration {
                symbol: "hado_bytes_fill_unchecked",
                parameters: &[AbiType::Pointer, AbiType::I64, AbiType::I64],
                result: Some(AbiType::Pointer),
                may_allocate: false,
            },
        }
    }
}
