//! Haxe Sys runtime implementation
//!
//! System and I/O functions

use std::io::{self, Write};

// Use the canonical HaxeString definition from haxe_string module
use crate::haxe_string::HaxeString;

// ============================================================================
// Console I/O
// ============================================================================

/// Print integer to stdout
#[no_mangle]
pub extern "C" fn haxe_sys_print_int(value: i64) {
    print!("{}", value);
    let _ = io::stdout().flush();
}

/// Print float to stdout
#[no_mangle]
pub extern "C" fn haxe_sys_print_float(value: f64) {
    print!("{}", value);
    let _ = io::stdout().flush();
}

/// Print boolean to stdout
#[no_mangle]
pub extern "C" fn haxe_sys_print_bool(value: bool) {
    print!("{}", value);
    let _ = io::stdout().flush();
}

/// Print newline
#[no_mangle]
pub extern "C" fn haxe_sys_println() {
    println!();
}

// ============================================================================
// Trace Functions (Runtime Logging)
// ============================================================================

/// Trace integer value
#[no_mangle]
pub extern "C" fn haxe_trace_int(value: i64) {
    println!("{}", value);
}

/// Trace float value
#[no_mangle]
pub extern "C" fn haxe_trace_float(value: f64) {
    println!("{}", value);
}

/// Trace boolean value
#[no_mangle]
pub extern "C" fn haxe_trace_bool(value: bool) {
    println!("{}", value);
}

/// Trace string value (ptr + len)
#[no_mangle]
pub extern "C" fn haxe_trace_string(ptr: *const u8, len: usize) {
    if ptr.is_null() {
        println!("null");
        return;
    }

    unsafe {
        let slice = std::slice::from_raw_parts(ptr, len);
        match std::str::from_utf8(slice) {
            Ok(s) => println!("{}", s),
            Err(_) => println!("<invalid utf8>"),
        }
    }
}

/// Trace string value (takes pointer to HaxeString struct)
#[no_mangle]
pub extern "C" fn haxe_trace_string_struct(s_ptr: *const HaxeString) {
    if s_ptr.is_null() {
        println!("null");
        return;
    }
    unsafe {
        let s = &*s_ptr;
        haxe_trace_string(s.ptr, s.len);
    }
}

/// Trace any Dynamic value using Std.string() for proper type dispatch
/// The value is expected to be a pointer to a DynamicValue (boxed Dynamic)
#[no_mangle]
pub extern "C" fn haxe_trace_any(dynamic_ptr: *mut u8) {
    if dynamic_ptr.is_null() {
        println!("null");
        return;
    }

    unsafe {
        // Call haxe_std_string_ptr to convert Dynamic to HaxeString
        let string_ptr = crate::type_system::haxe_std_string_ptr(dynamic_ptr);

        if !string_ptr.is_null() {
            let haxe_str = &*string_ptr;
            if !haxe_str.ptr.is_null() && haxe_str.len > 0 {
                let slice = std::slice::from_raw_parts(haxe_str.ptr, haxe_str.len);
                if let Ok(s) = std::str::from_utf8(slice) {
                    println!("{}", s);
                    return;
                }
            }
        }
        // Fallback
        println!("<Dynamic@{:p}>", dynamic_ptr);
    }
}

// ============================================================================
// Std.string() - Type-specific string conversions
// All functions return *mut HaxeString to avoid struct return ABI issues
// ============================================================================

/// Convert Int to String - returns heap-allocated HaxeString pointer
#[no_mangle]
pub extern "C" fn haxe_string_from_int(value: i64) -> *mut HaxeString {
    let s = value.to_string();
    let bytes = s.into_bytes();
    let len = bytes.len();
    let cap = bytes.capacity();
    let ptr = bytes.as_ptr() as *mut u8;
    std::mem::forget(bytes); // Transfer ownership to HaxeString

    Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
}

/// Convert Float to String - returns heap-allocated HaxeString pointer
#[no_mangle]
pub extern "C" fn haxe_string_from_float(value: f64) -> *mut HaxeString {
    let s = value.to_string();
    let bytes = s.into_bytes();
    let len = bytes.len();
    let cap = bytes.capacity();
    let ptr = bytes.as_ptr() as *mut u8;
    std::mem::forget(bytes);

    Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
}

/// Convert Bool to String - returns heap-allocated HaxeString pointer
#[no_mangle]
pub extern "C" fn haxe_string_from_bool(value: bool) -> *mut HaxeString {
    let s = if value { "true" } else { "false" };
    // For static strings, use the static pointer with cap=0 to indicate no-free
    Box::into_raw(Box::new(HaxeString {
        ptr: s.as_ptr() as *mut u8,
        len: s.len(),
        cap: 0, // cap=0 means static string, don't free
    }))
}

/// Convert String to String (identity, but normalizes representation)
#[no_mangle]
pub extern "C" fn haxe_string_from_string(ptr: *const u8, len: usize) -> *mut HaxeString {
    // Create a copy of the string data
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    let vec = slice.to_vec();
    let cap = vec.capacity();
    let new_ptr = vec.as_ptr() as *mut u8;
    std::mem::forget(vec);

    Box::into_raw(Box::new(HaxeString { ptr: new_ptr, len, cap }))
}

/// Convert null to String - returns heap-allocated HaxeString pointer
#[no_mangle]
pub extern "C" fn haxe_string_from_null() -> *mut HaxeString {
    let s = "null";
    Box::into_raw(Box::new(HaxeString {
        ptr: s.as_ptr() as *mut u8,
        len: s.len(),
        cap: 0, // static string
    }))
}

/// Create a string literal from embedded bytes
/// Returns a pointer to a heap-allocated HaxeString struct
/// The bytes are NOT copied - they must remain valid (e.g., in JIT code section)
#[no_mangle]
pub extern "C" fn haxe_string_literal(ptr: *const u8, len: usize) -> *mut HaxeString {
    Box::into_raw(Box::new(HaxeString {
        ptr: ptr as *mut u8,
        len,
        cap: 0  // cap=0 means static/borrowed, don't free the data
    }))
}

/// Convert string to uppercase (wrapper returning pointer)
/// Takes pointer to input string, returns pointer to new heap-allocated uppercase string
#[no_mangle]
pub extern "C" fn haxe_string_upper(s: *const HaxeString) -> *mut HaxeString {
    if s.is_null() {
        return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
    }
    unsafe {
        let s_ref = &*s;
        if s_ref.ptr.is_null() || s_ref.len == 0 {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
        }
        let slice = std::slice::from_raw_parts(s_ref.ptr, s_ref.len);
        if let Ok(rust_str) = std::str::from_utf8(slice) {
            let upper = rust_str.to_uppercase();
            let bytes = upper.into_bytes();
            let len = bytes.len();
            let cap = bytes.capacity();
            let ptr = bytes.as_ptr() as *mut u8;
            std::mem::forget(bytes);
            Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
        } else {
            // Invalid UTF-8, return copy of original
            let new_bytes = slice.to_vec();
            let len = new_bytes.len();
            let cap = new_bytes.capacity();
            let ptr = new_bytes.as_ptr() as *mut u8;
            std::mem::forget(new_bytes);
            Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
        }
    }
}

