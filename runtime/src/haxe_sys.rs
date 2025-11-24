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
