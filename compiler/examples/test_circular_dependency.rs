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
//! Test circular dependency detection
//!
//! This demonstrates:
//! 1. Detection of circular dependencies
//! 2. Proper error reporting with cycle paths
//! 3. Best-effort compilation order even with cycles
//! 4. Topological sorting for non-circular dependencies

use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    println!("=== Testing Circular Dependency Detection ===\n");

    test_simple_circular();
    test_complex_circular();
    test_valid_ordering();
}

fn test_simple_circular() {
    println!("Test 1: Simple Circular Dependency (A → B → A)\n");

    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false, // Skip stdlib for this test
        ..Default::default()
    });

    // File A imports B
    let file_a = r#"
        package com.example;
        import com.example.B;

        class A {
            public function new() {}
            public function useB():Void {
                var b = new B();
            }
        }
    "#;

    // File B imports A (creates cycle)
    let file_b = r#"
        package com.example;
        import com.example.A;

        class B {
            public function new() {}
            public function useA():Void {
                var a = new A();
            }
        }
    "#;

    unit.add_file(file_a, "com/example/A.hx")
        .expect("Failed to add A.hx");
    unit.add_file(file_b, "com/example/B.hx")
        .expect("Failed to add B.hx");

    println!("Added files with circular dependency: A → B → A");

    // Analyze dependencies
    match unit.analyze_dependencies() {
        Ok(analysis) => {
            if analysis.circular_dependencies.is_empty() {
                println!("❌ FAILED: No circular dependency detected!\n");
            } else {
                println!(
                    "✓ Detected {} circular dependency(ies)",
                    analysis.circular_dependencies.len()
                );
                for cycle in &analysis.circular_dependencies {
                    println!("  Cycle: {}", cycle.cycle.join(" → "));
                }
                println!(
                    "✓ Compilation order provided: {:?}",
                    analysis.compilation_order
                );
                println!("✓ TEST PASSED\n");
            }
        }
        Err(e) => {
            println!("❌ FAILED: Error during analysis: {:?}\n", e);
        }
    }
}

fn test_complex_circular() {
    println!("Test 2: Complex Circular Dependency (A → B → C → A)\n");

    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    let file_a = r#"
        package com.example;
        import com.example.B;

        class A {
            public function new() {}
            public function useB():B {
                return new B();
            }
        }
    "#;

    let file_b = r#"
        package com.example;
        import com.example.C;

        class B {
            public function new() {}
            public function useC():C {
                return new C();
            }
        }
    "#;

    let file_c = r#"
        package com.example;
        import com.example.A;

        class C {
            public function new() {}
            public function useA():A {
                return new A();
            }
        }
    "#;

    unit.add_file(file_a, "com/example/A.hx")
        .expect("Failed to add A.hx");
    unit.add_file(file_b, "com/example/B.hx")
        .expect("Failed to add B.hx");
    unit.add_file(file_c, "com/example/C.hx")
        .expect("Failed to add C.hx");

    println!("Added files with 3-way circular dependency");

    match unit.analyze_dependencies() {
        Ok(analysis) => {
            if analysis.circular_dependencies.is_empty() {
                println!("❌ FAILED: No circular dependency detected!\n");
            } else {
                println!(
                    "✓ Detected {} circular dependency(ies)",
                    analysis.circular_dependencies.len()
                );
                for (i, cycle) in analysis.circular_dependencies.iter().enumerate() {
                    println!("  Cycle {}: {}", i + 1, cycle.cycle.join(" → "));
                }
                println!(
                    "✓ Compilation still possible with order: {:?}",
                    analysis.compilation_order
                );
                println!("✓ TEST PASSED\n");
            }
        }
        Err(e) => {
            println!("❌ FAILED: Error during analysis: {:?}\n", e);
        }
    }
}

fn test_valid_ordering() {
    println!("Test 3: Valid Dependency Chain (D → C → B → A, no cycles)\n");

    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    // A has no dependencies
    let file_a = r#"
        package com.example;

        class A {
            public function new() {}
            public function getValue():Int {
                return 42;
            }
        }
    "#;

    // B depends on A
    let file_b = r#"
        package com.example;
        import com.example.A;

        class B {
            private var a:A;
            public function new() {
                this.a = new A();
            }
            public function compute():Int {
                return a.getValue() + 10;
            }
        }
    "#;

    // C depends on B
    let file_c = r#"
        package com.example;
        import com.example.B;

        class C {
            private var b:B;
            public function new() {
                this.b = new B();
            }
            public function compute():Int {
                return b.compute() + 10;
            }
        }
    "#;

    // D depends on C
    let file_d = r#"
        package com.example;
        import com.example.C;

        class D {
            public static function main():Void {
                var c = new C();
                var result = c.compute();
            }
        }
    "#;

    unit.add_file(file_d, "com/example/D.hx")
        .expect("Failed to add D.hx");
    unit.add_file(file_c, "com/example/C.hx")
        .expect("Failed to add C.hx");
    unit.add_file(file_b, "com/example/B.hx")
        .expect("Failed to add B.hx");
    unit.add_file(file_a, "com/example/A.hx")
        .expect("Failed to add A.hx");

    println!("Added files in reverse dependency order (D, C, B, A)");

    match unit.analyze_dependencies() {
        Ok(analysis) => {
            if !analysis.circular_dependencies.is_empty() {
                println!("❌ FAILED: False positive circular dependency detected!");
                for cycle in &analysis.circular_dependencies {
                    println!("  Cycle: {}", cycle.cycle.join(" → "));
                }
                println!();
            } else {
                println!("✓ No circular dependencies (correct)");
                println!("✓ Compilation order: {:?}", analysis.compilation_order);

                // Verify order: A should come before B, B before C, C before D
                let order = &analysis.compilation_order;
                let a_idx = order.iter().position(|&i| i == 3).unwrap(); // A was added 4th (index 3)
                let b_idx = order.iter().position(|&i| i == 2).unwrap(); // B was added 3rd (index 2)
                let c_idx = order.iter().position(|&i| i == 1).unwrap(); // C was added 2nd (index 1)
                let d_idx = order.iter().position(|&i| i == 0).unwrap(); // D was added 1st (index 0)

                if a_idx < b_idx && b_idx < c_idx && c_idx < d_idx {
                    println!("✓ Correct topological order: A → B → C → D");
                    println!("✓ TEST PASSED\n");
                } else {
                    println!("❌ FAILED: Incorrect topological order!");
                    println!("  Expected: A before B before C before D");
                    println!(
                        "  Got: positions A={}, B={}, C={}, D={}\n",
                        a_idx, b_idx, c_idx, d_idx
                    );
                }
            }
        }
        Err(e) => {
            println!("❌ FAILED: Error during analysis: {:?}\n", e);
        }
    }
}
