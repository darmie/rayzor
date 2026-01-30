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
//! Systems-level types end-to-end test suite
//!
//! Tests the complete pipeline for rayzor systems types:
//! - Box<T>: single-owner heap allocation
//! - Ptr<T>: raw mutable pointer
//! - Ref<T>: read-only reference
//! - Usize: unsigned pointer-sized integer
//! - Arc.asPtrTyped() / Arc.asRef(): typed pointer access to Arc data

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;

/// Test result
#[derive(Debug)]
enum TestResult {
    Success,
    Failed { error: String },
}

impl TestResult {
    fn is_success(&self) -> bool {
        matches!(self, TestResult::Success)
    }
}

/// A single end-to-end test case
struct E2ETestCase {
    name: String,
    haxe_source: String,
}

impl E2ETestCase {
    fn new(name: &str, haxe_source: &str) -> Self {
        Self {
            name: name.to_string(),
            haxe_source: haxe_source.to_string(),
        }
    }

    fn run(&self) -> TestResult {
        println!("\n{}", "=".repeat(70));
        println!("TEST: {}", self.name);
        println!("{}", "=".repeat(70));

        let mut unit = CompilationUnit::new(CompilationConfig::fast());

        if let Err(e) = unit.load_stdlib() {
            return TestResult::Failed {
                error: format!("Failed to load stdlib: {}", e),
            };
        }

        let filename = format!("{}.hx", self.name);
        if let Err(e) = unit.add_file(&self.haxe_source, &filename) {
            return TestResult::Failed {
                error: format!("Failed to add file: {}", e),
            };
        }

        println!("  Compiling to TAST...");
        let typed_files = match unit.lower_to_tast() {
            Ok(files) => {
                println!("  ✅ TAST ({} files)", files.len());
                files
            }
            Err(errors) => {
                return TestResult::Failed {
                    error: format!("TAST failed: {:?}", errors),
                };
            }
        };

        println!("  Lowering to MIR...");
        let mir_modules = unit.get_mir_modules();
        if mir_modules.is_empty() {
            return TestResult::Failed {
                error: "No MIR modules generated".to_string(),
            };
        }
        println!("  ✅ MIR ({} modules)", mir_modules.len());

        println!("  Compiling to native...");
        let plugin = rayzor_runtime::plugin_impl::get_plugin();
        let symbols = plugin.runtime_symbols();
        let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

        let mut backend = match CraneliftBackend::with_symbols(&symbols_ref) {
            Ok(b) => b,
            Err(e) => {
                return TestResult::Failed {
                    error: format!("Backend init failed: {}", e),
                };
            }
        };

        for module in &mir_modules {
            if let Err(e) = backend.compile_module(module) {
                return TestResult::Failed {
                    error: format!("Codegen failed: {}", e),
                };
            }
        }
        println!("  ✅ Codegen succeeded");

        println!("  Executing...");
        for module in mir_modules.iter().rev() {
            if let Ok(()) = backend.call_main(module) {
                println!("  ✅ Execution succeeded");
                return TestResult::Success;
            }
        }

        TestResult::Failed {
            error: "Failed to execute main".to_string(),
        }
    }
}

