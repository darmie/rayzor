//! BLADE Format - Blazing Language Artifact Deployment Environment
//!
//! This module provides serialization and deserialization of MIR (Mid-level IR)
//! to the `.blade` binary format using the `postcard` crate for efficient,
//! compact binary serialization.
//!
//! # BLADE Format Benefits
//!
//! - **Incremental Compilation**: Avoid recompiling unchanged modules (30x faster)
//! - **Module Caching**: Fast startup by loading pre-compiled modules
//! - **Build Artifacts**: Distribute pre-compiled libraries
//! - **Compact**: Uses `postcard` for minimal size
//! - **Fast**: Efficient binary deserialization
//!
//! # File Extension
//!
//! - **`.blade`** - Compiled Rayzor module (binary format)
//!
//! # Usage
//!
//! ```rust,ignore
//! use compiler::ir::blade::{save_blade, load_blade, BladeMetadata};
//!
//! // Serialize MIR to .blade file
//! let metadata = BladeMetadata {
//!     name: "MyModule".to_string(),
//!     source_path: "src/Main.hx".to_string(),
//!     source_timestamp: 1234567890,
//!     compile_timestamp: 1234567900,
//!     dependencies: vec![],
//!     compiler_version: env!("CARGO_PKG_VERSION").to_string(),
//! };
//! save_blade("output.blade", &mir_module, metadata)?;
//!
//! // Deserialize .blade file to MIR
//! let (mir_module, metadata) = load_blade("output.blade")?;
//! ```

use serde::{Serialize, Deserialize};
use std::path::Path;
use std::fs;
use crate::ir::IrModule;

/// BLADE file magic number (first 4 bytes)
const BLADE_MAGIC: &[u8; 4] = b"BLAD";

/// Current BLADE format version
const BLADE_VERSION: u32 = 1;

/// Metadata about the compiled module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BladeMetadata {
    /// Module name
    pub name: String,

    /// Source file path
    pub source_path: String,

    /// Source file modification timestamp (Unix epoch seconds)
    pub source_timestamp: u64,

    /// Compilation timestamp (Unix epoch seconds)
    pub compile_timestamp: u64,

    /// List of module dependencies
    pub dependencies: Vec<String>,

    /// Compiler version that created this BLADE file
    pub compiler_version: String,
}

/// A complete BLADE module ready for serialization
#[derive(Debug, Serialize, Deserialize)]
struct BladeModule {
    /// Magic number for validation
    magic: [u8; 4],

    /// Format version
    version: u32,

    /// Module metadata
    metadata: BladeMetadata,

    /// The actual MIR module (directly serialized)
    mir: IrModule,
}

/// Errors that can occur during BLADE operations
#[derive(Debug)]
pub enum BladeError {
    /// I/O error
    Io(std::io::Error),

    /// Serialization error
    Serialization(postcard::Error),

    /// Invalid magic number
    InvalidMagic,

    /// Unsupported version
    UnsupportedVersion(u32),
}

impl std::fmt::Display for BladeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BladeError::Io(e) => write!(f, "I/O error: {}", e),
            BladeError::Serialization(e) => write!(f, "Serialization error: {}", e),
            BladeError::InvalidMagic => write!(f, "Invalid BLADE magic number"),
            BladeError::UnsupportedVersion(v) => write!(f, "Unsupported BLADE version: {}", v),
        }
    }
}

impl std::error::Error for BladeError {}

impl From<std::io::Error> for BladeError {
    fn from(e: std::io::Error) -> Self {
        BladeError::Io(e)
    }
}

impl From<postcard::Error> for BladeError {
    fn from(e: postcard::Error) -> Self {
        BladeError::Serialization(e)
    }
}

/// Save a MIR module to a .blade file
///
/// # Arguments
///
/// * `path` - Path to the .blade file to create
/// * `module` - The MIR module to serialize
/// * `metadata` - Metadata about the module
///
/// # Example
///
/// ```rust,ignore
/// let metadata = BladeMetadata {
///     name: "Main".to_string(),
///     source_path: "Main.hx".to_string(),
///     source_timestamp: 1234567890,
///     compile_timestamp: 1234567900,
///     dependencies: vec![],
///     compiler_version: env!("CARGO_PKG_VERSION").to_string(),
/// };
/// save_blade("Main.blade", &mir_module, metadata)?;
/// ```
pub fn save_blade(
    path: impl AsRef<Path>,
    module: &IrModule,
    metadata: BladeMetadata,
) -> Result<(), BladeError> {
    let blade = BladeModule {
        magic: *BLADE_MAGIC,
        version: BLADE_VERSION,
        metadata,
        mir: module.clone(),
    };

    // Serialize using postcard
    let bytes = postcard::to_allocvec(&blade)?;

    // Write to file
    fs::write(path, bytes)?;

    Ok(())
}

/// Load a MIR module from a .blade file
///
/// # Arguments
///
/// * `path` - Path to the .blade file to load
///
/// # Returns
///
/// A tuple of (IrModule, BladeMetadata)
///
/// # Example
///
/// ```rust,ignore
/// let (mir_module, metadata) = load_blade("Main.blade")?;
/// println!("Loaded module: {}", metadata.name);
/// ```
pub fn load_blade(path: impl AsRef<Path>) -> Result<(IrModule, BladeMetadata), BladeError> {
    // Read file
    let bytes = fs::read(path)?;

    // Deserialize using postcard
    let blade: BladeModule = postcard::from_bytes(&bytes)?;

    // Validate magic number
    if &blade.magic != BLADE_MAGIC {
        return Err(BladeError::InvalidMagic);
    }

    // Check version
    if blade.version != BLADE_VERSION {
        return Err(BladeError::UnsupportedVersion(blade.version));
    }

    Ok((blade.mir, blade.metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::modules::IrModule;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_blade_roundtrip() {
        // Create a simple IR module
        let module = IrModule::new("test_module".to_string(), "test.hx".to_string());

        // Create metadata
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let metadata = BladeMetadata {
            name: "test_module".to_string(),
            source_path: "test.hx".to_string(),
            source_timestamp: now,
            compile_timestamp: now,
            dependencies: vec![],
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Serialize to bytes
        let blade = BladeModule {
            magic: *BLADE_MAGIC,
            version: BLADE_VERSION,
            metadata: metadata.clone(),
            mir: module.clone(),
        };

        let bytes = postcard::to_allocvec(&blade).unwrap();

        // Deserialize
        let decoded: BladeModule = postcard::from_bytes(&bytes).unwrap();

        assert_eq!(&decoded.magic, BLADE_MAGIC);
        assert_eq!(decoded.version, BLADE_VERSION);
        assert_eq!(decoded.metadata.name, "test_module");
        assert_eq!(decoded.mir.name, "test_module");
    }
}