/// Convert string to lowercase (wrapper returning pointer)
/// Takes pointer to input string, returns pointer to new heap-allocated lowercase string
#[no_mangle]
pub extern "C" fn haxe_string_lower(s: *const HaxeString) -> *mut HaxeString {
    if s.is_null() {
        return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
    }
    unsafe {
        let s_ref = &*s;
        if s_ref.ptr.is_null() || s_ref.len == 0 {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
        }
        let slice = std::slice::from_raw_parts(s_ref.ptr, s_ref.len);
        if let Ok(rust_str) = std::str::from_utf8(slice) {
            let lower = rust_str.to_lowercase();
            let bytes = lower.into_bytes();
            let len = bytes.len();
            let cap = bytes.capacity();
            let ptr = bytes.as_ptr() as *mut u8;
            std::mem::forget(bytes);
            Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
        } else {
            // Invalid UTF-8, return copy of original
            let new_bytes = slice.to_vec();
            let len = new_bytes.len();
            let cap = new_bytes.capacity();
            let ptr = new_bytes.as_ptr() as *mut u8;
            std::mem::forget(new_bytes);
            Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
        }
    }
}

// ============================================================================
// String Instance Methods (working with *const HaxeString)
// ============================================================================

/// Get string length
#[no_mangle]
pub extern "C" fn haxe_string_len(s: *const HaxeString) -> i32 {
    if s.is_null() {
        return 0;
    }
    unsafe { (*s).len as i32 }
}

/// Get character at index - returns empty string if out of bounds
/// Note: charAt returns String, not Int, per Haxe specification
#[no_mangle]
pub extern "C" fn haxe_string_char_at_ptr(s: *const HaxeString, index: i32) -> *mut HaxeString {
    if s.is_null() {
        return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
    }
    unsafe {
        let s_ref = &*s;
        if index < 0 || (index as usize) >= s_ref.len || s_ref.ptr.is_null() {
            // Return empty string for out of bounds
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
        }

        // Get the byte at the index
        let byte = *s_ref.ptr.add(index as usize);
        let bytes = vec![byte];
        let len = bytes.len();
        let cap = bytes.capacity();
        let ptr = bytes.as_ptr() as *mut u8;
        std::mem::forget(bytes);
        Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
    }
}

/// Get character code at index - returns -1 (represented as null Int) if out of bounds
#[no_mangle]
pub extern "C" fn haxe_string_char_code_at_ptr(s: *const HaxeString, index: i32) -> i32 {
    if s.is_null() {
        return -1; // null
    }
    unsafe {
        let s_ref = &*s;
        if index < 0 || (index as usize) >= s_ref.len || s_ref.ptr.is_null() {
            return -1; // null
        }
        *s_ref.ptr.add(index as usize) as i32
    }
}

/// Find index of substring, starting from startIndex
/// Returns -1 if not found
#[no_mangle]
pub extern "C" fn haxe_string_index_of_ptr(s: *const HaxeString, needle: *const HaxeString, start_index: i32) -> i32 {
    if s.is_null() || needle.is_null() {
        return -1;
    }
    unsafe {
        let s_ref = &*s;
        let needle_ref = &*needle;

        if s_ref.ptr.is_null() || needle_ref.ptr.is_null() {
            return -1;
        }

        // Empty needle - return start_index (or 0 if start_index < 0)
        if needle_ref.len == 0 {
            return if start_index < 0 { 0 } else { start_index };
        }

        let start = if start_index < 0 { 0 } else { start_index as usize };
        if start >= s_ref.len {
            return -1;
        }

        let haystack = std::slice::from_raw_parts(s_ref.ptr, s_ref.len);
        let needle_bytes = std::slice::from_raw_parts(needle_ref.ptr, needle_ref.len);

        // Simple substring search
        for i in start..=(s_ref.len.saturating_sub(needle_ref.len)) {
            if &haystack[i..i + needle_ref.len] == needle_bytes {
                return i as i32;
            }
        }
        -1
    }
}

/// Find last index of substring, searching backwards from startIndex
/// Returns -1 if not found
#[no_mangle]
pub extern "C" fn haxe_string_last_index_of_ptr(s: *const HaxeString, needle: *const HaxeString, start_index: i32) -> i32 {
    if s.is_null() || needle.is_null() {
        return -1;
    }
    unsafe {
        let s_ref = &*s;
        let needle_ref = &*needle;

        if s_ref.ptr.is_null() || needle_ref.ptr.is_null() {
            return -1;
        }

        // Empty needle - return end of string (or start_index if provided and smaller)
        if needle_ref.len == 0 {
            let len = s_ref.len as i32;
            return if start_index < 0 || start_index >= len { len } else { start_index };
        }

        if needle_ref.len > s_ref.len {
            return -1;
        }

        let haystack = std::slice::from_raw_parts(s_ref.ptr, s_ref.len);
        let needle_bytes = std::slice::from_raw_parts(needle_ref.ptr, needle_ref.len);

        // Calculate the maximum starting position
        let max_start = s_ref.len - needle_ref.len;
        let search_start = if start_index < 0 {
            max_start
        } else {
            (start_index as usize).min(max_start)
        };

        // Search backwards
        for i in (0..=search_start).rev() {
            if &haystack[i..i + needle_ref.len] == needle_bytes {
                return i as i32;
            }
        }
        -1
    }
}

/// Get substring using substr semantics (pos, len)
/// If len is negative, returns empty string
/// If pos is negative, calculated from end
#[no_mangle]
pub extern "C" fn haxe_string_substr_ptr(s: *const HaxeString, pos: i32, len: i32) -> *mut HaxeString {
    if s.is_null() {
        return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
    }
    unsafe {
        let s_ref = &*s;
        if s_ref.ptr.is_null() || s_ref.len == 0 || len < 0 {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
        }

        // Handle negative pos (from end)
        let actual_pos = if pos < 0 {
            let from_end = (-pos) as usize;
            if from_end > s_ref.len {
                0
            } else {
                s_ref.len - from_end
            }
        } else {
            pos as usize
        };

        if actual_pos >= s_ref.len {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
        }

        let available = s_ref.len - actual_pos;
        let actual_len = (len as usize).min(available);

        if actual_len == 0 {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
        }

        let slice = std::slice::from_raw_parts(s_ref.ptr.add(actual_pos), actual_len);
        let bytes = slice.to_vec();
        let new_len = bytes.len();
        let cap = bytes.capacity();
        let ptr = bytes.as_ptr() as *mut u8;
        std::mem::forget(bytes);
        Box::into_raw(Box::new(HaxeString { ptr, len: new_len, cap }))
    }
}

