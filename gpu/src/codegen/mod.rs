//! GPU kernel source code generation.
//!
//! Translates `KernelOp` descriptors into backend-specific shader source
//! strings that are runtime-compiled on the GPU device.

#[cfg(feature = "metal-backend")]
pub mod msl;
#[cfg(feature = "metal-backend")]
pub mod msl_fused;
#[cfg(feature = "metal-backend")]
pub mod msl_matmul;
#[cfg(feature = "metal-backend")]
pub mod msl_reduction;

#[cfg(feature = "webgpu-backend")]
pub mod wgsl;
#[cfg(feature = "webgpu-backend")]
pub mod wgsl_fused;
#[cfg(feature = "webgpu-backend")]
pub mod wgsl_matmul;
#[cfg(feature = "webgpu-backend")]
pub mod wgsl_reduction;
