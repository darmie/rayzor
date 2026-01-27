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
//! Test that @:op metadata is correctly extracted and stored in abstract type methods

use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("\n=== Testing @:op Metadata Extraction ===\n");

    let source = r#"
        package test;

        abstract Counter(Int) from Int to Int {
            @:op(A + B)
            public inline function add(rhs:Counter):Counter {
                return new Counter(this + rhs.toInt());
            }

            @:op(A * B)
            public inline function multiply(rhs:Counter):Counter {
                return new Counter(this * rhs.toInt());
            }

            @:op(-A)
            public inline function negate():Counter {
                return new Counter(-this);
            }

            public inline function toInt():Int {
                return this;
            }
        }

        class Main {
            public static function main():Int {
                var a:Counter = 5;
                return a.toInt();
            }
        }
    "#;

    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    // Parse and lower to TAST
    unit.add_file(source, "test.hx")?;
    let typed_files = unit.lower_to_tast().map_err(|e| format!("{:?}", e))?;

    println!("✓ TAST generation successful\n");

    // Find the Counter abstract type
    let counter_abstract = typed_files
        .iter()
        .flat_map(|f| &f.abstracts)
        .find(|a| unit.string_interner.get(a.name) == Some("Counter"))
        .ok_or("Counter abstract not found")?;

    println!(
        "Found Counter abstract with {} methods\n",
        counter_abstract.methods.len()
    );

    // Check each method for operator metadata
    for method in &counter_abstract.methods {
        let method_name = unit.string_interner.get(method.name).unwrap_or("<unknown>");
        println!("Method: {}", method_name);

        if !method.metadata.operator_metadata.is_empty() {
            for (op_expr, params) in &method.metadata.operator_metadata {
                println!("  ✓ Operator metadata: {}", op_expr);
                if !params.is_empty() {
                    println!("    Additional params: {:?}", params);
                }
            }
        } else {
            println!("  (no operator metadata)");
        }
        println!();
    }

    // Verify specific operators
    let add_method = counter_abstract
        .methods
        .iter()
        .find(|m| unit.string_interner.get(m.name) == Some("add"))
        .ok_or("add method not found")?;

    if add_method.metadata.operator_metadata.is_empty() {
        return Err("❌ FAILED: add method has no operator metadata".to_string());
    }

    let (op_expr, _) = &add_method.metadata.operator_metadata[0];
    // The operator will be in Debug format, e.g. "A Add B" instead of "A + B"
    if !op_expr.contains("Add") && !op_expr.contains("+") {
        return Err(format!(
            "❌ FAILED: Expected 'Add' or '+' operator, got: {}",
            op_expr
        ));
    }

    println!("✅ TEST PASSED: Operator metadata correctly extracted!");
    println!("  - add() has operator: {}", op_expr);

    let multiply_method = counter_abstract
        .methods
        .iter()
        .find(|m| unit.string_interner.get(m.name) == Some("multiply"))
        .ok_or("multiply method not found")?;

    let (mul_expr, _) = &multiply_method.metadata.operator_metadata[0];
    if !mul_expr.contains("Mul") && !mul_expr.contains("*") {
        return Err(format!(
            "❌ FAILED: Expected 'Mul' or '*' operator, got: {}",
            mul_expr
        ));
    }

    println!("  - multiply() has operator: {}", mul_expr);

    let negate_method = counter_abstract
        .methods
        .iter()
        .find(|m| unit.string_interner.get(m.name) == Some("negate"))
        .ok_or("negate method not found")?;

    let (neg_expr, _) = &negate_method.metadata.operator_metadata[0];
    // Unary negation will be "NegA" or "-A"
    if !neg_expr.contains("Neg") && !neg_expr.contains("-") {
        return Err(format!(
            "❌ FAILED: Expected 'Neg' or '-' operator, got: {}",
            neg_expr
        ));
    }

    println!("  - negate() has operator: {}", neg_expr);

    println!("\n✅ All operator metadata extracted correctly!\n");

    Ok(())
}
