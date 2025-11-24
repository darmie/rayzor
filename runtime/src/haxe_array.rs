//! Haxe Array runtime implementation
//!
//! Generic dynamic array supporting any element type
//! Memory layout: [length: usize, capacity: usize, elements...]

use std::alloc::{alloc, dealloc, realloc, Layout};
use std::ptr;

/// Haxe Array representation (generic via element size)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct HaxeArray {
    pub ptr: *mut u8,   // Pointer to array data
    pub len: usize,     // Number of elements
    pub cap: usize,     // Capacity (number of elements)
    pub elem_size: usize, // Size of each element in bytes
}

const INITIAL_CAPACITY: usize = 8;

// ============================================================================
// Array Creation
// ============================================================================

/// Create a new empty array
#[no_mangle]
pub extern "C" fn haxe_array_new(out: *mut HaxeArray, elem_size: usize) {
    unsafe {
        let total_size = INITIAL_CAPACITY * elem_size;
        let layout = Layout::from_size_align_unchecked(total_size, 8);
        let ptr = alloc(layout);

        if ptr.is_null() {
            panic!("Failed to allocate memory for Array");
        }

        (*out).ptr = ptr;
        (*out).len = 0;
        (*out).cap = INITIAL_CAPACITY;
        (*out).elem_size = elem_size;
    }
}

/// Create array from existing elements
#[no_mangle]
pub extern "C" fn haxe_array_from_elements(
    out: *mut HaxeArray,
    elements: *const u8,
    count: usize,
    elem_size: usize
) {
    if count == 0 {
        haxe_array_new(out, elem_size);
        return;
    }

    unsafe {
        let cap = count.max(INITIAL_CAPACITY);
        let total_size = cap * elem_size;
        let layout = Layout::from_size_align_unchecked(total_size, 8);
        let ptr = alloc(layout);

        if ptr.is_null() {
            panic!("Failed to allocate memory for Array");
        }

        // Copy elements
        ptr::copy_nonoverlapping(elements, ptr, count * elem_size);

        (*out).ptr = ptr;
        (*out).len = count;
        (*out).cap = cap;
        (*out).elem_size = elem_size;
    }
}

// ============================================================================
// Array Properties
// ============================================================================

/// Get array length
#[no_mangle]
pub extern "C" fn haxe_array_length(arr: *const HaxeArray) -> usize {
    eprintln!("[DEBUG haxe_array_length] Called with arr={:?}", arr);
    if arr.is_null() {
        eprintln!("[DEBUG haxe_array_length] arr is null, returning 0");
        return 0;
    }
    let len = unsafe { (*arr).len };
    eprintln!("[DEBUG haxe_array_length] arr.len={}", len);
    len
}

// ============================================================================
// Element Access
// ============================================================================

/// Get element at index (copies to out buffer)
#[no_mangle]
pub extern "C" fn haxe_array_get(arr: *const HaxeArray, index: usize, out: *mut u8) -> bool {
    if arr.is_null() || out.is_null() {
        return false;
    }

    unsafe {
        let arr_ref = &*arr;
        if index >= arr_ref.len {
            return false;
        }

        let elem_ptr = arr_ref.ptr.add(index * arr_ref.elem_size);
        ptr::copy_nonoverlapping(elem_ptr, out, arr_ref.elem_size);
        true
    }
}

/// Set element at index (copies from data buffer)
#[no_mangle]
pub extern "C" fn haxe_array_set(arr: *mut HaxeArray, index: usize, data: *const u8) -> bool {
    if arr.is_null() || data.is_null() {
        return false;
    }

    unsafe {
        let arr_ref = &mut *arr;
        if index >= arr_ref.len {
            return false;
        }

        let elem_ptr = arr_ref.ptr.add(index * arr_ref.elem_size);
        ptr::copy_nonoverlapping(data, elem_ptr, arr_ref.elem_size);
        true
    }
}

/// Get pointer to element (for direct access)
#[no_mangle]
pub extern "C" fn haxe_array_get_ptr(arr: *const HaxeArray, index: usize) -> *mut u8 {
    if arr.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        let arr_ref = &*arr;
        if index >= arr_ref.len {
            return ptr::null_mut();
        }

        arr_ref.ptr.add(index * arr_ref.elem_size)
    }
}

// ============================================================================
// Array Modification
// ============================================================================

