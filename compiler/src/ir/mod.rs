//! Intermediate Representation (IR) for the Haxe Compiler
//!
//! This module defines a low-level, platform-independent intermediate representation
//! that serves as the target for TAST lowering and the source for code generation.
//! The IR is designed to be:
//! - Simple and explicit (no implicit operations)
//! - Strongly typed with explicit type information
//! - Easy to optimize and transform
//! - Suitable for targeting multiple backends (JS, C++, JVM, etc.)

pub mod types;
pub mod instructions;
pub mod blocks;
pub mod functions;
pub mod modules;
pub mod builder;
pub mod lowering;
pub mod optimization;
pub mod validation;

pub use types::*;
pub use instructions::*;
pub use blocks::*;
pub use functions::*;
pub use modules::*;
pub use builder::*;

use std::fmt;

/// IR version for compatibility checking
pub const IR_VERSION: u32 = 1;

/// Unique identifier for IR entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrId(u32);

impl IrId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    
    pub fn invalid() -> Self {
        Self(u32::MAX)
    }
    
    pub fn is_valid(&self) -> bool {
        self.0 != u32::MAX
    }
}

impl fmt::Display for IrId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

/// Source location information for debugging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrSourceLocation {
    pub file_id: u32,
    pub line: u32,
    pub column: u32,
}

impl IrSourceLocation {
    pub fn unknown() -> Self {
        Self {
            file_id: 0,
            line: 0,
            column: 0,
        }
    }
}

/// Linkage type for symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linkage {
    /// Private to the module
    Private,
    /// Available within the package
    Internal,
    /// Publicly exported
    Public,
    /// External symbol (defined elsewhere)
    External,
}

/// Calling convention for functions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    /// Standard Haxe calling convention
    Haxe,
    /// C calling convention (for FFI)
    C,
    /// Fast calling convention (optimized)
    Fast,
    /// Platform-specific convention
    Native,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ir_id() {
        let id = IrId::new(42);
        assert_eq!(format!("{}", id), "$42");
        assert!(id.is_valid());
        
        let invalid = IrId::invalid();
        assert!(!invalid.is_valid());
    }
}