//! Hashlink type system compatibility
//!
//! This module provides type definitions compatible with Hashlink's type system,
//! enabling loading and calling functions from HDLL (Hashlink Dynamic Library) files.

use super::IrTypeDescriptor;

/// Hashlink type codes (from hl.h in the Hashlink VM)
///
/// These represent the fundamental types in Hashlink's type system.
/// When loading HDLL files, we map these to our `IrTypeDescriptor` for
/// consistent type handling across the compiler.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HlTypeKind {
    /// Void type - no value
    HVoid = 0,
    /// Unsigned 8-bit integer
    HUI8 = 1,
    /// Unsigned 16-bit integer
    HUI16 = 2,
    /// Signed 32-bit integer (Haxe Int)
    HI32 = 3,
    /// Signed 64-bit integer
    HI64 = 4,
    /// 32-bit floating point
    HF32 = 5,
    /// 64-bit floating point (Haxe Float)
    HF64 = 6,
    /// Boolean
    HBool = 7,
    /// Byte pointer/buffer
    HBytes = 8,
    /// Dynamic type (runtime typed)
    HDyn = 9,
    /// Function type
    HFun = 10,
    /// Object/class instance
    HObj = 11,
    /// Array type
    HArray = 12,
    /// Type reference
    HType = 13,
    /// Reference type
    HRef = 14,
    /// Virtual type (interface-like)
    HVirtual = 15,
    /// Dynamic object
    HDynObj = 16,
    /// Abstract type (opaque handle)
    HAbstract = 17,
    /// Enum type
    HEnum = 18,
    /// Null type
    HNull = 19,
    /// Method type
    HMethod = 20,
    /// Struct type (value type)
    HStruct = 21,
}

impl HlTypeKind {
    /// Convert a Hashlink type to our IR type descriptor.
    ///
    /// This mapping allows HDLL functions to be integrated into the
    /// rayzor type system for proper code generation.
    pub fn to_ir_type_descriptor(&self) -> IrTypeDescriptor {
        match self {
            // Direct scalar mappings
            HlTypeKind::HVoid => IrTypeDescriptor::Void,
            HlTypeKind::HUI8 => IrTypeDescriptor::U8,
            HlTypeKind::HUI16 => IrTypeDescriptor::I32, // Promote to i32
            HlTypeKind::HI32 => IrTypeDescriptor::I32,
            HlTypeKind::HI64 => IrTypeDescriptor::I64,
            HlTypeKind::HF32 => IrTypeDescriptor::F32,
            HlTypeKind::HF64 => IrTypeDescriptor::F64,
            HlTypeKind::HBool => IrTypeDescriptor::Bool,

            // Pointer types
            HlTypeKind::HBytes => IrTypeDescriptor::PtrU8,

            // All other types are treated as opaque pointers
            // This includes HDyn, HObj, HArray, etc.
            HlTypeKind::HDyn
            | HlTypeKind::HFun
            | HlTypeKind::HObj
            | HlTypeKind::HArray
            | HlTypeKind::HType
            | HlTypeKind::HRef
            | HlTypeKind::HVirtual
            | HlTypeKind::HDynObj
            | HlTypeKind::HAbstract
            | HlTypeKind::HEnum
            | HlTypeKind::HNull
            | HlTypeKind::HMethod
            | HlTypeKind::HStruct => IrTypeDescriptor::PtrVoid,
        }
    }