/// Push element onto array
#[no_mangle]
pub extern "C" fn haxe_array_push(arr: *mut HaxeArray, data: *const u8) {
    eprintln!("[DEBUG haxe_array_push] Called with arr={:?}, data={:?}", arr, data);
    if arr.is_null() || data.is_null() {
        eprintln!("[DEBUG haxe_array_push] arr or data is null, returning");
        return;
    }
    eprintln!("[DEBUG haxe_array_push] Before: arr.ptr={:?}, arr.len={}, arr.cap={}, arr.elem_size={}",
              unsafe { (*arr).ptr }, unsafe { (*arr).len }, unsafe { (*arr).cap }, unsafe { (*arr).elem_size });

    unsafe {
        let arr_ref = &mut *arr;

        // Check if we need to grow
        if arr_ref.len >= arr_ref.cap {
            let new_cap = if arr_ref.cap == 0 {
                INITIAL_CAPACITY
            } else {
                arr_ref.cap * 2
            };

            let new_size = new_cap * arr_ref.elem_size;

            let new_ptr = if arr_ref.ptr.is_null() || arr_ref.cap == 0 {
                // First allocation - use alloc instead of realloc
                let layout = Layout::from_size_align_unchecked(new_size, 8);
                alloc(layout)
            } else {
                // Grow existing allocation
                let old_size = arr_ref.cap * arr_ref.elem_size;
                let old_layout = Layout::from_size_align_unchecked(old_size, 8);
                realloc(arr_ref.ptr, old_layout, new_size)
            };

            if new_ptr.is_null() {
                panic!("Failed to allocate/reallocate memory for Array");
            }

            arr_ref.ptr = new_ptr;
            arr_ref.cap = new_cap;
        }

        // Add element
        let elem_ptr = arr_ref.ptr.add(arr_ref.len * arr_ref.elem_size);
        ptr::copy_nonoverlapping(data, elem_ptr, arr_ref.elem_size);
        arr_ref.len += 1;
    }
}

/// Pop element from array
#[no_mangle]
pub extern "C" fn haxe_array_pop(arr: *mut HaxeArray, out: *mut u8) -> bool {
    if arr.is_null() {
        return false;
    }

    unsafe {
        let arr_ref = &mut *arr;
        if arr_ref.len == 0 {
            return false;
        }

        arr_ref.len -= 1;

        if !out.is_null() {
            let elem_ptr = arr_ref.ptr.add(arr_ref.len * arr_ref.elem_size);
            ptr::copy_nonoverlapping(elem_ptr, out, arr_ref.elem_size);
        }

        true
    }
}

/// Insert element at index
#[no_mangle]
pub extern "C" fn haxe_array_insert(arr: *mut HaxeArray, index: usize, data: *const u8) {
    if arr.is_null() || data.is_null() {
        return;
    }

    unsafe {
        let arr_ref = &mut *arr;
        let insert_pos = index.min(arr_ref.len);

        // Ensure capacity
        if arr_ref.len >= arr_ref.cap {
            let new_cap = arr_ref.cap * 2;
            let old_size = arr_ref.cap * arr_ref.elem_size;
            let new_size = new_cap * arr_ref.elem_size;

            let old_layout = Layout::from_size_align_unchecked(old_size, 8);
            let new_ptr = realloc(arr_ref.ptr, old_layout, new_size);

            if new_ptr.is_null() {
                panic!("Failed to reallocate memory for Array");
            }

            arr_ref.ptr = new_ptr;
            arr_ref.cap = new_cap;
        }

        // Shift elements to the right
        if insert_pos < arr_ref.len {
            let src = arr_ref.ptr.add(insert_pos * arr_ref.elem_size);
            let dst = src.add(arr_ref.elem_size);
            let count = (arr_ref.len - insert_pos) * arr_ref.elem_size;
            ptr::copy(src, dst, count);
        }

        // Insert new element
        let elem_ptr = arr_ref.ptr.add(insert_pos * arr_ref.elem_size);
        ptr::copy_nonoverlapping(data, elem_ptr, arr_ref.elem_size);
        arr_ref.len += 1;
    }
}

/// Remove element at index
#[no_mangle]
pub extern "C" fn haxe_array_remove(arr: *mut HaxeArray, index: usize) -> bool {
    if arr.is_null() {
        return false;
    }

    unsafe {
        let arr_ref = &mut *arr;
        if index >= arr_ref.len {
            return false;
        }

        // Shift elements to the left
        if index < arr_ref.len - 1 {
            let src = arr_ref.ptr.add((index + 1) * arr_ref.elem_size);
            let dst = arr_ref.ptr.add(index * arr_ref.elem_size);
            let count = (arr_ref.len - index - 1) * arr_ref.elem_size;
            ptr::copy(src, dst, count);
        }

        arr_ref.len -= 1;
        true
    }
}

