//! Rayzor Runtime Library
//!
//! Provides memory management and runtime support for compiled Haxe code.
//! Works with both JIT and AOT compilation.
//!
//! # Architecture
//!
//! - **JIT Mode**: Runtime is linked into the JIT process, functions are called directly
//! - **AOT Mode**: Runtime is statically linked or compiled alongside the output binary
//!
//! # Memory Management
//!
//! Uses Rust's global allocator (`std::alloc::Global`) which is:
//! - Fast and efficient
//! - Memory-safe
//! - Platform-independent
//! - No external C dependencies

use std::alloc::{alloc, dealloc, realloc, Layout};
use std::ptr;

// Export Vec module (old API - keeping for backward compat)
pub mod vec;
// Note: old string.rs removed due to ABI issues with struct returns

// Export Haxe core type runtime modules
pub mod vec_plugin;      // Pointer-based Vec API
pub mod haxe_string;     // Comprehensive String API
pub mod haxe_array;      // Dynamic Array API
pub mod haxe_math;       // Math functions
pub mod haxe_sys;        // System/IO functions
pub mod concurrency;     // Concurrency primitives (Thread, Arc, Mutex, Channel)

pub mod plugin_impl;     // Plugin registration

// Re-export main types
pub use vec::HaxeVec;
pub use haxe_string::HaxeString;
pub use haxe_array::HaxeArray;

// Re-export plugin
pub use plugin_impl::get_plugin;

/// Allocate memory on the heap
///
/// # Safety
/// The returned pointer must be freed with `rayzor_free` when no longer needed.
///
/// # Arguments
/// * `size` - Number of bytes to allocate
///
/// # Returns
/// Pointer to allocated memory, or null on failure
#[no_mangle]
pub unsafe extern "C" fn rayzor_malloc(size: u64) -> *mut u8 {
    if size == 0 {
        return ptr::null_mut();
    }

    // Create layout for allocation
    let layout = match Layout::from_size_align(size as usize, 1) {
        Ok(layout) => layout,
        Err(_) => return ptr::null_mut(),
    };

    // Allocate memory
    let ptr = alloc(layout);

    if ptr.is_null() {
        return ptr::null_mut();
    }

    ptr
}

/// Reallocate memory to a new size
///
/// # Safety
/// - `ptr` must have been allocated by `rayzor_malloc` or `rayzor_realloc`
/// - If reallocation fails, the original pointer remains valid
///
/// # Arguments
/// * `ptr` - Pointer to existing allocation
/// * `old_size` - Original size in bytes
/// * `new_size` - New size in bytes
///
/// # Returns
/// Pointer to reallocated memory, or null on failure
#[no_mangle]
pub unsafe extern "C" fn rayzor_realloc(ptr: *mut u8, old_size: u64, new_size: u64) -> *mut u8 {
    if ptr.is_null() {
        return rayzor_malloc(new_size);
    }

    if new_size == 0 {
        rayzor_free(ptr, old_size);
        return ptr::null_mut();
    }

    // Create layouts
    let old_layout = match Layout::from_size_align(old_size as usize, 1) {
        Ok(layout) => layout,
        Err(_) => return ptr::null_mut(),
    };

    // Reallocate
    let new_ptr = realloc(ptr, old_layout, new_size as usize);

    if new_ptr.is_null() {
        return ptr::null_mut();
    }

    new_ptr
}

/// Free allocated memory
///
/// # Safety
/// - `ptr` must have been allocated by `rayzor_malloc` or `rayzor_realloc`
/// - `size` must match the size used when allocating
/// - After calling this function, `ptr` is invalid and must not be used
///
/// # Arguments
/// * `ptr` - Pointer to memory to free
/// * `size` - Size of the allocation in bytes
#[no_mangle]
pub unsafe extern "C" fn rayzor_free(ptr: *mut u8, size: u64) {
    if ptr.is_null() || size == 0 {
        return;
    }

    // Create layout
    let layout = match Layout::from_size_align(size as usize, 1) {
        Ok(layout) => layout,
        Err(_) => return, // Invalid layout, can't free
    };

    // Deallocate
    dealloc(ptr, layout);
}

/// Initialize the runtime (called before any other runtime functions)
#[no_mangle]
pub extern "C" fn rayzor_runtime_init() {
    // Nothing to do for now - Rust's global allocator is always initialized
}

/// Shutdown the runtime (called when program exits)
#[no_mangle]
pub extern "C" fn rayzor_runtime_shutdown() {
    // Nothing to do - Rust handles cleanup automatically
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_malloc_free() {
        unsafe {
            let ptr = rayzor_malloc(100);
            assert!(!ptr.is_null());

            // Write some data
            *ptr = 42;
            assert_eq!(*ptr, 42);

            rayzor_free(ptr, 100);
        }
    }

    #[test]
    fn test_realloc() {
        unsafe {
            // Allocate 10 bytes
            let ptr = rayzor_malloc(10);
            assert!(!ptr.is_null());

            // Write data
            for i in 0..10 {
                *ptr.add(i) = i as u8;
            }

            // Reallocate to 20 bytes
            let new_ptr = rayzor_realloc(ptr, 10, 20);
            assert!(!new_ptr.is_null());

            // Check that old data is preserved
            for i in 0..10 {
                assert_eq!(*new_ptr.add(i), i as u8);
            }

            rayzor_free(new_ptr, 20);
        }
    }

    #[test]
    fn test_zero_size() {
        unsafe {
            let ptr = rayzor_malloc(0);
            assert!(ptr.is_null());
        }
    }

    #[test]
    fn test_realloc_null() {
        unsafe {
            // Realloc with null ptr should act like malloc
            let ptr = rayzor_realloc(ptr::null_mut(), 0, 100);
            assert!(!ptr.is_null());
            rayzor_free(ptr, 100);
        }
    }
}