/// Get substring using substring semantics (startIndex, endIndex)
/// Negative indices become 0
/// If startIndex > endIndex, they are swapped
#[no_mangle]
pub extern "C" fn haxe_string_substring_ptr(s: *const HaxeString, start_index: i32, end_index: i32) -> *mut HaxeString {
    if s.is_null() {
        return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
    }
    unsafe {
        let s_ref = &*s;
        if s_ref.ptr.is_null() || s_ref.len == 0 {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
        }

        // Clamp negative values to 0
        let mut start = if start_index < 0 { 0 } else { start_index as usize };
        let mut end = if end_index < 0 { 0 } else { end_index as usize };

        // Clamp to string length
        start = start.min(s_ref.len);
        end = end.min(s_ref.len);

        // Swap if start > end
        if start > end {
            std::mem::swap(&mut start, &mut end);
        }

        if start == end {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
        }

        let slice = std::slice::from_raw_parts(s_ref.ptr.add(start), end - start);
        let bytes = slice.to_vec();
        let new_len = bytes.len();
        let cap = bytes.capacity();
        let ptr = bytes.as_ptr() as *mut u8;
        std::mem::forget(bytes);
        Box::into_raw(Box::new(HaxeString { ptr, len: new_len, cap }))
    }
}

/// Create string from character code (static method)
#[no_mangle]
pub extern "C" fn haxe_string_from_char_code(code: i32) -> *mut HaxeString {
    if code < 0 || code > 0x10FFFF {
        // Invalid code point, return empty string
        return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
    }

    // Convert to char and encode as UTF-8
    if let Some(c) = char::from_u32(code as u32) {
        let mut buf = [0u8; 4];
        let encoded = c.encode_utf8(&mut buf);
        let bytes = encoded.as_bytes().to_vec();
        let len = bytes.len();
        let cap = bytes.capacity();
        let ptr = bytes.as_ptr() as *mut u8;
        std::mem::forget(bytes);
        Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
    } else {
        Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }))
    }
}

/// Copy string (for toString() method)
#[no_mangle]
pub extern "C" fn haxe_string_copy(s: *const HaxeString) -> *mut HaxeString {
    if s.is_null() {
        return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
    }
    unsafe {
        let s_ref = &*s;
        if s_ref.ptr.is_null() || s_ref.len == 0 {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
        }

        let slice = std::slice::from_raw_parts(s_ref.ptr, s_ref.len);
        let bytes = slice.to_vec();
        let len = bytes.len();
        let cap = bytes.capacity();
        let ptr = bytes.as_ptr() as *mut u8;
        std::mem::forget(bytes);
        Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
    }
}

/// Split string by delimiter - returns array pointer and sets length
/// Note: Caller is responsible for freeing the returned array and strings
#[no_mangle]
pub extern "C" fn haxe_string_split_ptr(
    s: *const HaxeString,
    delimiter: *const HaxeString,
    out_len: *mut i64
) -> *mut *mut HaxeString {
    unsafe {
        if out_len.is_null() {
            return std::ptr::null_mut();
        }

        if s.is_null() {
            *out_len = 0;
            return std::ptr::null_mut();
        }

        let s_ref = &*s;

        // Handle null or empty string
        if s_ref.ptr.is_null() || s_ref.len == 0 {
            // Return array with one empty string
            let empty = Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 }));
            let result = Box::into_raw(vec![empty].into_boxed_slice()) as *mut *mut HaxeString;
            *out_len = 1;
            return result;
        }

        let haystack = std::slice::from_raw_parts(s_ref.ptr, s_ref.len);

        // Handle null delimiter - return array with original string
        if delimiter.is_null() {
            let copy = haxe_string_copy(s);
            let result = Box::into_raw(vec![copy].into_boxed_slice()) as *mut *mut HaxeString;
            *out_len = 1;
            return result;
        }

        let delim_ref = &*delimiter;

        // Empty delimiter - split into individual characters
        if delim_ref.ptr.is_null() || delim_ref.len == 0 {
            let mut parts: Vec<*mut HaxeString> = Vec::with_capacity(s_ref.len);
            for i in 0..s_ref.len {
                let byte = *s_ref.ptr.add(i);
                let bytes = vec![byte];
                let cap = bytes.capacity();
                let ptr = bytes.as_ptr() as *mut u8;
                std::mem::forget(bytes);
                parts.push(Box::into_raw(Box::new(HaxeString { ptr, len: 1, cap })));
            }
            *out_len = parts.len() as i64;
            Box::into_raw(parts.into_boxed_slice()) as *mut *mut HaxeString
        } else {
            let delim_bytes = std::slice::from_raw_parts(delim_ref.ptr, delim_ref.len);

            let mut parts: Vec<*mut HaxeString> = Vec::new();
            let mut start = 0;

            while start <= s_ref.len {
                // Find next occurrence of delimiter
                let mut found_at = None;
                for i in start..=(s_ref.len.saturating_sub(delim_ref.len)) {
                    if &haystack[i..i + delim_ref.len] == delim_bytes {
                        found_at = Some(i);
                        break;
                    }
                }

                match found_at {
                    Some(idx) => {
                        // Add substring before delimiter
                        let part_len = idx - start;
                        if part_len == 0 {
                            parts.push(Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 })));
                        } else {
                            let bytes = haystack[start..idx].to_vec();
                            let len = bytes.len();
                            let cap = bytes.capacity();
                            let ptr = bytes.as_ptr() as *mut u8;
                            std::mem::forget(bytes);
                            parts.push(Box::into_raw(Box::new(HaxeString { ptr, len, cap })));
                        }
                        start = idx + delim_ref.len;
                    }
                    None => {
                        // Add remaining string
                        let part_len = s_ref.len - start;
                        if part_len == 0 {
                            parts.push(Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null_mut(), len: 0, cap: 0 })));
                        } else {
                            let bytes = haystack[start..].to_vec();
                            let len = bytes.len();
                            let cap = bytes.capacity();
                            let ptr = bytes.as_ptr() as *mut u8;
                            std::mem::forget(bytes);
                            parts.push(Box::into_raw(Box::new(HaxeString { ptr, len, cap })));
                        }
                        break;
                    }
                }
            }

            *out_len = parts.len() as i64;
            Box::into_raw(parts.into_boxed_slice()) as *mut *mut HaxeString
        }
    }
}

// ============================================================================
// Program Control
// ============================================================================

/// Exit program with code
#[no_mangle]
pub extern "C" fn haxe_sys_exit(code: i32) -> ! {
    std::process::exit(code)
}

/// Get current time in milliseconds
#[no_mangle]
pub extern "C" fn haxe_sys_time() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Get command line arguments count
#[no_mangle]
pub extern "C" fn haxe_sys_args_count() -> i32 {
    std::env::args().count() as i32
}

// ============================================================================
// Environment Variables
// ============================================================================

/// Get environment variable value
/// Returns null if the variable doesn't exist
#[no_mangle]
pub extern "C" fn haxe_sys_get_env(name: *const HaxeString) -> *mut HaxeString {
    if name.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let name_ref = &*name;
        if name_ref.ptr.is_null() || name_ref.len == 0 {
            return std::ptr::null_mut();
        }

        let slice = std::slice::from_raw_parts(name_ref.ptr, name_ref.len);
        let var_name = match std::str::from_utf8(slice) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        match std::env::var(var_name) {
            Ok(value) => {
                let bytes = value.into_bytes();
                let len = bytes.len();
                let cap = bytes.capacity();
                let ptr = bytes.as_ptr() as *mut u8;
                std::mem::forget(bytes);
                Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
            }
            Err(_) => std::ptr::null_mut(), // Variable not found
        }
    }
}

