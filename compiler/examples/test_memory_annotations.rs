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
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::tast::MemoryAnnotation;

fn main() {
    println!("=== Memory Annotation Test ===\n");

    let source = r#"package test;

@:move
@:unique
class MoveOnlyType {
    public var value:Int;
    public function new(v:Int) {
        this.value = v;
    }
}

@:arc
class SharedType {
    public var data:String;
    public function new() {
        this.data = "";
    }
}

class Test {
    @:borrow
    public static function borrowValue(x:Int):Void {
        trace(x);
    }

    @:owned
    public static function consumeValue(x:Int):Void {
        trace(x);
    }
}
"#;

    let config = CompilationConfig::default();
    let mut unit = CompilationUnit::new(config);

    match unit.add_file(source, "test_annotations.hx") {
        Ok(_) => {
            match unit.lower_to_tast() {
                Ok(typed_files) => {
                    println!("✅ Successfully parsed and lowered code with memory annotations!\n");

                    // Count annotations found
                    let mut total_class_annotations = 0;
                    let mut total_method_annotations = 0;

                    for typed_file in &typed_files {
                        for class in &typed_file.classes {
                            if !class.memory_annotations.is_empty() {
                                println!(
                                    "Found class with {} memory annotation(s)",
                                    class.memory_annotations.len()
                                );
                                for annotation in &class.memory_annotations {
                                    println!("  - {:?}", annotation);
                                    total_class_annotations += 1;
                                }
                                println!();
                            }

                            for method in &class.methods {
                                if !method.metadata.memory_annotations.is_empty() {
                                    println!(
                                        "Found method with {} memory annotation(s)",
                                        method.metadata.memory_annotations.len()
                                    );
                                    for annotation in &method.metadata.memory_annotations {
                                        println!("  - {:?}", annotation);
                                        total_method_annotations += 1;
                                    }
                                    println!();
                                }
                            }
                        }
                    }

                    println!(
                        "Total: {} class annotations, {} method annotations\n",
                        total_class_annotations, total_method_annotations
                    );

                    // Verify expected counts
                    println!("=== Verification ===");
                    // Expected: 3 class annotations (@:move, @:unique on MoveOnlyType, @:arc on SharedType)
                    // Expected: 2 method annotations (@:borrow on borrowValue, @:owned on consumeValue)
                    if total_class_annotations == 3 && total_method_annotations == 2 {
                        println!("✅ All expected annotations found!");
                        println!("   - 3 class annotations (@:move, @:unique, @:arc)");
                        println!("   - 2 method annotations (@:borrow, @:owned)");
                        println!("\n🎉 Memory annotation system working correctly!");
                    } else {
                        println!("❌ Unexpected annotation counts");
                        println!(
                            "   Expected: 3 class annotations, got {}",
                            total_class_annotations
                        );
                        println!(
                            "   Expected: 2 method annotations, got {}",
                            total_method_annotations
                        );
                    }
                }
                Err(errors) => {
                    println!("❌ Lowering failed with {} error(s)", errors.len());
                    for error in &errors {
                        println!("  {}", error.message);
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Parsing failed: {:?}", e);
        }
    }
}
