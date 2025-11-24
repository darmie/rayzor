use compiler::pipeline::compile_haxe_file;
use std::fs;

fn main() {
    env_logger::init();

    let test_file = "test_property.hx";

    println!("Testing property access with custom getter...");
    println!("Compiling: {}", test_file);

    // Read the source file
    let source = fs::read_to_string(test_file)
        .expect("Failed to read test file");

    let result = compile_haxe_file(test_file, &source);

    println!("✅ Compilation successful!");

    if !result.errors.is_empty() {
        println!("\n⚠️  Compilation had warnings/errors:");
        for error in &result.errors {
            println!("  {:?}", error);
        }
    }

    // Check MIR modules
    println!("\nMIR Stats:");
    println!("  Modules: {}", result.mir_modules.len());

    for mir_module in &result.mir_modules {
        println!("\n  Module: {}", mir_module.name);
        println!("    Functions: {}", mir_module.functions.len());
        println!("    Extern functions: {}", mir_module.extern_functions.len());

        // Print function names
        println!("\n    Functions:");
        for (_func_id, func) in &mir_module.functions {
            println!("      - {}", func.name);
        }
    }
}