/// Set environment variable value
/// If value is null, removes the environment variable
#[no_mangle]
pub extern "C" fn haxe_sys_put_env(name: *const HaxeString, value: *const HaxeString) {
    if name.is_null() {
        return;
    }

    unsafe {
        let name_ref = &*name;
        if name_ref.ptr.is_null() || name_ref.len == 0 {
            return;
        }

        let slice = std::slice::from_raw_parts(name_ref.ptr, name_ref.len);
        let var_name = match std::str::from_utf8(slice) {
            Ok(s) => s,
            Err(_) => return,
        };

        if value.is_null() {
            // Remove the environment variable
            std::env::remove_var(var_name);
        } else {
            let value_ref = &*value;
            if value_ref.ptr.is_null() {
                std::env::remove_var(var_name);
            } else {
                let value_slice = std::slice::from_raw_parts(value_ref.ptr, value_ref.len);
                if let Ok(val_str) = std::str::from_utf8(value_slice) {
                    std::env::set_var(var_name, val_str);
                }
            }
        }
    }
}

// ============================================================================
// Working Directory
// ============================================================================

/// Get current working directory
#[no_mangle]
pub extern "C" fn haxe_sys_get_cwd() -> *mut HaxeString {
    match std::env::current_dir() {
        Ok(path) => {
            let path_str = path.to_string_lossy().into_owned();
            let bytes = path_str.into_bytes();
            let len = bytes.len();
            let cap = bytes.capacity();
            let ptr = bytes.as_ptr() as *mut u8;
            std::mem::forget(bytes);
            Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Set current working directory
#[no_mangle]
pub extern "C" fn haxe_sys_set_cwd(path: *const HaxeString) {
    if path.is_null() {
        return;
    }

    unsafe {
        let path_ref = &*path;
        if path_ref.ptr.is_null() || path_ref.len == 0 {
            return;
        }

        let slice = std::slice::from_raw_parts(path_ref.ptr, path_ref.len);
        if let Ok(path_str) = std::str::from_utf8(slice) {
            let _ = std::env::set_current_dir(path_str);
        }
    }
}

// ============================================================================
// Sleep
// ============================================================================

/// Sleep for the specified number of seconds
#[no_mangle]
pub extern "C" fn haxe_sys_sleep(seconds: f64) {
    if seconds <= 0.0 {
        return;
    }

    let duration = std::time::Duration::from_secs_f64(seconds);
    std::thread::sleep(duration);
}

// ============================================================================
// System Information
// ============================================================================

/// Get the system/OS name
/// Returns "Windows", "Linux", "Mac", or "BSD"
#[no_mangle]
pub extern "C" fn haxe_sys_system_name() -> *mut HaxeString {
    let name = if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "Mac"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "freebsd") || cfg!(target_os = "openbsd") || cfg!(target_os = "netbsd") {
        "BSD"
    } else {
        "Unknown"
    };

    // Return a static string (cap=0 means no-free)
    Box::into_raw(Box::new(HaxeString {
        ptr: name.as_ptr() as *mut u8,
        len: name.len(),
        cap: 0,
    }))
}

/// Get CPU time for current process (in seconds)
#[no_mangle]
pub extern "C" fn haxe_sys_cpu_time() -> f64 {
    // This is a simplified implementation - full accuracy would require platform-specific code
    // On Unix, we could use getrusage() for accurate CPU time
    // On Windows, we could use GetProcessTimes()
    // For portability, we use a static start time and return elapsed time
    static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START_TIME.get_or_init(std::time::Instant::now);
    start.elapsed().as_secs_f64()
}

/// Get path to current executable
#[no_mangle]
pub extern "C" fn haxe_sys_program_path() -> *mut HaxeString {
    match std::env::current_exe() {
        Ok(path) => {
            let path_str = path.to_string_lossy().into_owned();
            let bytes = path_str.into_bytes();
            let len = bytes.len();
            let cap = bytes.capacity();
            let ptr = bytes.as_ptr() as *mut u8;
            std::mem::forget(bytes);
            Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Execute a shell command and return the exit code
/// Sys.command(cmd: String, args: Array<String>): Int
/// When args is null, cmd is passed directly to the shell
#[no_mangle]
pub extern "C" fn haxe_sys_command(cmd: *const HaxeString) -> i32 {
    unsafe {
        let cmd_str = match haxe_string_to_rust(cmd) {
            Some(s) => s,
            None => return -1,
        };

        // Execute command via shell
        #[cfg(target_os = "windows")]
        let output = std::process::Command::new("cmd")
            .args(["/C", &cmd_str])
            .status();

        #[cfg(not(target_os = "windows"))]
        let output = std::process::Command::new("sh")
            .args(["-c", &cmd_str])
            .status();

        match output {
            Ok(status) => status.code().unwrap_or(-1),
            Err(_) => -1,
        }
    }
}

/// Read a single character from stdin
/// Sys.getChar(echo: Bool): Int
#[no_mangle]
pub extern "C" fn haxe_sys_get_char(echo: bool) -> i32 {
    use std::io::Read;

    let mut buffer = [0u8; 1];
    match std::io::stdin().read_exact(&mut buffer) {
        Ok(_) => {
            if echo {
                print!("{}", buffer[0] as char);
            }
            buffer[0] as i32
        }
        Err(_) => -1,
    }
}

// ============================================================================
// File I/O (sys.io.File)
// ============================================================================

/// Helper to convert HaxeString pointer to Rust String
unsafe fn haxe_string_to_rust(s: *const HaxeString) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let s_ref = &*s;
    if s_ref.ptr.is_null() || s_ref.len == 0 {
        return Some(String::new());
    }
    let slice = std::slice::from_raw_parts(s_ref.ptr, s_ref.len);
    std::str::from_utf8(slice).ok().map(|s| s.to_string())
}

/// Helper to create HaxeString from Rust String
fn rust_string_to_haxe(s: String) -> *mut HaxeString {
    let bytes = s.into_bytes();
    let len = bytes.len();
    let cap = bytes.capacity();
    let ptr = bytes.as_ptr() as *mut u8;
    std::mem::forget(bytes);
    Box::into_raw(Box::new(HaxeString { ptr, len, cap }))
}

/// Read entire file content as string
/// File.getContent(path: String): String
#[no_mangle]
pub extern "C" fn haxe_file_get_content(path: *const HaxeString) -> *mut HaxeString {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => {
                match std::fs::read_to_string(&path_str) {
                    Ok(content) => rust_string_to_haxe(content),
                    Err(e) => {
                        eprintln!("File.getContent error: {} - {}", path_str, e);
                        std::ptr::null_mut()
                    }
                }
            }
            None => std::ptr::null_mut(),
        }
    }
}

/// Write string content to file
/// File.saveContent(path: String, content: String): Void
#[no_mangle]
pub extern "C" fn haxe_file_save_content(path: *const HaxeString, content: *const HaxeString) {
    unsafe {
        let path_str = match haxe_string_to_rust(path) {
            Some(s) => s,
            None => return,
        };
        let content_str = match haxe_string_to_rust(content) {
            Some(s) => s,
            None => String::new(),
        };
        if let Err(e) = std::fs::write(&path_str, content_str) {
            eprintln!("File.saveContent error: {} - {}", path_str, e);
        }
    }
}

/// Copy file from src to dst
/// File.copy(srcPath: String, dstPath: String): Void
#[no_mangle]
pub extern "C" fn haxe_file_copy(src: *const HaxeString, dst: *const HaxeString) {
    unsafe {
        let src_str = match haxe_string_to_rust(src) {
            Some(s) => s,
            None => return,
        };
        let dst_str = match haxe_string_to_rust(dst) {
            Some(s) => s,
            None => return,
        };
        if let Err(e) = std::fs::copy(&src_str, &dst_str) {
            eprintln!("File.copy error: {} -> {} - {}", src_str, dst_str, e);
        }
    }
}

// ============================================================================
// FileSystem (sys.FileSystem)
// ============================================================================

/// Check if file or directory exists
/// FileSystem.exists(path: String): Bool
#[no_mangle]
pub extern "C" fn haxe_filesystem_exists(path: *const HaxeString) -> bool {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => std::path::Path::new(&path_str).exists(),
            None => false,
        }
    }
}

