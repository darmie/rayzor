//! Safety validation and error reporting utilities
//!
//! This module provides centralized safety violation tracking and reporting
//! for the Rayzor runtime. It helps identify memory safety issues during
//! development and can be optionally enabled in production.

use std::sync::atomic::{AtomicU64, Ordering};

/// Global counter for safety violations
static SAFETY_VIOLATION_COUNT: AtomicU64 = AtomicU64::new(0);

/// Safety violation types
#[derive(Debug, Clone, Copy)]
pub enum SafetyViolation {
    /// NULL pointer where valid pointer expected
    NullPointer,
    /// Invalid magic number in handle
    InvalidMagicNumber,
    /// Attempt to join thread twice
    DoubleJoin,
    /// Pointer has incorrect alignment
    MisalignedPointer,
    /// Use of freed/invalid handle
    UseAfterFree,
    /// Invalid closure environment pointer
    InvalidEnvironment,
}

/// Report a safety violation with context
///
/// This function logs the violation and increments a global counter.
/// In debug builds or with `panic-on-safety-violation` feature, it will panic.
pub fn report_violation(violation: SafetyViolation, function: &str, details: &str) {
    let count = SAFETY_VIOLATION_COUNT.fetch_add(1, Ordering::SeqCst);

    eprintln!("\n═══════════════════════════════════════════════════════════════");
    eprintln!(
        "[SAFETY VIOLATION #{:04}] {:?} in {}",
        count + 1,
        violation,
        function
    );
    eprintln!("Details: {}", details);
    eprintln!("═══════════════════════════════════════════════════════════════\n");

    #[cfg(feature = "panic-on-safety-violation")]
    {
        panic!(
            "Safety violation detected: {:?} in {} - {}",
            violation, function, details
        );
    }
}

/// Get total count of safety violations
pub fn violation_count() -> u64 {
    SAFETY_VIOLATION_COUNT.load(Ordering::SeqCst)
}

/// Validate heap pointer (alignment and NULL check)
///
/// # Safety
/// This function checks basic pointer validity but cannot guarantee
/// the pointer points to valid allocated memory.
pub unsafe fn validate_heap_pointer(ptr: *const u8, name: &str) -> Result<(), &'static str> {
    if ptr.is_null() {
        return Ok(()); // NULL is valid for no-capture closures
    }

    // Check alignment (heap pointers should be at least 8-byte aligned on 64-bit)
    if ptr as usize % 8 != 0 {
        report_violation(
            SafetyViolation::MisalignedPointer,
            "validate_heap_pointer",
            &format!("{}: pointer {:?} is not 8-byte aligned", name, ptr),
        );
        return Err("Misaligned heap pointer");
    }

    Ok(())
}

/// Validate pointer alignment for a specific type
///
/// # Safety
/// Only checks alignment, not validity of memory
pub unsafe fn validate_alignment<T>(ptr: *const u8, name: &str) -> Result<(), &'static str> {
    let align = std::mem::align_of::<T>();

    if ptr as usize % align != 0 {
        report_violation(
            SafetyViolation::MisalignedPointer,
            "validate_alignment",
            &format!("{}: pointer {:?} not aligned to {} bytes", name, ptr, align),
        );
        return Err("Misaligned pointer");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_pointer_is_valid() {
        unsafe {
            assert!(validate_heap_pointer(std::ptr::null(), "test").is_ok());
        }
    }

    #[test]
    fn test_aligned_pointer_is_valid() {
        let value: u64 = 42;
        let ptr = &value as *const u64 as *const u8;
        unsafe {
            assert!(validate_heap_pointer(ptr, "test").is_ok());
        }
    }

    #[test]
    fn test_misaligned_pointer_is_invalid() {
        // Create a misaligned pointer (offset by 1 byte)
        let value: u64 = 42;
        let ptr = &value as *const u64 as *const u8;
        let misaligned = unsafe { ptr.add(1) };

        unsafe {
            assert!(validate_heap_pointer(misaligned, "test").is_err());
        }
    }
}