/// Reverse array in place
#[no_mangle]
pub extern "C" fn haxe_array_reverse(arr: *mut HaxeArray) {
    if arr.is_null() {
        return;
    }

    unsafe {
        let arr_ref = &mut *arr;
        if arr_ref.len <= 1 {
            return;
        }

        let elem_size = arr_ref.elem_size;
        let mut i = 0;
        let mut j = arr_ref.len - 1;

        // Allocate temp buffer for swapping
        let temp_layout = Layout::from_size_align_unchecked(elem_size, 8);
        let temp = alloc(temp_layout);

        while i < j {
            let left = arr_ref.ptr.add(i * elem_size);
            let right = arr_ref.ptr.add(j * elem_size);

            // Swap via temp buffer
            ptr::copy_nonoverlapping(left, temp, elem_size);
            ptr::copy_nonoverlapping(right, left, elem_size);
            ptr::copy_nonoverlapping(temp, right, elem_size);

            i += 1;
            j -= 1;
        }

        dealloc(temp, temp_layout);
    }
}

/// Copy array
#[no_mangle]
pub extern "C" fn haxe_array_copy(out: *mut HaxeArray, arr: *const HaxeArray) {
    if arr.is_null() {
        return;
    }

    unsafe {
        let arr_ref = &*arr;
        haxe_array_from_elements(out, arr_ref.ptr, arr_ref.len, arr_ref.elem_size);
    }
}

/// Slice array
#[no_mangle]
pub extern "C" fn haxe_array_slice(out: *mut HaxeArray, arr: *const HaxeArray, start: usize, end: usize) {
    if arr.is_null() {
        return;
    }

    unsafe {
        let arr_ref = &*arr;
        let actual_start = start.min(arr_ref.len);
        let actual_end = end.min(arr_ref.len);

        if actual_start >= actual_end {
            haxe_array_new(out, arr_ref.elem_size);
            return;
        }

        let count = actual_end - actual_start;
        let start_ptr = arr_ref.ptr.add(actual_start * arr_ref.elem_size);
        haxe_array_from_elements(out, start_ptr, count, arr_ref.elem_size);
    }
}

// ============================================================================
// Memory Management
// ============================================================================

/// Free array memory
#[no_mangle]
pub extern "C" fn haxe_array_free(arr: *mut HaxeArray) {
    if arr.is_null() {
        return;
    }

    unsafe {
        let arr_ref = &*arr;
        if !arr_ref.ptr.is_null() && arr_ref.cap > 0 {
            let total_size = arr_ref.cap * arr_ref.elem_size;
            let layout = Layout::from_size_align_unchecked(total_size, 8);
            dealloc(arr_ref.ptr, layout);
        }
    }
}

// ============================================================================
// Specialized Integer Array Functions
// ============================================================================

/// Push i32 onto array
#[no_mangle]
pub extern "C" fn haxe_array_push_i32(arr: *mut HaxeArray, value: i32) {
    haxe_array_push(arr, &value as *const i32 as *const u8);
}

/// Get i32 from array
#[no_mangle]
pub extern "C" fn haxe_array_get_i32(arr: *const HaxeArray, index: usize) -> i32 {
    let mut value: i32 = 0;
    if haxe_array_get(arr, index, &mut value as *mut i32 as *mut u8) {
        value
    } else {
        0
    }
}

/// Push i64 onto array
#[no_mangle]
pub extern "C" fn haxe_array_push_i64(arr: *mut HaxeArray, value: i64) {
    haxe_array_push(arr, &value as *const i64 as *const u8);
}

/// Get i64 from array
#[no_mangle]
pub extern "C" fn haxe_array_get_i64(arr: *const HaxeArray, index: usize) -> i64 {
    let mut value: i64 = 0;
    if haxe_array_get(arr, index, &mut value as *mut i64 as *mut u8) {
        value
    } else {
        0
    }
}

/// Push f64 onto array
#[no_mangle]
pub extern "C" fn haxe_array_push_f64(arr: *mut HaxeArray, value: f64) {
    haxe_array_push(arr, &value as *const f64 as *const u8);
}

/// Get f64 from array
#[no_mangle]
pub extern "C" fn haxe_array_get_f64(arr: *const HaxeArray, index: usize) -> f64 {
    let mut value: f64 = 0.0;
    if haxe_array_get(arr, index, &mut value as *mut f64 as *mut u8) {
        value
    } else {
        0.0
    }
}
