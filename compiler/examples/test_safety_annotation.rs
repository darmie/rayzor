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
    println!("=== @:safety Annotation Test ===\n");

    let source = r#"package test;

// Regular class - uses runtime-managed memory (default)
class ManagedClass {
    public var value:Int;
    public function new() {
        this.value = 0;
    }
}

// Explicitly managed class - uses runtime-managed memory
@:managed
class ExplicitManagedClass {
    public var data:String;
    public function new() {
        this.data = "";
    }
}

// Opt-in to manual memory management
@:safety
@:move
class SafetyClass {
    public var count:Int;
    public function new() {
        this.count = 0;
    }
}

// Another safety class with unique ownership
@:safety
@:unique
class UniqueResource {
    public var id:Int;
    public function new(i:Int) {
        this.id = i;
    }
}
"#;

    let config = CompilationConfig::default();
    let mut unit = CompilationUnit::new(config);

    match unit.add_file(source, "test_safety.hx") {
        Ok(_) => {
            match unit.lower_to_tast() {
                Ok(typed_files) => {
                    println!("✅ Successfully parsed and lowered code!\n");

                    for typed_file in &typed_files {
                        for class in &typed_file.classes {
                            let has_safety = class.has_safety_annotation();
                            let is_managed = class.is_managed();
                            let uses_manual = class.uses_manual_memory();

                            println!(
                                "Class with {} annotation(s):",
                                class.memory_annotations.len()
                            );
                            for annotation in &class.memory_annotations {
                                println!("  - {:?}", annotation);
                            }
                            println!("  has_safety_annotation(): {}", has_safety);
                            println!("  is_managed(): {}", is_managed);
                            println!("  uses_manual_memory(): {}", uses_manual);
                            println!();
                        }
                    }

                    // Verify expected behavior
                    println!("=== Verification ===");
                    let mut managed_default = 0;
                    let mut managed_explicit = 0;
                    let mut safety_classes = 0;

                    for typed_file in &typed_files {
                        for class in &typed_file.classes {
                            if class.has_safety_annotation() {
                                safety_classes += 1;
                            } else if class.is_managed() {
                                managed_explicit += 1;
                            } else {
                                // No annotation = default managed
                                managed_default += 1;
                            }
                        }
                    }

                    println!(
                        "Classes using default runtime-managed memory: {}",
                        managed_default
                    );
                    println!("Classes explicitly marked @:managed: {}", managed_explicit);
                    println!("Classes with @:safety (manual memory): {}", safety_classes);

                    if managed_default == 1 && managed_explicit == 1 && safety_classes == 2 {
                        println!("\n✅ All classes correctly categorized!");
                        println!("   - ManagedClass uses runtime-managed memory (default)");
                        println!("   - ExplicitManagedClass uses runtime-managed memory (@:managed)");
                        println!("   - SafetyClass uses manual memory (@:safety)");
                        println!("   - UniqueResource uses manual memory (@:safety)");
                    } else {
                        println!("\n❌ Unexpected categorization");
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