    /// Parse a type from a manifest string representation.
    ///
    /// This is used when reading HDLL manifest files that describe
    /// function signatures in a human-readable format.
    pub fn from_manifest_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "void" => Some(HlTypeKind::HVoid),
            "u8" | "ui8" | "byte" => Some(HlTypeKind::HUI8),
            "u16" | "ui16" => Some(HlTypeKind::HUI16),
            "i32" | "int" => Some(HlTypeKind::HI32),
            "i64" => Some(HlTypeKind::HI64),
            "f32" => Some(HlTypeKind::HF32),
            "f64" | "float" => Some(HlTypeKind::HF64),
            "bool" => Some(HlTypeKind::HBool),
            "bytes" | "string" => Some(HlTypeKind::HBytes),
            "dyn" | "dynamic" => Some(HlTypeKind::HDyn),
            "fun" | "function" => Some(HlTypeKind::HFun),
            "obj" | "object" => Some(HlTypeKind::HObj),
            "array" => Some(HlTypeKind::HArray),
            "type" => Some(HlTypeKind::HType),
            "ref" => Some(HlTypeKind::HRef),
            "virtual" => Some(HlTypeKind::HVirtual),
            "dynobj" => Some(HlTypeKind::HDynObj),
            "abstract" | "handle" => Some(HlTypeKind::HAbstract),
            "enum" => Some(HlTypeKind::HEnum),
            "null" => Some(HlTypeKind::HNull),
            "method" => Some(HlTypeKind::HMethod),
            "struct" => Some(HlTypeKind::HStruct),
            _ => None,
        }
    }

    /// Parse a Hashlink type from a single-character type code.
    ///
    /// These codes are used in HL function signatures returned by `hlp_` symbols.
    /// The codes are defined in `hl.h` and match the `DEFINE_PRIM` macro type suffixes.
    ///
    /// # Type Code Reference
    ///
    /// | Code | Type      | Code | Type     |
    /// |------|-----------|------|----------|
    /// | `v`  | Void      | `O`  | Object   |
    /// | `c`  | UI8       | `A`  | Array    |
    /// | `s`  | UI16      | `T`  | Type     |
    /// | `i`  | I32       | `R`  | Ref      |
    /// | `l`  | I64       | `V`  | Virtual  |
    /// | `f`  | F32       | `W`  | DynObj   |
    /// | `d`  | F64       | `X`  | Abstract |
    /// | `b`  | Bool      | `E`  | Enum     |
    /// | `B`  | Bytes     | `N`  | Null     |
    /// | `D`  | Dynamic   | `M`  | Method   |
    /// | `F`  | Function  | `S`  | Struct   |
    pub fn from_type_code(c: char) -> Option<Self> {
        match c {
            'v' => Some(HlTypeKind::HVoid),
            'c' => Some(HlTypeKind::HUI8),
            's' => Some(HlTypeKind::HUI16),
            'i' => Some(HlTypeKind::HI32),
            'l' => Some(HlTypeKind::HI64),
            'f' => Some(HlTypeKind::HF32),
            'd' => Some(HlTypeKind::HF64),
            'b' => Some(HlTypeKind::HBool),
            'B' => Some(HlTypeKind::HBytes),
            'D' => Some(HlTypeKind::HDyn),
            'F' => Some(HlTypeKind::HFun),
            'O' => Some(HlTypeKind::HObj),
            'A' => Some(HlTypeKind::HArray),
            'T' => Some(HlTypeKind::HType),
            'R' => Some(HlTypeKind::HRef),
            'V' => Some(HlTypeKind::HVirtual),
            'W' => Some(HlTypeKind::HDynObj),
            'X' => Some(HlTypeKind::HAbstract),
            'E' => Some(HlTypeKind::HEnum),
            'N' => Some(HlTypeKind::HNull),
            'M' => Some(HlTypeKind::HMethod),
            'S' => Some(HlTypeKind::HStruct),
            _ => None,
        }
    }

    /// Get a string representation suitable for manifests
    pub fn to_manifest_str(&self) -> &'static str {
        match self {
            HlTypeKind::HVoid => "void",
            HlTypeKind::HUI8 => "u8",
            HlTypeKind::HUI16 => "u16",
            HlTypeKind::HI32 => "i32",
            HlTypeKind::HI64 => "i64",
            HlTypeKind::HF32 => "f32",
            HlTypeKind::HF64 => "f64",
            HlTypeKind::HBool => "bool",
            HlTypeKind::HBytes => "bytes",
            HlTypeKind::HDyn => "dyn",
            HlTypeKind::HFun => "fun",
            HlTypeKind::HObj => "obj",
            HlTypeKind::HArray => "array",
            HlTypeKind::HType => "type",
            HlTypeKind::HRef => "ref",
            HlTypeKind::HVirtual => "virtual",
            HlTypeKind::HDynObj => "dynobj",
            HlTypeKind::HAbstract => "abstract",
            HlTypeKind::HEnum => "enum",
            HlTypeKind::HNull => "null",
            HlTypeKind::HMethod => "method",
            HlTypeKind::HStruct => "struct",
        }
    }
}

