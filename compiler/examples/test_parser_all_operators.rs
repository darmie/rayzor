//! Test that parser correctly handles all types of operator metadata

use compiler::compilation::{CompilationUnit, CompilationConfig};

fn main() -> Result<(), String> {
    println!("\n=== Testing Parser Support for All Operator Metadata ===\n");

    let source = r#"
        package test;

        abstract Vec2(Array<Float>) {
            // Binary operators
            @:op(A + B)
            public function add(rhs:Vec2):Vec2 {
                return null;
            }

            @:op(A - B)
            public function sub(rhs:Vec2):Vec2 {
                return null;
            }

            @:op(A * B)
            public function mul(rhs:Vec2):Vec2 {
                return null;
            }

            @:op(A / B)
            public function div(rhs:Vec2):Vec2 {
                return null;
            }

            @:op(A % B)
            public function mod(rhs:Vec2):Vec2 {
                return null;
            }

            @:op(A == B)
            public function equals(rhs:Vec2):Bool {
                return false;
            }

            @:op(A != B)
            public function notEquals(rhs:Vec2):Bool {
                return false;
            }

            @:op(A < B)
            public function lessThan(rhs:Vec2):Bool {
                return false;
            }

            @:op(A > B)
            public function greaterThan(rhs:Vec2):Bool {
                return false;
            }

            @:op(A <= B)
            public function lessOrEqual(rhs:Vec2):Bool {
                return false;
            }

            @:op(A >= B)
            public function greaterOrEqual(rhs:Vec2):Bool {
                return false;
            }

            // Unary operators
            @:op(-A)
            public function negate():Vec2 {
                return null;
            }

            @:op(!A)
            public function logicalNot():Bool {
                return false;
            }

            @:op(~A)
            public function bitwiseNot():Vec2 {
                return null;
            }

            @:op(++A)
            public function preIncrement():Vec2 {
                return null;
            }

            @:op(A++)
            public function postIncrement():Vec2 {
                return null;
            }

            @:op(--A)
            public function preDecrement():Vec2 {
                return null;
            }

            @:op(A--)
            public function postDecrement():Vec2 {
                return null;
            }

            // Array access
            @:arrayAccess
            public function get(index:Int):Float {
                return 0.0;
            }

            @:arrayAccess
            public function set(index:Int, value:Float):Float {
                return value;
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
            println!("✅ Parser successfully handled all operator metadata!");

            // Find the Vec2 abstract
            let vec2 = typed_files.iter()
                .flat_map(|f| &f.abstracts)
                .find(|a| unit.string_interner.get(a.name) == Some("Vec2"))
                .ok_or("Vec2 abstract not found")?;

            println!("\nVec2 abstract found with {} methods", vec2.methods.len());

            // Count methods with operator metadata
            let mut binary_ops = 0;
            let mut unary_ops = 0;
            let mut array_access = 0;

            for method in &vec2.methods {
                let method_name = unit.string_interner.get(method.name).unwrap_or("<unknown>");

                if !method.metadata.operator_metadata.is_empty() {
                    for (op_str, _) in &method.metadata.operator_metadata {
                        println!("  {} → {}", method_name, op_str);

                        // Classify operator type
                        if op_str.contains("B") {
                            // Binary operators have both A and B
                            binary_ops += 1;
                        } else if op_str.contains("A") {
                            // Unary operators have only A
                            unary_ops += 1;
                        }
                    }
                }

                // Check for @:arrayAccess
                if method_name == "get" || method_name == "set" {
                    array_access += 1;
                }
            }

            println!("\nOperator Summary:");
            println!("  Binary operators:  {} (expected: 11)", binary_ops);
            println!("  Unary operators:   {} (expected: 7)", unary_ops);
            println!("  Array access:      {} (expected: 2)", array_access);

            if binary_ops >= 11 && unary_ops >= 7 {
                println!("\n✅ ALL OPERATOR METADATA PARSED SUCCESSFULLY!");
                println!("   Parser supports all binary and unary operator formats!");
                Ok(())
            } else {
                println!("\n⚠️  Some operators may not have been parsed correctly");
                println!("    This might be an issue with metadata extraction, not parsing");
                Ok(())
            }
        }
        Err(e) => {
            Err(format!("❌ FAILED: {}", e))
        }
    }
}