/// Check if path is a directory
/// FileSystem.isDirectory(path: String): Bool
#[no_mangle]
pub extern "C" fn haxe_filesystem_is_directory(path: *const HaxeString) -> bool {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => std::path::Path::new(&path_str).is_dir(),
            None => false,
        }
    }
}

/// Create directory (recursively)
/// FileSystem.createDirectory(path: String): Void
#[no_mangle]
pub extern "C" fn haxe_filesystem_create_directory(path: *const HaxeString) {
    unsafe {
        if let Some(path_str) = haxe_string_to_rust(path) {
            if let Err(e) = std::fs::create_dir_all(&path_str) {
                eprintln!("FileSystem.createDirectory error: {} - {}", path_str, e);
            }
        }
    }
}

/// Delete file
/// FileSystem.deleteFile(path: String): Void
#[no_mangle]
pub extern "C" fn haxe_filesystem_delete_file(path: *const HaxeString) {
    unsafe {
        if let Some(path_str) = haxe_string_to_rust(path) {
            if let Err(e) = std::fs::remove_file(&path_str) {
                eprintln!("FileSystem.deleteFile error: {} - {}", path_str, e);
            }
        }
    }
}

/// Delete directory (must be empty)
/// FileSystem.deleteDirectory(path: String): Void
#[no_mangle]
pub extern "C" fn haxe_filesystem_delete_directory(path: *const HaxeString) {
    unsafe {
        if let Some(path_str) = haxe_string_to_rust(path) {
            if let Err(e) = std::fs::remove_dir(&path_str) {
                eprintln!("FileSystem.deleteDirectory error: {} - {}", path_str, e);
            }
        }
    }
}

/// Rename/move file or directory
/// FileSystem.rename(path: String, newPath: String): Void
#[no_mangle]
pub extern "C" fn haxe_filesystem_rename(path: *const HaxeString, new_path: *const HaxeString) {
    unsafe {
        let path_str = match haxe_string_to_rust(path) {
            Some(s) => s,
            None => return,
        };
        let new_path_str = match haxe_string_to_rust(new_path) {
            Some(s) => s,
            None => return,
        };
        if let Err(e) = std::fs::rename(&path_str, &new_path_str) {
            eprintln!("FileSystem.rename error: {} -> {} - {}", path_str, new_path_str, e);
        }
    }
}

/// Get full/absolute path
/// FileSystem.fullPath(relPath: String): String
#[no_mangle]
pub extern "C" fn haxe_filesystem_full_path(path: *const HaxeString) -> *mut HaxeString {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => {
                match std::fs::canonicalize(&path_str) {
                    Ok(full_path) => rust_string_to_haxe(full_path.to_string_lossy().into_owned()),
                    Err(_) => std::ptr::null_mut(),
                }
            }
            None => std::ptr::null_mut(),
        }
    }
}

/// Get absolute path (doesn't need to exist)
/// FileSystem.absolutePath(relPath: String): String
#[no_mangle]
pub extern "C" fn haxe_filesystem_absolute_path(path: *const HaxeString) -> *mut HaxeString {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => {
                let abs_path = if std::path::Path::new(&path_str).is_absolute() {
                    path_str
                } else {
                    match std::env::current_dir() {
                        Ok(cwd) => cwd.join(&path_str).to_string_lossy().into_owned(),
                        Err(_) => path_str,
                    }
                };
                rust_string_to_haxe(abs_path)
            }
            None => std::ptr::null_mut(),
        }
    }
}

/// FileStat struct - matches Haxe's sys.FileStat typedef
/// All fields are 8 bytes for consistent sizing/boxing
/// Date fields stored as f64 timestamps (seconds since Unix epoch)
#[repr(C)]
pub struct HaxeFileStat {
    pub gid: i64,    // group id
    pub uid: i64,    // user id
    pub atime: f64,  // access time (seconds since epoch)
    pub mtime: f64,  // modification time (seconds since epoch)
    pub ctime: f64,  // creation/change time (seconds since epoch)
    pub size: i64,   // file size in bytes
    pub dev: i64,    // device id
    pub ino: i64,    // inode number
    pub nlink: i64,  // number of hard links
    pub rdev: i64,   // device type (special files)
    pub mode: i64,   // permission bits
}

/// Get file/directory statistics
/// FileSystem.stat(path: String): FileStat
#[no_mangle]
pub extern "C" fn haxe_filesystem_stat(path: *const HaxeString) -> *mut HaxeFileStat {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => {
                match std::fs::metadata(&path_str) {
                    Ok(meta) => {
                        // Convert SystemTime to f64 (seconds since Unix epoch)
                        let to_timestamp = |time: std::io::Result<std::time::SystemTime>| -> f64 {
                            time.ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs_f64())
                                .unwrap_or(0.0)
                        };

                        let stat = Box::new(HaxeFileStat {
                            #[cfg(unix)]
                            gid: {
                                use std::os::unix::fs::MetadataExt;
                                meta.gid() as i64
                            },
                            #[cfg(not(unix))]
                            gid: 0,

                            #[cfg(unix)]
                            uid: {
                                use std::os::unix::fs::MetadataExt;
                                meta.uid() as i64
                            },
                            #[cfg(not(unix))]
                            uid: 0,

                            atime: to_timestamp(meta.accessed()),
                            mtime: to_timestamp(meta.modified()),
                            ctime: to_timestamp(meta.created()),
                            size: meta.len() as i64,

                            #[cfg(unix)]
                            dev: {
                                use std::os::unix::fs::MetadataExt;
                                meta.dev() as i64
                            },
                            #[cfg(not(unix))]
                            dev: 0,

                            #[cfg(unix)]
                            ino: {
                                use std::os::unix::fs::MetadataExt;
                                meta.ino() as i64
                            },
                            #[cfg(not(unix))]
                            ino: 0,

                            #[cfg(unix)]
                            nlink: {
                                use std::os::unix::fs::MetadataExt;
                                meta.nlink() as i64
                            },
                            #[cfg(not(unix))]
                            nlink: 1,

                            #[cfg(unix)]
                            rdev: {
                                use std::os::unix::fs::MetadataExt;
                                meta.rdev() as i64
                            },
                            #[cfg(not(unix))]
                            rdev: 0,

                            #[cfg(unix)]
                            mode: {
                                use std::os::unix::fs::MetadataExt;
                                meta.mode() as i64
                            },
                            #[cfg(not(unix))]
                            mode: if meta.is_dir() { 0o755 } else { 0o644 } as i64,
                        });
                        Box::into_raw(stat)
                    }
                    Err(_) => std::ptr::null_mut(),
                }
            }
            None => std::ptr::null_mut(),
        }
    }
}

