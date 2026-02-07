//! GPU compute context — device initialization and lifecycle

#[cfg(target_os = "macos")]
use crate::metal::device_init;

/// Opaque GPU context handle passed as i64 through the JIT ABI.
///
/// On macOS this wraps an MTLDevice + MTLCommandQueue.
/// On unsupported platforms all operations return null/0.
pub struct GpuContext {
    #[cfg(target_os = "macos")]
    pub(crate) inner: device_init::MetalContext,
}

// ---------------------------------------------------------------------------
// Extern C API
// ---------------------------------------------------------------------------

/// Create a new GPU compute context.
/// Returns an opaque i64 handle (pointer), or 0 on failure.
#[no_mangle]
pub extern "C" fn rayzor_gpu_compute_create() -> i64 {
    #[cfg(target_os = "macos")]
    {
        match device_init::MetalContext::new() {
            Some(ctx) => {
                let gpu_ctx = GpuContext { inner: ctx };
                let boxed = Box::new(gpu_ctx);
                Box::into_raw(boxed) as i64
            }
            None => 0,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

/// Destroy a GPU compute context and free its resources.
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_destroy(ctx: i64) {
    if ctx == 0 {
        return;
    }
    let _ = Box::from_raw(ctx as *mut GpuContext);
}

/// Check if GPU compute is available on this system.
/// Returns 1 if available, 0 otherwise.
#[no_mangle]
pub extern "C" fn rayzor_gpu_compute_is_available() -> i64 {
    #[cfg(target_os = "macos")]
    {
        if device_init::MetalContext::is_available() { 1 } else { 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}
