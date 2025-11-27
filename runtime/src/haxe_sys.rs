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

/// Trace any value (fallback for Dynamic type)
/// For now, just prints the raw i64 value until we have Std.string()
#[no_mangle]
pub extern "C" fn haxe_trace_any(value: i64) {
    // Without type information, we can only print the raw value
    // TODO: Once Std.string() is implemented, use that instead
    println!("{}", value);
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