/// Check if path is a file (not directory)
/// FileSystem.isFile(path: String): Bool
#[no_mangle]
pub extern "C" fn haxe_filesystem_is_file(path: *const HaxeString) -> bool {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => std::path::Path::new(&path_str).is_file(),
            None => false,
        }
    }
}

/// Read directory contents
/// FileSystem.readDirectory(path: String): Array<String>
#[no_mangle]
pub extern "C" fn haxe_filesystem_read_directory(path: *const HaxeString) -> *mut crate::haxe_array::HaxeArray {
    use crate::haxe_array::{HaxeArray, haxe_array_new, haxe_array_push};

    unsafe {
        let path_str = match haxe_string_to_rust(path) {
            Some(s) => s,
            None => return std::ptr::null_mut(),
        };

        let entries = match std::fs::read_dir(&path_str) {
            Ok(entries) => entries,
            Err(_) => return std::ptr::null_mut(),
        };

        // Allocate array on heap
        let arr = Box::into_raw(Box::new(std::mem::zeroed::<HaxeArray>()));

        // Initialize array with 8-byte element size (pointer to HaxeString)
        haxe_array_new(arr, 8);

        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                // Skip . and ..
                if name == "." || name == ".." {
                    continue;
                }

                let haxe_str = rust_string_to_haxe(name.to_string());
                if !haxe_str.is_null() {
                    // Push pointer to string (pass address of the pointer)
                    let str_ptr = haxe_str as u64;
                    haxe_array_push(arr, &str_ptr as *const u64 as *const u8);
                }
            }
        }

        arr
    }
}

// ============================================================================
// FileInput (sys.io.FileInput) - File reading handle
// ============================================================================
//
// FileInput wraps a Rust File handle for reading operations.
// Extends haxe.io.Input which provides readByte() as the core method.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, BufReader, BufWriter};

/// File input handle for reading
#[repr(C)]
pub struct HaxeFileInput {
    reader: BufReader<File>,
    eof_reached: bool,
}

/// File output handle for writing
#[repr(C)]
pub struct HaxeFileOutput {
    writer: BufWriter<File>,
}

/// FileSeek enum values (matching sys.io.FileSeek)
/// SeekBegin = 0, SeekCur = 1, SeekEnd = 2
const SEEK_BEGIN: i32 = 0;
const SEEK_CUR: i32 = 1;
const SEEK_END: i32 = 2;

/// Open file for reading
/// File.read(path: String, binary: Bool): FileInput
#[no_mangle]
pub extern "C" fn haxe_file_read(path: *const HaxeString, _binary: bool) -> *mut HaxeFileInput {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => {
                match File::open(&path_str) {
                    Ok(file) => {
                        Box::into_raw(Box::new(HaxeFileInput {
                            reader: BufReader::new(file),
                            eof_reached: false,
                        }))
                    }
                    Err(e) => {
                        eprintln!("File.read error: {} - {}", path_str, e);
                        std::ptr::null_mut()
                    }
                }
            }
            None => std::ptr::null_mut(),
        }
    }
}

/// Open file for writing (creates or truncates)
/// File.write(path: String, binary: Bool): FileOutput
#[no_mangle]
pub extern "C" fn haxe_file_write(path: *const HaxeString, _binary: bool) -> *mut HaxeFileOutput {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => {
                match File::create(&path_str) {
                    Ok(file) => {
                        Box::into_raw(Box::new(HaxeFileOutput {
                            writer: BufWriter::new(file),
                        }))
                    }
                    Err(e) => {
                        eprintln!("File.write error: {} - {}", path_str, e);
                        std::ptr::null_mut()
                    }
                }
            }
            None => std::ptr::null_mut(),
        }
    }
}

/// Open file for appending
/// File.append(path: String, binary: Bool): FileOutput
#[no_mangle]
pub extern "C" fn haxe_file_append(path: *const HaxeString, _binary: bool) -> *mut HaxeFileOutput {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => {
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path_str)
                {
                    Ok(file) => {
                        Box::into_raw(Box::new(HaxeFileOutput {
                            writer: BufWriter::new(file),
                        }))
                    }
                    Err(e) => {
                        eprintln!("File.append error: {} - {}", path_str, e);
                        std::ptr::null_mut()
                    }
                }
            }
            None => std::ptr::null_mut(),
        }
    }
}

/// Open file for updating (read/write, seek anywhere)
/// File.update(path: String, binary: Bool): FileOutput
#[no_mangle]
pub extern "C" fn haxe_file_update(path: *const HaxeString, _binary: bool) -> *mut HaxeFileOutput {
    unsafe {
        match haxe_string_to_rust(path) {
            Some(path_str) => {
                match std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&path_str)
                {
                    Ok(file) => {
                        Box::into_raw(Box::new(HaxeFileOutput {
                            writer: BufWriter::new(file),
                        }))
                    }
                    Err(e) => {
                        eprintln!("File.update error: {} - {}", path_str, e);
                        std::ptr::null_mut()
                    }
                }
            }
            None => std::ptr::null_mut(),
        }
    }
}

// ============================================================================
// FileInput methods (reading)
// ============================================================================

/// Read one byte from FileInput
/// FileInput.readByte(): Int
#[no_mangle]
pub extern "C" fn haxe_fileinput_read_byte(handle: *mut HaxeFileInput) -> i32 {
    if handle.is_null() {
        return -1;
    }
    unsafe {
        let input = &mut *handle;
        let mut buf = [0u8; 1];
        match input.reader.read(&mut buf) {
            Ok(0) => {
                input.eof_reached = true;
                -1 // EOF
            }
            Ok(_) => buf[0] as i32,
            Err(_) => {
                input.eof_reached = true;
                -1
            }
        }
    }
}

