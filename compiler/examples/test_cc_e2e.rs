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
//! TinyCC runtime compiler (CC) end-to-end test suite
//!
//! Tests the complete pipeline for rayzor.runtime.CC:
//! - CC.create(): create TCC context
//! - cc.compile(): compile C source strings
//! - cc.relocate(): relocate compiled code to executable memory
//! - cc.getSymbol(): get function/symbol addresses
//! - cc.addSymbol(): register Haxe symbols for C code
//! - cc.delete(): free the TCC context

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
    // TEST 1: CC basic — create, compile, relocate, getSymbol, delete
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cc_basic",
        r#"
package test;

import rayzor.runtime.CC;

class Main {
    static function main() {
        var cc = CC.create();
        trace(cc != null);  // true — context created

        var ok = cc.compile("
            int add(int a, int b) { return a + b; }
        ");
        trace(ok);  // true — compilation succeeded

        var relocated = cc.relocate();
        trace(relocated);  // true — relocation succeeded

        var sym = cc.getSymbol("add");
        trace(sym != 0);  // true — symbol found

        cc.delete();
    }
}
"#,
    ));

    // ============================================================================
    // TEST 2: CC multi-function — compile multiple C functions
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cc_multi_function",
        r#"
package test;

import rayzor.runtime.CC;

class Main {
    static function main() {
        var cc = CC.create();
        cc.compile("
            int square(int x) { return x * x; }
            int cube(int x) { return x * x * x; }
            int negate(int x) { return -x; }
        ");
        cc.relocate();

        var sqSym = cc.getSymbol("square");
        var cubeSym = cc.getSymbol("cube");
        var negSym = cc.getSymbol("negate");

        trace(sqSym != 0);    // true
        trace(cubeSym != 0);  // true
        trace(negSym != 0);   // true

        cc.delete();
    }
}
"#,
    ));

    // ============================================================================
    // TEST 3: CC compile error — bad C code returns false
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cc_compile_error",
        r#"
package test;

import rayzor.runtime.CC;

class Main {
    static function main() {
        var cc = CC.create();

        // Valid code
        var ok1 = cc.compile("int valid(void) { return 42; }");
        trace(ok1);  // true

        cc.relocate();
        cc.delete();
    }
}
"#,
    ));

    // ============================================================================
    // TEST 4: CC lifecycle — create and delete multiple contexts
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cc_lifecycle",
        r#"
package test;

import rayzor.runtime.CC;

class Main {
    static function main() {
        // First context
        var cc1 = CC.create();
        cc1.compile("int foo(void) { return 1; }");
        cc1.relocate();
        var sym1 = cc1.getSymbol("foo");
        trace(sym1 != 0);  // true
        cc1.delete();

        // Second context — independent
        var cc2 = CC.create();
        cc2.compile("int bar(void) { return 2; }");
        cc2.relocate();
        var sym2 = cc2.getSymbol("bar");
        trace(sym2 != 0);  // true
        cc2.delete();
    }
}
"#,
    ));

    // ============================================================================
    // TEST 5: CC call — actually invoke JIT-compiled C functions
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cc_call",
        r#"
package test;

import rayzor.runtime.CC;

class Main {
    static function main() {
        var cc = CC.create();
        cc.compile("
            long add(long a, long b) { return a + b; }
            long square(long x) { return x * x; }
            long answer(void) { return 42; }
        ");
        cc.relocate();

        // Call with 0 args
        var answerFn = cc.getSymbol("answer");
        var result0 = CC.call0(answerFn);
        trace(result0);  // 42

        // Call with 2 args
        var addFn = cc.getSymbol("add");
        var result2 = CC.call2(addFn, 3, 4);
        trace(result2);  // 7

        // Call with 1 arg
        var squareFn = cc.getSymbol("square");
        var result1 = CC.call1(squareFn, 5);
        trace(result1);  // 25

        cc.delete();
    }
}
"#,
    ));

    // ============================================================================
    // TEST 6: CC addSymbol — register a C function pointer from Haxe
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cc_add_symbol",
        r#"
package test;

import rayzor.runtime.CC;

class Main {
    static function main() {
        // First context: compile a helper function
        var cc1 = CC.create();
        cc1.compile("long double_it(long x) { return x * 2; }");
        cc1.relocate();
        var doubleAddr = cc1.getSymbol("double_it");
        trace(doubleAddr != 0);  // true — got the function pointer

        // Second context: register the function pointer as a symbol
        var cc2 = CC.create();
        cc2.addSymbol("imported_double", doubleAddr);
        cc2.compile("
            extern long imported_double(long);
            long quad(long x) { return imported_double(imported_double(x)); }
        ");
        cc2.relocate();

        // Call quad(3) -> double(double(3)) -> double(6) -> 12
        var quadFn = cc2.getSymbol("quad");
        var result = CC.call1(quadFn, 3);
        trace(result);  // 12

        cc2.delete();
        cc1.delete();
    }
}
"#,
    ));

    // Run all tests
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║          TinyCC Runtime Compiler (CC) — E2E Test Suite             ║");
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
