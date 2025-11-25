//! Standard Library Implementation using MIR Builder
//!
//! This module provides the Haxe standard library built directly in MIR
//! without parsing source files. This approach provides:
//!
//! - **Fast compilation**: No parsing overhead
//! - **Type-safe**: Built using MIR builder API
//! - **Extern support**: Native runtime functions
//! - **Version control**: Stdlib is Rust code
//! - **Easy maintenance**: No complex Haxe parsing needed
//!
//! # Architecture
//!
//! The stdlib is organized into modules:
//! - `string` - String type with UTF-8 support and extern methods
//! - `array` - Array<T> dynamic array with extern methods
//! - `stdtypes` - Int, Float, Bool with extern methods
//! - `math` - Mathematical functions
//! - `sys` - System interactions
//!
//! # Extern Functions
//!
//! Extern functions are declared in MIR but implemented in the runtime.
//! The runtime provides native implementations for:
//! - String operations (concat, substring, indexOf)
//! - Array operations (push, pop, slice, sort)
//! - Type conversions (toString, parseInt)
//! - I/O operations (print, trace)

pub mod string;
pub mod array;
pub mod stdtypes;
pub mod memory;
pub mod vec_u8;
pub mod runtime_mapping;
pub mod vec;

// Rayzor concurrent primitives
pub mod thread;
pub mod channel;
pub mod sync;

use crate::ir::{IrModule, mir_builder::MirBuilder};

// Re-export runtime mapping types
pub use runtime_mapping::{StdlibMapping, MethodSignature, RuntimeFunctionCall};

/// Build the complete standard library as an MIR module
///
/// This creates all standard library types and functions using the MIR builder.
/// The stdlib includes:
/// - Memory management (malloc, realloc, free)
/// - String with extern methods
/// - Array<T> with extern methods
/// - Standard types (Int, Float, Bool)
/// - Built-in functions (trace, print)
///
/// # Returns
///
/// An IrModule containing the complete standard library
pub fn build_stdlib() -> IrModule {
    let mut builder = MirBuilder::new("haxe");

    // Memory management functions
    memory::build_memory_functions(&mut builder);

    // Build Vec<u8> type and methods
    vec_u8::build_vec_u8_type(&mut builder);

    // Build String type and methods
    string::build_string_type(&mut builder);

    // Build Array<T> type and methods
    array::build_array_type(&mut builder);

    // Build standard types and conversions
    stdtypes::build_std_types(&mut builder);

    // Build concurrent primitives
    thread::build_thread_type(&mut builder);
    channel::build_channel_type(&mut builder);
    sync::build_sync_types(&mut builder);

    // Build Vec<T> extern declarations (monomorphized specializations)
    vec::build_vec_externs(&mut builder);

    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdlib_builds() {
        let stdlib = build_stdlib();

        // Should have functions
        assert!(!stdlib.functions.is_empty(), "Stdlib should have functions");

        // Module should be named "haxe"
        assert_eq!(stdlib.name, "haxe");
    }

    #[test]
    fn test_stdlib_has_string_functions() {
        let stdlib = build_stdlib();

        // Should have string functions
        let has_string_concat = stdlib.functions.iter()
            .any(|(_, f)| f.name.contains("string"));

        assert!(has_string_concat, "Should have string functions");
    }
}
