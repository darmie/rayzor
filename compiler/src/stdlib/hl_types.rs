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
}
