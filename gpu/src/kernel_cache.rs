//! Kernel cache — avoids recompiling the same MSL kernel multiple times.
//!
//! Keyed by (KernelOp, dtype), since the same op+dtype always produces
//! identical MSL source. The cache lives for the lifetime of the GpuContext.

use std::collections::HashMap;

use crate::kernel_ir::KernelOp;

#[cfg(target_os = "macos")]
use crate::codegen::msl;
#[cfg(target_os = "macos")]
use crate::metal::{compile, device_init::MetalContext};

/// Cache key: (operation, dtype tag).
type CacheKey = (KernelOp, u8);

/// Cached compiled kernel with associated metadata.
pub struct CachedKernel {
    #[cfg(target_os = "macos")]
    pub compiled: compile::CompiledKernel,
    #[cfg(not(target_os = "macos"))]
    _placeholder: (),
}

/// Thread-local kernel cache per GPU context.
pub struct KernelCache {
    #[cfg(target_os = "macos")]
    entries: HashMap<CacheKey, CachedKernel>,
    #[cfg(not(target_os = "macos"))]
    _placeholder: (),
}

impl KernelCache {
    pub fn new() -> Self {
        KernelCache {
            #[cfg(target_os = "macos")]
            entries: HashMap::new(),
            #[cfg(not(target_os = "macos"))]
            _placeholder: (),
        }
    }

    /// Get or compile a kernel for the given op and dtype.
    ///
    /// Returns a reference to the compiled kernel on success.
    #[cfg(target_os = "macos")]
    pub fn get_or_compile(
        &mut self,
        ctx: &MetalContext,
        op: KernelOp,
        dtype: u8,
    ) -> Result<&CachedKernel, String> {
        let key = (op, dtype);

        if !self.entries.contains_key(&key) {
            let source = msl::emit_kernel(op, dtype);
            let fn_name = msl::kernel_fn_name(op, dtype);
            let compiled = compile::compile_msl(ctx, &source, &fn_name)?;
            self.entries.insert(key, CachedKernel { compiled });
        }

        Ok(self.entries.get(&key).unwrap())
    }

    /// Number of cached kernels.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        #[cfg(target_os = "macos")]
        {
            self.entries.len()
        }
        #[cfg(not(target_os = "macos"))]
        {
            0
        }
    }
}