/// Read multiple bytes into buffer
/// Returns actual bytes read
#[no_mangle]
pub extern "C" fn haxe_fileinput_read_bytes(handle: *mut HaxeFileInput, buf: *mut u8, len: i32) -> i32 {
    if handle.is_null() || buf.is_null() || len <= 0 {
        return 0;
    }
    unsafe {
        let input = &mut *handle;
        let slice = std::slice::from_raw_parts_mut(buf, len as usize);
        match input.reader.read(slice) {
            Ok(0) => {
                input.eof_reached = true;
                0
            }
            Ok(n) => n as i32,
            Err(_) => {
                input.eof_reached = true;
                0
            }
        }
    }
}

/// Seek to position in FileInput
/// FileInput.seek(p: Int, pos: FileSeek): Void
#[no_mangle]
pub extern "C" fn haxe_fileinput_seek(handle: *mut HaxeFileInput, p: i32, pos: i32) {
    if handle.is_null() {
        return;
    }
    unsafe {
        let input = &mut *handle;
        let seek_pos = match pos {
            SEEK_BEGIN => SeekFrom::Start(p as u64),
            SEEK_CUR => SeekFrom::Current(p as i64),
            SEEK_END => SeekFrom::End(p as i64),
            _ => return,
        };
        let _ = input.reader.seek(seek_pos);
        input.eof_reached = false; // Reset EOF on seek
    }
}

/// Get current position in FileInput
/// FileInput.tell(): Int
#[no_mangle]
pub extern "C" fn haxe_fileinput_tell(handle: *mut HaxeFileInput) -> i32 {
    if handle.is_null() {
        return 0;
    }
    unsafe {
        let input = &mut *handle;
        match input.reader.stream_position() {
            Ok(pos) => pos as i32,
            Err(_) => 0,
        }
    }
}

/// Check if EOF reached
/// FileInput.eof(): Bool
#[no_mangle]
pub extern "C" fn haxe_fileinput_eof(handle: *mut HaxeFileInput) -> bool {
    if handle.is_null() {
        return true;
    }
    unsafe {
        (*handle).eof_reached
    }
}

/// Close FileInput
/// FileInput.close(): Void
#[no_mangle]
pub extern "C" fn haxe_fileinput_close(handle: *mut HaxeFileInput) {
    if handle.is_null() {
        return;
    }
    unsafe {
        // Drop the Box, which closes the file
        let _ = Box::from_raw(handle);
    }
}

// ============================================================================
// FileOutput methods (writing)
// ============================================================================

/// Write one byte to FileOutput
/// FileOutput.writeByte(c: Int): Void
#[no_mangle]
pub extern "C" fn haxe_fileoutput_write_byte(handle: *mut HaxeFileOutput, c: i32) {
    if handle.is_null() {
        return;
    }
    unsafe {
        let output = &mut *handle;
        let _ = output.writer.write(&[c as u8]);
    }
}

/// Write multiple bytes from buffer
/// Returns actual bytes written
#[no_mangle]
pub extern "C" fn haxe_fileoutput_write_bytes(handle: *mut HaxeFileOutput, buf: *const u8, len: i32) -> i32 {
    if handle.is_null() || buf.is_null() || len <= 0 {
        return 0;
    }
    unsafe {
        let output = &mut *handle;
        let slice = std::slice::from_raw_parts(buf, len as usize);
        match output.writer.write(slice) {
            Ok(n) => n as i32,
            Err(_) => 0,
        }
    }
}

/// Seek to position in FileOutput
/// FileOutput.seek(p: Int, pos: FileSeek): Void
#[no_mangle]
pub extern "C" fn haxe_fileoutput_seek(handle: *mut HaxeFileOutput, p: i32, pos: i32) {
    if handle.is_null() {
        return;
    }
    unsafe {
        let output = &mut *handle;
        // Flush before seeking
        let _ = output.writer.flush();
        let seek_pos = match pos {
            SEEK_BEGIN => SeekFrom::Start(p as u64),
            SEEK_CUR => SeekFrom::Current(p as i64),
            SEEK_END => SeekFrom::End(p as i64),
            _ => return,
        };
        let _ = output.writer.seek(seek_pos);
    }
}

/// Get current position in FileOutput
/// FileOutput.tell(): Int
#[no_mangle]
pub extern "C" fn haxe_fileoutput_tell(handle: *mut HaxeFileOutput) -> i32 {
    if handle.is_null() {
        return 0;
    }
    unsafe {
        let output = &mut *handle;
        match output.writer.stream_position() {
            Ok(pos) => pos as i32,
            Err(_) => 0,
        }
    }
}

/// Flush FileOutput buffer
/// FileOutput.flush(): Void
#[no_mangle]
pub extern "C" fn haxe_fileoutput_flush(handle: *mut HaxeFileOutput) {
    if handle.is_null() {
        return;
    }
    unsafe {
        let output = &mut *handle;
        let _ = output.writer.flush();
    }
}

/// Close FileOutput
/// FileOutput.close(): Void
#[no_mangle]
pub extern "C" fn haxe_fileoutput_close(handle: *mut HaxeFileOutput) {
    if handle.is_null() {
        return;
    }
    unsafe {
        // Flush and drop
        let mut output = Box::from_raw(handle);
        let _ = output.writer.flush();
        // Box drops here, closing the file
    }
}

// ============================================================================
// StringMap<T> (haxe.ds.StringMap)
// ============================================================================
//
// High-performance StringMap with inline value storage.
// Values are stored as raw 64-bit values (u64) - no boxing, no heap allocation per value.
// Type is known at compile time; the runtime stores raw bits.
//
// For primitives (Int, Float, Bool): value is stored directly as bits
// For pointers (String, objects): pointer value is stored as u64
//
// This gives us:
// - No heap allocation per value (values inline in HashMap)
// - No type tags (type known at compile time)
// - Cache-friendly layout
// - Zero-cost abstraction over HashMap<String, u64>

use std::collections::HashMap;

/// High-performance StringMap with inline 8-byte value storage
/// Values are stored as raw u64 bits - no boxing overhead
#[repr(C)]
pub struct HaxeStringMap {
    map: HashMap<String, u64>,
}

/// Create a new StringMap
#[no_mangle]
pub extern "C" fn haxe_stringmap_new() -> *mut HaxeStringMap {
    Box::into_raw(Box::new(HaxeStringMap {
        map: HashMap::new(),
    }))
}

/// Set a value in the StringMap
/// Value is passed as raw u64 bits (compiler handles type conversion)
#[no_mangle]
pub extern "C" fn haxe_stringmap_set(map_ptr: *mut HaxeStringMap, key: *const HaxeString, value: u64) {
    if map_ptr.is_null() {
        return;
    }
    unsafe {
        let map = &mut *map_ptr;
        if let Some(key_str) = haxe_string_to_rust(key) {
            map.map.insert(key_str, value);
        }
    }
}

/// Get a value from the StringMap
/// Returns raw u64 bits (compiler handles type conversion)
/// Returns 0 if key doesn't exist (caller should use exists() to distinguish)
#[no_mangle]
pub extern "C" fn haxe_stringmap_get(map_ptr: *mut HaxeStringMap, key: *const HaxeString) -> u64 {
    if map_ptr.is_null() {
        return 0;
    }
    unsafe {
        let map = &*map_ptr;
        if let Some(key_str) = haxe_string_to_rust(key) {
            map.map.get(&key_str).copied().unwrap_or(0)
        } else {
            0
        }
    }
}

