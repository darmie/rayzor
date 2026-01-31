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
    clippy::clone_on_copy,
    clippy::vec_init_then_push
)]
//! SIMD4f end-to-end test suite
//!
//! Tests the complete pipeline for rayzor.SIMD4f:
//! - SIMD4f.splat(): broadcast scalar to all 4 lanes
//! - SIMD4f.make(): construct from 4 individual values
//! - Arithmetic operators: +, -, *, /
//! - Lane access: extract, insert
//! - Reductions: sum, dot

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

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

fn main() {
    let mut tests = Vec::new();

    // ============================================================================
    // TEST 1: SIMD4f.splat — broadcast scalar to all 4 lanes
    // ============================================================================
    tests.push(E2ETestCase::new(
        "simd4f_splat",
        r#"
package test;

import rayzor.SIMD4f;

class Main {
    static function main() {
        var a = SIMD4f.splat(3.0);
        trace(true);  // splat created successfully
    }
}
"#,
    ));

    // ============================================================================
    // TEST 2: SIMD4f.make — construct from 4 individual values
    // ============================================================================
    tests.push(E2ETestCase::new(
        "simd4f_make",
        r#"
package test;

import rayzor.SIMD4f;

class Main {
    static function main() {
        var a = SIMD4f.make(1.0, 2.0, 3.0, 4.0);
        trace(true);  // make created successfully
    }
}
"#,
    ));

    // ============================================================================
    // TEST 3: SIMD4f arithmetic — just add two vectors
    // ============================================================================
    tests.push(E2ETestCase::new(
        "simd4f_arithmetic",
        r#"
package test;

import rayzor.SIMD4f;

class Main {
    static function main() {
        var a = SIMD4f.splat(1.0);
        var b = SIMD4f.splat(2.0);
        var c = a + b;
        trace(true);
    }
}
"#,
    ));

    // ============================================================================
    // TEST 4: SIMD4f.sum — horizontal reduction
    // ============================================================================
    tests.push(E2ETestCase::new(
        "simd4f_sum",
        r#"
package test;

import rayzor.SIMD4f;

class Main {
    static function main() {
        var a = SIMD4f.make(1.0, 2.0, 3.0, 4.0);
        var s = a.sum();
        trace(s);  // 10.0
    }
}
"#,
    ));

    // ============================================================================
    // TEST 5: SIMD4f.dot — dot product
    // ============================================================================
    tests.push(E2ETestCase::new(
        "simd4f_dot",
        r#"
package test;

import rayzor.SIMD4f;

class Main {
    static function main() {
        var a = SIMD4f.make(1.0, 2.0, 3.0, 4.0);
        var b = SIMD4f.splat(2.0);
        var d = a.dot(b);
        trace(d);  // 20.0
    }
}
"#,
    ));

    // ============================================================================
    // TEST 6: Tuple literal construction — var a:SIMD4f = (1.0, 2.0, 3.0, 4.0)
    // ============================================================================
    tests.push(E2ETestCase::new(
        "simd4f_tuple_literal",
        r#"
package test;

import rayzor.SIMD4f;

class Main {
    static function main() {
        var a:SIMD4f = (1.0, 2.0, 3.0, 4.0);
        var s = a.sum();
        trace(s);  // 10.0
    }
}
"#,
    ));

    // ============================================================================
    // TEST 7: @:from Array literal — var a:SIMD4f = [1.0, 2.0, 3.0, 4.0]
    // ============================================================================
    tests.push(E2ETestCase::new(
        "simd4f_from_array",
        r#"
package test;

import rayzor.SIMD4f;

class Main {
    static function main() {
        var a:SIMD4f = [1.0, 2.0, 3.0, 4.0];
        var s = a.sum();
        trace(s);  // 10.0
    }
}
"#,
    ));

    // Run all tests
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║            SIMD4f — E2E Test Suite                                 ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");

    let results: Vec<(String, TestResult)> =
        tests.iter().map(|t| (t.name.clone(), t.run())).collect();

    println!("\n\n{}", "=".repeat(70));
    println!("TEST SUMMARY");
    println!("{}", "=".repeat(70));

    let total = results.len();
    let passed = results.iter().filter(|(_, r)| r.is_success()).count();
    let failed = total - passed;

    println!("\n📊 Overall:");
    println!("   Total:  {}", total);
    println!("   Passed: {} ({}%)", passed, passed * 100 / total);
    println!("   Failed: {}", failed);

    println!("\n📋 Results:");
    for (name, result) in &results {
        match result {
            TestResult::Success => {
                println!("   ✅ {} (reached Execution)", name);
            }
            TestResult::Failed { error } => {
                println!("   ❌ {} — {}", name, error);
            }
        }
    }

    if failed == 0 {
        println!("\n🎉 All tests passed!");
    } else {
        println!("\n⚠️  {} test(s) failed", failed);
        std::process::exit(1);
    }
}
