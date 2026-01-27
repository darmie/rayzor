#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unreachable_patterns,
    unused_mut,
    unused_assignments,
    unused_parens
)]
#![allow(
    clippy::single_component_path_imports,
    clippy::for_kv_map,
    clippy::explicit_auto_deref
)]
#![allow(
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::useless_vec,
    clippy::field_reassign_with_default
)]
#![allow(
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]
#![allow(
    clippy::empty_line_after_doc_comments,
    clippy::useless_format,
    clippy::clone_on_copy
)]
/// Test program-level safety mode detection and validation
/// Tests @:safety(strict=true) and @:safety(strict=false) behavior
use compiler::pipeline::compile_haxe_source;

fn main() {
    println!("=== Program Safety Modes Test ===\n");

    test_gc_mode();
    test_non_strict_mode();
    test_strict_mode_valid();
    test_strict_mode_invalid();

    println!("\n✅ All program safety mode tests completed!");
}

/// Test 1: No @:safety on Main = GC mode (default)
fn test_gc_mode() {
    println!("Test 1: GC Mode (no @:safety on Main)");
    let source = r#"
        class Main {
            static function main() {
                var x = new ManagedClass();
            }
        }

        class ManagedClass {
            var data: String;
        }
    "#;

    let result = compile_haxe_source(source);

    if let Some(typed_file) = result.typed_files.first() {
        let safety_mode = typed_file.get_program_safety_mode();
        assert!(
            safety_mode.is_none(),
            "Expected GC mode (None), got {:?}",
            safety_mode
        );
        println!("  ✅ Program uses GC (no manual memory management)");
    } else {
        println!("  ❌ Failed to compile");
    }
}

/// Test 2: @:safety (non-strict) on Main = allows unannotated classes
fn test_non_strict_mode() {
    println!("\nTest 2: Non-Strict Safety Mode (@:safety or @:safety(false))");
    let source = r#"
        @:safety(false)
        class Main {
            static function main() {
                var safe = new SafeClass();
                var legacy = new LegacyClass();  // Should be allowed (auto-wrapped in Rc)
            }
        }

        @:safety
        class SafeClass {
            var data: Int;
        }

        class LegacyClass {
            var data: String;  // No @:safety, should be auto-wrapped
        }
    "#;

    let result = compile_haxe_source(source);

    if let Some(typed_file) = result.typed_files.first() {
        let safety_mode = typed_file.get_program_safety_mode();
        match safety_mode {
            Some(compiler::tast::SafetyMode::NonStrict) => {
                println!("  ✅ Program uses non-strict manual memory mode");
                println!("  ✅ Unannotated classes allowed (will be wrapped in Rc)");
            }
            other => {
                println!("  ❌ Expected NonStrict mode, got {:?}", other);
            }
        }

        // Should have no errors - non-strict allows mixing
        if result.errors.is_empty() {
            println!("  ✅ No compilation errors (unannotated classes accepted)");
        } else {
            println!(
                "  ⚠️  Got {} errors (may be type errors, not safety errors)",
                result.errors.len()
            );
            for err in &result.errors {
                println!("      {}", err.message);
            }
        }
    } else {
        println!("  ❌ Failed to compile");
    }
}

/// Test 3: @:safety(true) with all classes annotated = valid
fn test_strict_mode_valid() {
    println!("\nTest 3: Strict Safety Mode with All Classes Annotated");
    let source = r#"
        @:safety(true)
        class Main {
            static function main() {
                var x = new SafeClass();
                var y = new AnotherSafeClass();
            }
        }

        @:safety
        class SafeClass {
            var data: Int;
        }

        @:safety @:move
        class AnotherSafeClass {
            var value: String;
        }
    "#;

    let result = compile_haxe_source(source);

    if let Some(typed_file) = result.typed_files.first() {
        let safety_mode = typed_file.get_program_safety_mode();
        match safety_mode {
            Some(compiler::tast::SafetyMode::Strict) => {
                println!("  ✅ Program uses strict manual memory mode");
            }
            other => {
                println!("  ❌ Expected Strict mode, got {:?}", other);
            }
        }

        // Should have no safety-related errors (all classes annotated)
        let safety_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|e| e.message.contains("must have @:safety"))
            .collect();

        if safety_errors.is_empty() {
            println!("  ✅ No safety annotation errors (all classes have @:safety)");
        } else {
            println!("  ❌ Got {} safety errors:", safety_errors.len());
            for err in safety_errors {
                println!("      {}", err.message);
            }
        }
    } else {
        println!("  ❌ Failed to compile");
    }
}

/// Test 4: @:safety(true) with unannotated class = compilation error
fn test_strict_mode_invalid() {
    println!("\nTest 4: Strict Safety Mode with Unannotated Class (Should Error)");
    let source = r#"
        @:safety(true)
        class Main {
            static function main() {
                var x = new UnsafeClass();  // ERROR: UnsafeClass lacks @:safety
            }
        }

        class UnsafeClass {
            var data: String;  // Missing @:safety annotation
        }
    "#;

    let result = compile_haxe_source(source);

    if let Some(typed_file) = result.typed_files.first() {
        let safety_mode = typed_file.get_program_safety_mode();
        match safety_mode {
            Some(compiler::tast::SafetyMode::Strict) => {
                println!("  ✅ Program uses strict manual memory mode");
            }
            other => {
                println!("  ❌ Expected Strict mode, got {:?}", other);
            }
        }

        // Should have errors about missing @:safety
        let safety_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|e| e.message.contains("must have @:safety"))
            .collect();

        if !safety_errors.is_empty() {
            println!("  ✅ Got expected safety error:");
            for err in safety_errors {
                println!("      {}", err.message);
            }
        } else {
            println!("  ❌ Expected safety annotation error, but got none");
            if !result.errors.is_empty() {
                println!("  Other errors:");
                for err in &result.errors {
                    println!("      {}", err.message);
                }
            }
        }
    } else {
        println!("  ❌ Failed to compile");
    }
}