/// Check if a key exists in the StringMap
#[no_mangle]
pub extern "C" fn haxe_stringmap_exists(map_ptr: *mut HaxeStringMap, key: *const HaxeString) -> bool {
    if map_ptr.is_null() {
        return false;
    }
    unsafe {
        let map = &*map_ptr;
        if let Some(key_str) = haxe_string_to_rust(key) {
            map.map.contains_key(&key_str)
        } else {
            false
        }
    }
}

/// Remove a key from the StringMap
/// Returns true if the key existed and was removed
#[no_mangle]
pub extern "C" fn haxe_stringmap_remove(map_ptr: *mut HaxeStringMap, key: *const HaxeString) -> bool {
    if map_ptr.is_null() {
        return false;
    }
    unsafe {
        let map = &mut *map_ptr;
        if let Some(key_str) = haxe_string_to_rust(key) {
            map.map.remove(&key_str).is_some()
        } else {
            false
        }
    }
}

/// Clear all entries from the StringMap
#[no_mangle]
pub extern "C" fn haxe_stringmap_clear(map_ptr: *mut HaxeStringMap) {
    if map_ptr.is_null() {
        return;
    }
    unsafe {
        let map = &mut *map_ptr;
        map.map.clear();
    }
}

/// Get the number of entries in the map
#[no_mangle]
pub extern "C" fn haxe_stringmap_count(map_ptr: *mut HaxeStringMap) -> i64 {
    if map_ptr.is_null() {
        return 0;
    }
    unsafe {
        let map = &*map_ptr;
        map.map.len() as i64
    }
}

/// Get all keys as an array
/// Returns pointer to array of HaxeString pointers, sets out_len to count
#[no_mangle]
pub extern "C" fn haxe_stringmap_keys(map_ptr: *mut HaxeStringMap, out_len: *mut i64) -> *mut *mut HaxeString {
    if map_ptr.is_null() || out_len.is_null() {
        if !out_len.is_null() {
            unsafe { *out_len = 0; }
        }
        return std::ptr::null_mut();
    }
    unsafe {
        let map = &*map_ptr;
        let keys: Vec<*mut HaxeString> = map.map.keys()
            .map(|k| rust_string_to_haxe(k.clone()))
            .collect();
        *out_len = keys.len() as i64;
        Box::into_raw(keys.into_boxed_slice()) as *mut *mut HaxeString
    }
}

/// Convert StringMap to string representation
#[no_mangle]
pub extern "C" fn haxe_stringmap_to_string(map_ptr: *mut HaxeStringMap) -> *mut HaxeString {
    if map_ptr.is_null() {
        return rust_string_to_haxe("{}".to_string());
    }
    unsafe {
        let map = &*map_ptr;
        let entries: Vec<String> = map.map.iter()
            .map(|(k, v)| format!("{} => {}", k, v))
            .collect();
        let result = format!("{{{}}}", entries.join(", "));
        rust_string_to_haxe(result)
    }
}

// ============================================================================
// IntMap<T> (haxe.ds.IntMap)
// ============================================================================
//
// High-performance IntMap with inline value storage.
// Same design as StringMap - values stored as raw u64 bits.

/// High-performance IntMap with inline 8-byte value storage
#[repr(C)]
pub struct HaxeIntMap {
    map: HashMap<i64, u64>,
}

/// Create a new IntMap
#[no_mangle]
pub extern "C" fn haxe_intmap_new() -> *mut HaxeIntMap {
    Box::into_raw(Box::new(HaxeIntMap {
        map: HashMap::new(),
    }))
}

/// Set a value in the IntMap
/// Value is passed as raw u64 bits
#[no_mangle]
pub extern "C" fn haxe_intmap_set(map_ptr: *mut HaxeIntMap, key: i64, value: u64) {
    if map_ptr.is_null() {
        return;
    }
    unsafe {
        let map = &mut *map_ptr;
        map.map.insert(key, value);
    }
}

/// Get a value from the IntMap
/// Returns raw u64 bits, 0 if key doesn't exist
#[no_mangle]
pub extern "C" fn haxe_intmap_get(map_ptr: *mut HaxeIntMap, key: i64) -> u64 {
    if map_ptr.is_null() {
        return 0;
    }
    unsafe {
        let map = &*map_ptr;
        map.map.get(&key).copied().unwrap_or(0)
    }
}

/// Check if a key exists in the IntMap
#[no_mangle]
pub extern "C" fn haxe_intmap_exists(map_ptr: *mut HaxeIntMap, key: i64) -> bool {
    if map_ptr.is_null() {
        return false;
    }
    unsafe {
        let map = &*map_ptr;
        map.map.contains_key(&key)
    }
}

/// Remove a key from the IntMap
/// Returns true if the key existed and was removed
#[no_mangle]
pub extern "C" fn haxe_intmap_remove(map_ptr: *mut HaxeIntMap, key: i64) -> bool {
    if map_ptr.is_null() {
        return false;
    }
    unsafe {
        let map = &mut *map_ptr;
        map.map.remove(&key).is_some()
    }
}

/// Clear all entries from the IntMap
#[no_mangle]
pub extern "C" fn haxe_intmap_clear(map_ptr: *mut HaxeIntMap) {
    if map_ptr.is_null() {
        return;
    }
    unsafe {
        let map = &mut *map_ptr;
        map.map.clear();
    }
}

/// Get the number of entries in the map
#[no_mangle]
pub extern "C" fn haxe_intmap_count(map_ptr: *mut HaxeIntMap) -> i64 {
    if map_ptr.is_null() {
        return 0;
    }
    unsafe {
        let map = &*map_ptr;
        map.map.len() as i64
    }
}

/// Get all keys as an array
/// Returns pointer to array of i64, sets out_len to count
#[no_mangle]
pub extern "C" fn haxe_intmap_keys(map_ptr: *mut HaxeIntMap, out_len: *mut i64) -> *mut i64 {
    if map_ptr.is_null() || out_len.is_null() {
        if !out_len.is_null() {
            unsafe { *out_len = 0; }
        }
        return std::ptr::null_mut();
    }
    unsafe {
        let map = &*map_ptr;
        let keys: Vec<i64> = map.map.keys().copied().collect();
        *out_len = keys.len() as i64;
        Box::into_raw(keys.into_boxed_slice()) as *mut i64
    }
}

/// Convert IntMap to string representation
#[no_mangle]
pub extern "C" fn haxe_intmap_to_string(map_ptr: *mut HaxeIntMap) -> *mut HaxeString {
    if map_ptr.is_null() {
        return rust_string_to_haxe("{}".to_string());
    }
    unsafe {
        let map = &*map_ptr;
        let entries: Vec<String> = map.map.iter()
            .map(|(k, v)| format!("{} => {}", k, v))
            .collect();
        let result = format!("{{{}}}", entries.join(", "));
        rust_string_to_haxe(result)
    }
}
