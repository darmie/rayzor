//! Logging configuration for the rayzor compiler
//!
//! This module provides utilities for initializing and configuring logging
//! using the `log` and `env_logger` crates.
//!
//! # Usage
//!
//! ```rust,ignore
//! use compiler::logging;
//!
//! // Initialize with default level (Warn)
//! logging::init();
//!
//! // Or initialize from RUST_LOG environment variable
//! logging::init_from_env();
//!
//! // Or initialize with a specific level
//! logging::init_with_level(log::LevelFilter::Debug);
//! ```
//!
//! # Log Levels
//!
//! The rayzor compiler uses log levels as follows:
//!
//! - `error!` - Actual errors that should always be shown
//! - `warn!` - Warnings that may indicate problems
//! - `info!` - High-level progress (compilation phases)
//! - `debug!` - Detailed debugging (function lowering)
//! - `trace!` - Very verbose (expression lowering, type details)
//!
//! # Environment Variable
//!
//! Set `RUST_LOG` to control logging at runtime:
//!
//! ```bash
//! RUST_LOG=warn ./rayzor compile main.hx  # Default, quiet output
//! RUST_LOG=info ./rayzor compile main.hx  # Show compilation phases
//! RUST_LOG=debug ./rayzor compile main.hx # Detailed debugging
//! RUST_LOG=trace ./rayzor compile main.hx # Very verbose
//! ```
//!
//! You can also filter by module:
//!
//! ```bash
//! RUST_LOG=compiler::ir::hir_to_mir=debug ./rayzor compile main.hx
//! RUST_LOG=compiler::codegen=trace ./rayzor compile main.hx
//! ```

use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;
use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize logging with sensible defaults (Warn level).
///
/// This only initializes once; subsequent calls are no-ops.
/// Use this in binaries and test entry points.
pub fn init() {
    init_with_level(LevelFilter::Warn);
}

/// Initialize logging with a specific level.
///
/// This only initializes once; subsequent calls are no-ops.
pub fn init_with_level(level: LevelFilter) {
    INIT.call_once(|| {
        Builder::new()
            .filter_level(level)
            .format(|buf, record| {
                writeln!(
                    buf,
                    "[{:5}] {}:{} - {}",
                    record.level(),
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.args()
                )
            })
            .init();
    });
}

/// Initialize logging from the RUST_LOG environment variable.
///
/// If RUST_LOG is not set, defaults to Warn level.
/// This only initializes once; subsequent calls are no-ops.
pub fn init_from_env() {
    INIT.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    });
}

/// Initialize logging for tests.
///
/// Uses test-friendly output format and suppresses most output unless
/// RUST_LOG is explicitly set.
///
/// # Usage in Tests
///
/// ```rust,ignore
/// #[test]
/// fn test_something() {
///     compiler::logging::init_test();
///     // Test code...
/// }
/// ```
pub fn init_test() {
    // try_init() doesn't panic if already initialized
    let _ = env_logger::builder()
        .filter_level(LevelFilter::Warn)
        .is_test(true)
        .try_init();
}

/// Check if logging has been initialized.
///
/// Note: This doesn't guarantee logs will be output - only that init was called.
pub fn is_initialized() -> bool {
    // We can't easily check if env_logger is initialized,
    // but we can track our own initialization
    INIT.is_completed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_is_idempotent() {
        // Multiple calls should not panic
        init_test();
        init_test();
        init_test();
    }

    #[test]
    fn test_log_levels() {
        init_test();

        // These should not panic
        log::error!("Test error message");
        log::warn!("Test warning message");
        log::info!("Test info message");
        log::debug!("Test debug message");
        log::trace!("Test trace message");
    }
}