/// Parse a Hashlink function signature string.
///
/// Hashlink HDLL libraries export `hlp_<name>` symbols that return a type
/// signature string describing the function's parameter and return types.
///
/// # Format
///
/// The signature format is: `arg_type_codes + "_" + return_type_code`
///
/// For example:
/// - `"ii_i"` = `fn(i32, i32) -> i32`
/// - `"d_v"` = `fn(f64) -> void`
/// - `"_v"` = `fn() -> void` (no parameters)
/// - `"BiB_B"` = `fn(bytes, i32, bytes) -> bytes`
///
/// # Returns
///
/// Returns `Some((param_types, return_type))` on success, or `None` if the
/// signature is malformed or contains unknown type codes.
pub fn parse_hl_signature(sig: &str) -> Option<(Vec<HlTypeKind>, HlTypeKind)> {
    let (params_str, return_str) = sig.rsplit_once('_')?;

    // Parse return type (single character)
    if return_str.len() != 1 {
        return None;
    }
    let return_type = HlTypeKind::from_type_code(return_str.chars().next()?)?;

    // Parse parameter types (each is a single character)
    let mut param_types = Vec::new();
    for c in params_str.chars() {
        param_types.push(HlTypeKind::from_type_code(c)?);
    }

    Some((param_types, return_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_type_mapping() {
        assert_eq!(HlTypeKind::HVoid.to_ir_type_descriptor(), IrTypeDescriptor::Void);
        assert_eq!(HlTypeKind::HI32.to_ir_type_descriptor(), IrTypeDescriptor::I32);
        assert_eq!(HlTypeKind::HI64.to_ir_type_descriptor(), IrTypeDescriptor::I64);
        assert_eq!(HlTypeKind::HF32.to_ir_type_descriptor(), IrTypeDescriptor::F32);
        assert_eq!(HlTypeKind::HF64.to_ir_type_descriptor(), IrTypeDescriptor::F64);
        assert_eq!(HlTypeKind::HBool.to_ir_type_descriptor(), IrTypeDescriptor::Bool);
        assert_eq!(HlTypeKind::HUI8.to_ir_type_descriptor(), IrTypeDescriptor::U8);
    }

    #[test]
    fn test_pointer_type_mapping() {
        assert_eq!(HlTypeKind::HBytes.to_ir_type_descriptor(), IrTypeDescriptor::PtrU8);
        assert_eq!(HlTypeKind::HDyn.to_ir_type_descriptor(), IrTypeDescriptor::PtrVoid);
        assert_eq!(HlTypeKind::HObj.to_ir_type_descriptor(), IrTypeDescriptor::PtrVoid);
        assert_eq!(HlTypeKind::HArray.to_ir_type_descriptor(), IrTypeDescriptor::PtrVoid);
    }

    #[test]
    fn test_manifest_parsing() {
        assert_eq!(HlTypeKind::from_manifest_str("i32"), Some(HlTypeKind::HI32));
        assert_eq!(HlTypeKind::from_manifest_str("int"), Some(HlTypeKind::HI32));
        assert_eq!(HlTypeKind::from_manifest_str("f64"), Some(HlTypeKind::HF64));
        assert_eq!(HlTypeKind::from_manifest_str("float"), Some(HlTypeKind::HF64));
        assert_eq!(HlTypeKind::from_manifest_str("bool"), Some(HlTypeKind::HBool));
        assert_eq!(HlTypeKind::from_manifest_str("void"), Some(HlTypeKind::HVoid));
        assert_eq!(HlTypeKind::from_manifest_str("dyn"), Some(HlTypeKind::HDyn));
        assert_eq!(HlTypeKind::from_manifest_str("dynamic"), Some(HlTypeKind::HDyn));
        assert_eq!(HlTypeKind::from_manifest_str("bytes"), Some(HlTypeKind::HBytes));
    }

    #[test]
    fn test_manifest_case_insensitive() {
        assert_eq!(HlTypeKind::from_manifest_str("I32"), Some(HlTypeKind::HI32));
        assert_eq!(HlTypeKind::from_manifest_str("BOOL"), Some(HlTypeKind::HBool));
        assert_eq!(HlTypeKind::from_manifest_str("Void"), Some(HlTypeKind::HVoid));
    }

    #[test]
    fn test_manifest_roundtrip() {
        let types = [
            HlTypeKind::HVoid,
            HlTypeKind::HI32,
            HlTypeKind::HI64,
            HlTypeKind::HF64,
            HlTypeKind::HBool,
            HlTypeKind::HDyn,
        ];

        for ty in types {
            let manifest_str = ty.to_manifest_str();
            let parsed = HlTypeKind::from_manifest_str(manifest_str);
            assert_eq!(parsed, Some(ty), "Roundtrip failed for {:?}", ty);
        }
    }

    #[test]
    fn test_unknown_type_returns_none() {
        assert_eq!(HlTypeKind::from_manifest_str("unknown_type"), None);
        assert_eq!(HlTypeKind::from_manifest_str(""), None);
        assert_eq!(HlTypeKind::from_manifest_str("foobar"), None);
    }

    #[test]
    fn test_type_code_scalars() {
        assert_eq!(HlTypeKind::from_type_code('v'), Some(HlTypeKind::HVoid));
        assert_eq!(HlTypeKind::from_type_code('i'), Some(HlTypeKind::HI32));
        assert_eq!(HlTypeKind::from_type_code('l'), Some(HlTypeKind::HI64));
        assert_eq!(HlTypeKind::from_type_code('f'), Some(HlTypeKind::HF32));
        assert_eq!(HlTypeKind::from_type_code('d'), Some(HlTypeKind::HF64));
        assert_eq!(HlTypeKind::from_type_code('b'), Some(HlTypeKind::HBool));
        assert_eq!(HlTypeKind::from_type_code('c'), Some(HlTypeKind::HUI8));
        assert_eq!(HlTypeKind::from_type_code('s'), Some(HlTypeKind::HUI16));
    }

    #[test]
    fn test_type_code_pointers() {
        assert_eq!(HlTypeKind::from_type_code('B'), Some(HlTypeKind::HBytes));
        assert_eq!(HlTypeKind::from_type_code('D'), Some(HlTypeKind::HDyn));
        assert_eq!(HlTypeKind::from_type_code('O'), Some(HlTypeKind::HObj));
        assert_eq!(HlTypeKind::from_type_code('A'), Some(HlTypeKind::HArray));
        assert_eq!(HlTypeKind::from_type_code('X'), Some(HlTypeKind::HAbstract));
    }

    #[test]
    fn test_type_code_unknown() {
        assert_eq!(HlTypeKind::from_type_code('z'), None);
        assert_eq!(HlTypeKind::from_type_code('0'), None);
        assert_eq!(HlTypeKind::from_type_code(' '), None);
    }

    #[test]
    fn test_parse_hl_signature_basic() {
        // fn(i32, i32) -> i32
        let (params, ret) = parse_hl_signature("ii_i").unwrap();
        assert_eq!(params, vec![HlTypeKind::HI32, HlTypeKind::HI32]);
        assert_eq!(ret, HlTypeKind::HI32);
    }

    #[test]
    fn test_parse_hl_signature_no_params() {
        // fn() -> void
        let (params, ret) = parse_hl_signature("_v").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, HlTypeKind::HVoid);
    }

    #[test]
    fn test_parse_hl_signature_mixed_types() {
        // fn(bytes, i32, bytes) -> bytes
        let (params, ret) = parse_hl_signature("BiB_B").unwrap();
        assert_eq!(params, vec![HlTypeKind::HBytes, HlTypeKind::HI32, HlTypeKind::HBytes]);
        assert_eq!(ret, HlTypeKind::HBytes);
    }

    #[test]
    fn test_parse_hl_signature_float_to_void() {
        // fn(f64) -> void
        let (params, ret) = parse_hl_signature("d_v").unwrap();
        assert_eq!(params, vec![HlTypeKind::HF64]);
        assert_eq!(ret, HlTypeKind::HVoid);
    }

    #[test]
    fn test_parse_hl_signature_abstract() {
        // fn(abstract, i32) -> abstract
        let (params, ret) = parse_hl_signature("Xi_X").unwrap();
        assert_eq!(params, vec![HlTypeKind::HAbstract, HlTypeKind::HI32]);
        assert_eq!(ret, HlTypeKind::HAbstract);
    }

    #[test]
    fn test_parse_hl_signature_invalid() {
        // No underscore separator
        assert!(parse_hl_signature("ii").is_none());
        // Unknown type code
        assert!(parse_hl_signature("iz_i").is_none());
        // Multiple return type chars
        assert!(parse_hl_signature("i_ii").is_none());
        // Empty string
        assert!(parse_hl_signature("").is_none());
    }
}
