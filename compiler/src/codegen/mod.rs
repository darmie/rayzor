/// Code generation backends for Rayzor
///
/// This module contains the code generation infrastructure for targeting
/// different backends:
/// - Cranelift (JIT with tiered compilation)
/// - LLVM (AOT compilation - future)
/// - WebAssembly (cross-platform AOT - future)

pub mod cranelift_backend;
pub mod profiling;
pub mod tiered_backend;
pub mod llvm_jit_backend;
mod instruction_lowering;

// Apple Silicon-specific JIT memory management
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
pub mod apple_jit_memory;

pub use cranelift_backend::CraneliftBackend;
pub use profiling::{HotnessLevel, ProfileConfig, ProfileData, ProfileStatistics};
pub use tiered_backend::{OptimizationTier, TieredBackend, TieredConfig, TieredStatistics};

#[cfg(feature = "llvm-backend")]
pub use llvm_jit_backend::LLVMJitBackend;
