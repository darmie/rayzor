//! Rayzor GPU Compute — opt-in native package
//!
//! Provides GPU-accelerated compute via Metal (macOS), with CUDA and WebGPU
//! planned for future phases. Ships as a cdylib loaded at runtime via dlopen.
//!
//! # Plugin Registration
//!
//! Method descriptors are declared via [`declare_native_methods!`] and exported
//! through `rayzor_gpu_plugin_describe()`. The compiler reads these at load time
//! to auto-register method mappings and extern declarations — **no compiler core
//! changes required**.

pub mod buffer;
pub mod device;

#[cfg(target_os = "macos")]
pub mod metal;

use rayzor_plugin::{NativeMethodDesc, declare_native_methods};
use std::ffi::c_void;

// ============================================================================
// Method descriptor table (read by compiler at plugin load time)
// ============================================================================

declare_native_methods! {
    GPU_METHODS;
    // GPUCompute lifecycle (static)
    "rayzor_gpu_GPUCompute", "create",       static,   "rayzor_gpu_compute_create",        []              => Ptr;
    "rayzor_gpu_GPUCompute", "isAvailable",  static,   "rayzor_gpu_compute_is_available",  []              => I64;
    // GPUCompute instance methods (self = Ptr is first param)
    "rayzor_gpu_GPUCompute", "destroy",      instance, "rayzor_gpu_compute_destroy",       [Ptr]           => Void;
    "rayzor_gpu_GPUCompute", "createBuffer", instance, "rayzor_gpu_compute_create_buffer", [Ptr, Ptr]      => Ptr;
    "rayzor_gpu_GPUCompute", "allocBuffer",  instance, "rayzor_gpu_compute_alloc_buffer",  [Ptr, I64, I64] => Ptr;
    "rayzor_gpu_GPUCompute", "toTensor",     instance, "rayzor_gpu_compute_to_tensor",     [Ptr, Ptr]      => Ptr;
    "rayzor_gpu_GPUCompute", "freeBuffer",   instance, "rayzor_gpu_compute_free_buffer",   [Ptr, Ptr]      => Void;
    // GpuBuffer instance methods
    "rayzor_gpu_GpuBuffer",  "numel",        instance, "rayzor_gpu_compute_buffer_numel",  [Ptr]           => I64;
    "rayzor_gpu_GpuBuffer",  "dtype",        instance, "rayzor_gpu_compute_buffer_dtype",  [Ptr]           => I64;
}

// ============================================================================
// Plugin exports (called by host via dlopen/dlsym)
// ============================================================================

/// Symbol table entry for plugin registration
#[repr(C)]
pub struct SymbolEntry {
    pub name: *const u8,
    pub name_len: usize,
    pub ptr: *const c_void,
}

/// Plugin initialization — returns a flat symbol table for JIT linking.
#[no_mangle]
pub extern "C" fn rayzor_gpu_plugin_init(out_count: *mut usize) -> *const SymbolEntry {
    let symbols = collect_symbols();
    let count = symbols.len();
    let ptr = symbols.as_ptr();
    std::mem::forget(symbols); // caller does not free — lives for process lifetime
    if !out_count.is_null() {
        unsafe { *out_count = count; }
    }
    ptr
}

/// Returns method descriptors for compiler-side registration.
///
/// The compiler reads these to auto-generate method mappings and extern
/// declarations — no manual MIR wrappers or compiler core changes needed.
#[no_mangle]
pub extern "C" fn rayzor_gpu_plugin_describe(out_count: *mut usize) -> *const NativeMethodDesc {
    if !out_count.is_null() {
        unsafe { *out_count = GPU_METHODS.len(); }
    }
    GPU_METHODS.as_ptr()
}

/// Rust-callable API returning runtime symbols.
pub fn get_runtime_symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        // Device lifecycle
        ("rayzor_gpu_compute_create", device::rayzor_gpu_compute_create as *const u8),
        ("rayzor_gpu_compute_destroy", device::rayzor_gpu_compute_destroy as *const u8),
        ("rayzor_gpu_compute_is_available", device::rayzor_gpu_compute_is_available as *const u8),
        // Buffer management
        ("rayzor_gpu_compute_create_buffer", buffer::rayzor_gpu_compute_create_buffer as *const u8),
        ("rayzor_gpu_compute_alloc_buffer", buffer::rayzor_gpu_compute_alloc_buffer as *const u8),
        ("rayzor_gpu_compute_to_tensor", buffer::rayzor_gpu_compute_to_tensor as *const u8),
        ("rayzor_gpu_compute_free_buffer", buffer::rayzor_gpu_compute_free_buffer as *const u8),
        ("rayzor_gpu_compute_buffer_numel", buffer::rayzor_gpu_compute_buffer_numel as *const u8),
        ("rayzor_gpu_compute_buffer_dtype", buffer::rayzor_gpu_compute_buffer_dtype as *const u8),
    ]
}

fn collect_symbols() -> Vec<SymbolEntry> {
    get_runtime_symbols()
        .into_iter()
        .map(|(name, ptr)| SymbolEntry {
            name: name.as_ptr(),
            name_len: name.len(),
            ptr: ptr as *const c_void,
        })
        .collect()
}

/// GPU compute plugin implementing RuntimePlugin trait
pub struct GpuComputePlugin;

impl rayzor_plugin::RuntimePlugin for GpuComputePlugin {
    fn name(&self) -> &str {
        "rayzor_gpu_compute"
    }

    fn runtime_symbols(&self) -> Vec<(&'static str, *const u8)> {
        get_runtime_symbols()
    }
}
