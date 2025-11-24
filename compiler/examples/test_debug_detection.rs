/// Debug program safety mode detection

use compiler::pipeline::compile_haxe_source;

fn main() {
    let source = r#"
        @:safety
        class Main {
            static function main() {}
        }
    "#;

    let result = compile_haxe_source(source);

    if let Some(mut typed_file) = result.typed_files.into_iter().next() {
        println!("Classes: {}", typed_file.classes.len());

        for class in &typed_file.classes {
            println!("\nClass:");
            println!("  Has safety annotation: {}", class.has_safety_annotation());
            println!("  Memory annotations: {:?}", class.memory_annotations);
            println!("  Methods: {}", class.methods.len());

            for method in &class.methods {
                let name = typed_file.get_string(method.name);
                println!("    Method:");
                println!("      Name: {:?}", name);
                println!("      InternedString: {:?}", method.name);
                println!("      is_static: {}", method.is_static);
            }
        }

        println!("\nCalling detect_program_safety_mode()...");
        let mode = typed_file.detect_program_safety_mode();
        println!("Result: {:?}", mode);
    }
}
