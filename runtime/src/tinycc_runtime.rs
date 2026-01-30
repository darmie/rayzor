//! TinyCC runtime API for Haxe — wraps libtcc FFI for runtime C compilation.
//!
//! These functions are registered as runtime symbols and called from compiled
//! Haxe code via the `rayzor.runtime.CC` extern class.
//!
//! TCC is statically linked into the compiler crate via build.rs. Since the
//! compiler and runtime link into the same binary, these extern declarations
//! resolve at link time.

use std::ffi::CString;
use std::ptr;

use crate::haxe_string::HaxeString;

// ============================================================================
// TCC FFI declarations (resolved from compiler's statically linked libtcc)
// ============================================================================

#[allow(non_camel_case_types)]
type TCCState = std::ffi::c_void;

extern "C" {
    fn tcc_new() -> *mut TCCState;
    fn tcc_delete(s: *mut TCCState);
    fn tcc_set_output_type(s: *mut TCCState, output_type: i32) -> i32;
    fn tcc_compile_string(s: *mut TCCState, buf: *const i8) -> i32;
    fn tcc_add_symbol(s: *mut TCCState, name: *const i8, val: *const std::ffi::c_void) -> i32;
    fn tcc_relocate(s: *mut TCCState) -> i32;
    fn tcc_get_symbol(s: *mut TCCState, name: *const i8) -> *mut std::ffi::c_void;
}

const TCC_OUTPUT_MEMORY: i32 = 1;

// ============================================================================
// Helper: extract a null-terminated CString from a HaxeString pointer
// ============================================================================

unsafe fn haxe_string_to_cstring(s: *const HaxeString) -> Option<CString> {
    if s.is_null() {
        return None;
    }
    let hs = &*s;
    if hs.ptr.is_null() || hs.len == 0 {
        return Some(CString::new("").unwrap());
    }
    let slice = std::slice::from_raw_parts(hs.ptr, hs.len);
    CString::new(slice).ok()
}

// ============================================================================
// Runtime API functions (called from Haxe via CC extern class)
// ============================================================================

/// Create a new TCC compilation context with output type set to memory.
/// Returns an opaque pointer to TCCState.
#[no_mangle]
pub extern "C" fn rayzor_tcc_create() -> *mut TCCState {
    unsafe {
        let state = tcc_new();
        if state.is_null() {
            return ptr::null_mut();
        }
        tcc_set_output_type(state, TCC_OUTPUT_MEMORY);
        state
    }
}

/// Compile a C source string.
/// Takes the TCC state and a HaxeString pointer to the source code.
/// Returns 1 on success, 0 on failure.
#[no_mangle]
pub extern "C" fn rayzor_tcc_compile(state: *mut TCCState, code: *const HaxeString) -> i32 {
    if state.is_null() {
        return 0;
    }
    unsafe {
        let c_code = match haxe_string_to_cstring(code) {
            Some(s) => s,
            None => return 0,
        };
        let ret = tcc_compile_string(state, c_code.as_ptr());
        if ret < 0 {
            0
        } else {
            1
        }
    }
}

/// Register a symbol (name → value) in the TCC context.
/// The value is an i64 that C code can reference via `extern`.
/// Takes the TCC state, a HaxeString pointer to the name, and the raw value.
#[no_mangle]
pub extern "C" fn rayzor_tcc_add_symbol(state: *mut TCCState, name: *const HaxeString, value: i64) {
    if state.is_null() {
        return;
    }
    unsafe {
        let c_name = match haxe_string_to_cstring(name) {
            Some(s) => s,
            None => return,
        };
        tcc_add_symbol(state, c_name.as_ptr(), value as *const std::ffi::c_void);
    }
}

/// Relocate all compiled code into executable memory.
/// Must be called after all compile() and addSymbol() calls.
/// Returns 1 on success, 0 on failure.
#[no_mangle]
pub extern "C" fn rayzor_tcc_relocate(state: *mut TCCState) -> i32 {
    if state.is_null() {
        return 0;
    }
    unsafe {
        let ret = tcc_relocate(state);
        if ret < 0 {
            0
        } else {
            1
        }
    }
}

/// Get a symbol address by name after relocation.
/// Returns the address as i64 (0 if not found).
#[no_mangle]
pub extern "C" fn rayzor_tcc_get_symbol(state: *mut TCCState, name: *const HaxeString) -> i64 {
    if state.is_null() {
        return 0;
    }
    unsafe {
        let c_name = match haxe_string_to_cstring(name) {
            Some(s) => s,
            None => return 0,
        };
        let sym = tcc_get_symbol(state, c_name.as_ptr());
        sym as i64
    }
}

/// Free the TCC compilation context.
/// Note: relocated code memory is intentionally leaked (JIT pattern).
#[no_mangle]
pub extern "C" fn rayzor_tcc_delete(state: *mut TCCState) {
    if state.is_null() {
        return;
    }
    unsafe {
        tcc_delete(state);
    }
}
