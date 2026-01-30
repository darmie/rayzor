//! Box<T> runtime functions — single-owner heap allocation.
//!
//! Box<T> is a zero-cost abstract over Int (i64) in Haxe.
//! At runtime, a Box is just a heap pointer. These functions provide
//! the allocation and deallocation primitives.
//!
//! Most Box operations (asPtr, asRef, raw, unbox) are identity or load
//! operations handled at MIR level. Only init and free need runtime support.

use std::ptr;

/// Allocate a value on the heap (Box.init).
///
/// Takes an i64 value (which may be a pointer to a larger object or a primitive),
/// allocates 8 bytes on the heap, stores the value, and returns the heap pointer.
///
/// # Safety
/// - Caller must eventually call `rayzor_box_free` to release the memory.
#[no_mangle]
pub unsafe extern "C" fn rayzor_box_init(value: i64) -> *mut u8 {
    let layout = std::alloc::Layout::new::<i64>();
    let ptr = std::alloc::alloc(layout);
    if ptr.is_null() {
        return ptr::null_mut();
    }
    *(ptr as *mut i64) = value;
    ptr
}

/// Free a Box allocation.
///
/// # Safety
/// - `box_ptr` must be a valid pointer returned by `rayzor_box_init`.
/// - Must not be called more than once on the same pointer.
#[no_mangle]
pub unsafe extern "C" fn rayzor_box_free(box_ptr: *mut u8) {
    if box_ptr.is_null() {
        return;
    }
    let layout = std::alloc::Layout::new::<i64>();
    std::alloc::dealloc(box_ptr, layout);
}

/// Read the value from a Box (Box.unbox).
///
/// Returns the stored i64 value. Does NOT free the box.
///
/// # Safety
/// - `box_ptr` must be a valid pointer returned by `rayzor_box_init`.
#[no_mangle]
pub unsafe extern "C" fn rayzor_box_unbox(box_ptr: *const u8) -> i64 {
    if box_ptr.is_null() {
        return 0;
    }
    *(box_ptr as *const i64)
}

/// Get the raw heap address as i64 (Box.raw / Box.asPtr / Box.asRef).
///
/// This is an identity operation — the Box IS the pointer.
///
/// # Safety
/// - `box_ptr` must be a valid pointer returned by `rayzor_box_init`.
#[no_mangle]
pub unsafe extern "C" fn rayzor_box_raw(box_ptr: *const u8) -> i64 {
    box_ptr as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_init_unbox_free() {
        unsafe {
            let boxed = rayzor_box_init(42);
            assert!(!boxed.is_null());
            assert_eq!(rayzor_box_unbox(boxed), 42);
            assert_eq!(rayzor_box_raw(boxed) as *const u8, boxed);
            rayzor_box_free(boxed);
        }
    }

    #[test]
    fn test_box_null_safety() {
        unsafe {
            assert_eq!(rayzor_box_unbox(ptr::null()), 0);
            rayzor_box_free(ptr::null_mut()); // should not crash
        }
    }
}
