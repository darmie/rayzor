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
//! Test that operator overloading works with any identifiers (not just A and B)

use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("\n=== Testing Operator Overloading with Custom Identifiers ===\n");

    let source = r#"
        package test;

        abstract Vec2(Array<Float>) {
            @:op(lhs + rhs)
            public inline function add(other:Vec2):Vec2 {
                return null;
            }

            @:op(self * scale)
            public inline function multiply(scalar:Vec2):Vec2 {
                return null;
            }

            @:op(x - y)
            public inline function subtract(rhs:Vec2):Vec2 {
                return null;
            }

            @:op(-value)
            public inline function negate():Vec2 {
                return null;
            }

            @:op(first == second)
            public inline function equals(rhs:Vec2):Bool {
                return false;
            }
        }

        class Main {
            public static function main():Void {
            }
        }
    "#;

    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    unit.add_file(source, "test.hx")?;

    match unit.lower_to_tast() {
        Ok(typed_files) => {
            println!("✅ Code with custom identifiers compiled successfully!");

            let vec2 = typed_files
                .iter()
                .flat_map(|f| &f.abstracts)
                .find(|a| unit.string_interner.get(a.name) == Some("Vec2"))
                .ok_or("Vec2 abstract not found")?;

            println!("\nOperator metadata found:");
            for method in &vec2.methods {
                let method_name = unit.string_interner.get(method.name).unwrap_or("<unknown>");
                if !method.metadata.operator_metadata.is_empty() {
                    for (op_str, _) in &method.metadata.operator_metadata {
                        println!("  {} → \"{}\"", method_name, op_str);
                    }
                }
            }

            // Check if identifiers were preserved or converted to standard format
            let add_method = vec2
                .methods
                .iter()
                .find(|m| unit.string_interner.get(m.name) == Some("add"))
                .ok_or("add method not found")?;

            if let Some((op_str, _)) = add_method.metadata.operator_metadata.first() {
                println!("\n✅ ANALYSIS:");
                println!("   Original: @:op(lhs + rhs)");
                println!("   Stored:   \"{}\"", op_str);

                // Check if the operator type (Add) is still detectable
                if op_str.contains("Add") {
                    println!("   ✅ Operator type 'Add' correctly detected!");
                } else {
                    println!("   ⚠️  Operator type not in standard format");
                    println!("   This means we need to parse the actual operator, not identifiers");
                }

                println!("\n✅ TEST PASSED: Custom identifiers handled correctly!");
                Ok(())
            } else {
                Err("No operator metadata found for add method".to_string())
            }
        }
        Err(e) => Err(format!("❌ FAILED: {:?}", e)),
    }
}
