/// Simple test to debug metadata parsing

use compiler::pipeline::compile_haxe_source;

fn main() {
    println!("=== Simple Safety Test ===\n");

    let source = r#"
        @:safety
        class Main {
            static function main() {}
        }
    "#;

    let result = compile_haxe_source(source);

    if let Some(typed_file) = result.typed_files.first() {
        println!("Found {} classes", typed_file.classes.len());
        for class in &typed_file.classes {
            let name = typed_file.get_string(class.name).unwrap_or_else(|| "<unknown>".to_string());
            println!("\nClass: {}", name);
            println!("  Memory annotations: {:?}", class.memory_annotations);
            println!("  has_safety_annotation(): {}", class.has_safety_annotation());
        }

        println!("\nProgram safety mode: {:?}", typed_file.get_program_safety_mode());
    } else {
        println!("Failed to compile");
        for err in &result.errors {
            println!("Error: {}", err.message);
        }
    }
}