fn main() -> Result<(), String> {
    println!("=== Rayzor Systems Types E2E Test Suite ===\n");

    let tests: Vec<E2ETestCase> = vec![
        // ============================================================================
        // TEST 1: Box — basic init, unbox, free
        // ============================================================================
        E2ETestCase::new(
            "box_basic",
            r#"
package test;

import rayzor.Box;

class Main {
    static function main() {
        // Box an integer on the heap
        var boxed = Box.init(42);

        // Read it back
        var value = boxed.unbox();

        // Get the raw heap address
        var addr = boxed.raw();

        // Free the box
        boxed.free();
    }
}
"#,
        ),
        // ============================================================================
        // TEST 2: Box — asPtr and asRef borrowing
        // ============================================================================
        E2ETestCase::new(
            "box_borrow",
            r#"
package test;

import rayzor.Box;
import rayzor.Ptr;
import rayzor.Ref;

class Main {
    static function main() {
        var boxed = Box.init(99);

        // Borrow as mutable pointer
        var ptr:Ptr<Int> = boxed.asPtr();
        var val1 = ptr.deref();

        // Borrow as read-only reference
        var ref_:Ref<Int> = boxed.asRef();
        var val2 = ref_.deref();

        boxed.free();
    }
}
"#,
        ),
        // ============================================================================
        // TEST 3: Ptr — fromRaw, deref, write, offset
        // ============================================================================
        E2ETestCase::new(
            "ptr_basic",
            r#"
package test;

import rayzor.Ptr;
import rayzor.Box;

class Main {
    static function main() {
        // Create a Box to get a valid heap address
        var boxed = Box.init(100);
        var addr = boxed.raw();

        // Create a Ptr from the raw address
        var ptr:Ptr<Int> = Ptr.fromRaw(addr);

        // Read the value
        var value = ptr.deref();

        // Write a new value
        ptr.write(200);

        // Read again
        var newValue = ptr.deref();

        // Get raw address back
        var rawAddr = ptr.raw();

        // Check null
        var isNull = ptr.isNull();

        boxed.free();
    }
}
"#,
        ),
        // ============================================================================
        // TEST 4: Ref — fromRaw, deref (read-only)
        // ============================================================================
        E2ETestCase::new(
            "ref_basic",
            r#"
package test;

import rayzor.Ref;
import rayzor.Box;

class Main {
    static function main() {
        var boxed = Box.init(77);
        var addr = boxed.raw();

        // Create a Ref from the raw address
        var ref_:Ref<Int> = Ref.fromRaw(addr);

        // Read the value (read-only)
        var value = ref_.deref();

        // Get raw address
        var rawAddr = ref_.raw();

        boxed.free();
    }
}
"#,
        ),
        // ============================================================================
        // TEST 5: Usize — arithmetic and bitwise operations
        // ============================================================================
        E2ETestCase::new(
            "usize_arithmetic",
            r#"
package test;

import rayzor.Usize;

class Main {
    static function main() {
        // Implicit @:from Int -> Usize
        var a:Usize = 100;
        var b:Usize = 50;

        // Arithmetic — trace Usize directly (implicit @:to Int)
        var sum = a + b;
        var diff = a - b;
        trace(sum);    // 150
        trace(diff);   // 50

        // Bitwise
        var x:Usize = 0xFF;
        var y:Usize = 0x0F;
        trace(x & y);  // 15
        trace(x | y);  // 255

        // Shifts
        var one:Usize = 1;
        trace(one << 4);     // 16

        // isZero
        var zero:Usize = 0;
        trace(zero.isZero());   // true
        trace(a.isZero());      // false
    }
}
"#,
        ),
        // ============================================================================
        // TEST 6: Usize — alignUp
        // ============================================================================
        E2ETestCase::new(
            "usize_align",
            r#"
package test;

import rayzor.Usize;

class Main {
    static function main() {
        // Align 13 up to 8-byte boundary -> 16
        var addr:Usize = 13;
        var alignment:Usize = 8;
        trace(addr.alignUp(alignment));   // 16

        // Align 16 up to 8-byte boundary -> 16 (already aligned)
        var addr2:Usize = 16;
        trace(addr2.alignUp(alignment));  // 16

        // Align 1 up to 16-byte boundary -> 16
        var addr3:Usize = 1;
        var align16:Usize = 16;
        trace(addr3.alignUp(align16));    // 16
    }
}
"#,
        ),
        // ============================================================================
        // TEST 7: Usize — Ptr/Ref conversions
        // ============================================================================
        E2ETestCase::new(
            "usize_ptr_conversion",
            r#"
package test;

import rayzor.Usize;
import rayzor.Ptr;
import rayzor.Ref;
import rayzor.Box;

class Main {
    static function main() {
        // Create a heap value
        var boxed = Box.init(42);
        var ptr:Ptr<Int> = boxed.asPtr();

        // Convert Ptr to Usize
        var addr = Usize.fromPtr(ptr);

        // Convert Usize back to Ptr
        var ptr2:Ptr<Int> = addr.toPtr();
        var value = ptr2.deref();

        // Convert Usize to Ref
        var ref_:Ref<Int> = addr.toRef();
        var value2 = ref_.deref();

        // Convert Ref to Usize
        var addr2 = Usize.fromRef(ref_);

        boxed.free();
    }
}
"#,
        ),
        // ============================================================================
        // TEST 8: Arc — asPtrTyped and asRef
        // ============================================================================
        E2ETestCase::new(
            "arc_typed_ptrs",
            r#"
package test;

import rayzor.concurrent.Arc;
import rayzor.Ptr;
import rayzor.Ref;

class Main {
    static function main() {
        var arc = Arc.init(42);

        // Get typed mutable pointer
        var ptr:Ptr<Int> = arc.asPtrTyped();
        var rawAddr = ptr.raw();

        // Get typed read-only reference
        var ref_:Ref<Int> = arc.asRef();
        var rawAddr2 = ref_.raw();
    }
}
"#,
        ),
        // ============================================================================
        // TEST 9: Integration — Box + Ptr + Arc
        // ============================================================================
        E2ETestCase::new(
            "systems_integration",
            r#"
package test;

import rayzor.Box;
import rayzor.Ptr;
import rayzor.Ref;
import rayzor.Usize;
import rayzor.concurrent.Arc;

class Main {
    static function main() {
        // Box a value, get Ptr, manipulate via Usize
        var boxed = Box.init(10);
        var ptr:Ptr<Int> = boxed.asPtr();
        var addr = Usize.fromPtr(ptr);

        // Check it's not zero
        var notZero = addr.isZero();  // false

        // Read via Ref from Usize
        var ref_:Ref<Int> = addr.toRef();
        var val = ref_.deref();

        // Write via Ptr
        ptr.write(20);
        var newVal = ptr.deref();

        // Arc with typed pointer access
        var arc = Arc.init(99);
        var arcPtr:Ptr<Int> = arc.asPtrTyped();
        var arcRef:Ref<Int> = arc.asRef();

        boxed.free();
    }
}
"#,
        ),
    ];

    // ============================================================================
    // Run all tests
    // ============================================================================
    let mut passed = 0;
    let mut failed = 0;
    let mut results: Vec<(String, bool)> = Vec::new();

    for test in &tests {
        let result = test.run();
        let success = result.is_success();
        if success {
            println!("\n✅ {} PASSED", test.name);
            passed += 1;
        } else {
            if let TestResult::Failed { error } = &result {
                println!("\n❌ {} FAILED: {}", test.name, error);
            }
            failed += 1;
        }
        results.push((test.name.clone(), success));
    }

    // Summary
    println!("\n{}", "=".repeat(70));
    println!("TEST SUMMARY");
    println!("{}", "=".repeat(70));
    println!("\n📊 Overall:");
    println!("   Total:  {}", results.len());
    println!("   Passed: {} ({}%)", passed, passed * 100 / results.len());
    println!("   Failed: {}", failed);

    println!("\n📋 Results:");
    for (name, success) in &results {
        if *success {
            println!("   ✅ {} (reached Execution)", name);
        } else {
            println!("   ❌ {} (failed)", name);
        }
    }

    if failed == 0 {
        println!("\n🎉 All tests passed!");
        Ok(())
    } else {
        Err(format!("{} test(s) failed", failed))
    }
}
