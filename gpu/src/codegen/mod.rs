//! GPU kernel source code generation.
//!
//! Translates `KernelOp` descriptors into backend-specific shader source
//! strings that are runtime-compiled on the GPU device.

pub mod msl;
pub mod msl_matmul;
pub mod msl_reduction;
