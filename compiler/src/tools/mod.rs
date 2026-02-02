//! Extracted tool logic for use as library functions.
//!
//! These modules contain the core logic that was previously only available
//! through standalone binaries (`preblade`, `rayzor-build`). They can now
//! be called from the unified `rayzor` CLI or programmatically.

pub mod aot_build;
pub mod preblade;
