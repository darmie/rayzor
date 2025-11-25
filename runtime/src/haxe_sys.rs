//! Haxe Sys runtime implementation
//!
//! System and I/O functions

use std::io::{self, Write};

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
// ============================================================================

/// String representation (ptr + len) for FFI
#[repr(C)]
pub struct HaxeString {
    pub ptr: *const u8,
    pub len: usize,
}

/// Convert Int to String
#[no_mangle]
pub extern "C" fn haxe_string_from_int(value: i64) -> HaxeString {
    let s = value.to_string();
    let boxed = s.into_boxed_str();
    let ptr = boxed.as_ptr();
    let len = boxed.len();
    std::mem::forget(boxed); // Leak the string (TODO: proper memory management)
    HaxeString { ptr, len }
}

/// Convert Float to String
#[no_mangle]
pub extern "C" fn haxe_string_from_float(value: f64) -> HaxeString {
    let s = value.to_string();
    let boxed = s.into_boxed_str();
    let ptr = boxed.as_ptr();
    let len = boxed.len();
    std::mem::forget(boxed);
    HaxeString { ptr, len }
}

/// Convert Bool to String
#[no_mangle]
pub extern "C" fn haxe_string_from_bool(value: bool) -> HaxeString {
    let s = if value { "true" } else { "false" };
    HaxeString {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

/// Convert String to String (identity, but normalizes representation)
#[no_mangle]
pub extern "C" fn haxe_string_from_string(ptr: *const u8, len: usize) -> HaxeString {
    HaxeString { ptr, len }
}

/// Convert null to String
#[no_mangle]
pub extern "C" fn haxe_string_from_null() -> HaxeString {
    let s = "null";
    HaxeString {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

/// Create a string literal from embedded bytes
/// Returns a pointer to a heap-allocated HaxeString struct
/// The bytes are NOT copied - they must remain valid (e.g., in JIT code section)
#[no_mangle]
pub extern "C" fn haxe_string_literal(ptr: *const u8, len: usize) -> *mut HaxeString {
    let boxed = Box::new(HaxeString { ptr, len });
    Box::into_raw(boxed)
}

/// Convert string to uppercase (wrapper returning pointer)
/// Takes pointer to input string, returns pointer to new heap-allocated uppercase string
#[no_mangle]
pub extern "C" fn haxe_string_upper(s: *const HaxeString) -> *mut HaxeString {
    if s.is_null() {
        return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null(), len: 0 }));
    }
    unsafe {
        let s_ref = &*s;
        if s_ref.ptr.is_null() || s_ref.len == 0 {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null(), len: 0 }));
        }
        let slice = std::slice::from_raw_parts(s_ref.ptr, s_ref.len);
        if let Ok(rust_str) = std::str::from_utf8(slice) {
            let upper = rust_str.to_uppercase();
            let bytes = upper.into_bytes().into_boxed_slice();
            let len = bytes.len();
            let ptr = Box::into_raw(bytes) as *const u8;
            Box::into_raw(Box::new(HaxeString { ptr, len }))
        } else {
            // Invalid UTF-8, return copy of original
            let mut new_bytes = Vec::with_capacity(s_ref.len);
            new_bytes.extend_from_slice(slice);
            let bytes = new_bytes.into_boxed_slice();
            let len = bytes.len();
            let ptr = Box::into_raw(bytes) as *const u8;
            Box::into_raw(Box::new(HaxeString { ptr, len }))
        }
    }
}

/// Convert string to lowercase (wrapper returning pointer)
/// Takes pointer to input string, returns pointer to new heap-allocated lowercase string
#[no_mangle]
pub extern "C" fn haxe_string_lower(s: *const HaxeString) -> *mut HaxeString {
    if s.is_null() {
        return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null(), len: 0 }));
    }
    unsafe {
        let s_ref = &*s;
        if s_ref.ptr.is_null() || s_ref.len == 0 {
            return Box::into_raw(Box::new(HaxeString { ptr: std::ptr::null(), len: 0 }));
        }
        let slice = std::slice::from_raw_parts(s_ref.ptr, s_ref.len);
        if let Ok(rust_str) = std::str::from_utf8(slice) {
            let lower = rust_str.to_lowercase();
            let bytes = lower.into_bytes().into_boxed_slice();
            let len = bytes.len();
            let ptr = Box::into_raw(bytes) as *const u8;
            Box::into_raw(Box::new(HaxeString { ptr, len }))
        } else {
            // Invalid UTF-8, return copy of original
            let mut new_bytes = Vec::with_capacity(s_ref.len);
            new_bytes.extend_from_slice(slice);
            let bytes = new_bytes.into_boxed_slice();
            let len = bytes.len();
            let ptr = Box::into_raw(bytes) as *const u8;
            Box::into_raw(Box::new(HaxeString { ptr, len }))
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
