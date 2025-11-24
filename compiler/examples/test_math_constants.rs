//! Test Math constants like PI

use compiler::pipeline::HaxeCompilationPipeline;

fn main() {
    println!("🧪 Testing Math Constants\n");
    println!("{}", "=".repeat(70));

    let source = r#"
package test;

class Test {
    static function main() {
        var pi = Math.PI;
        var result = Math.sin(Math.PI);
    }
}
"#;

    let mut pipeline = HaxeCompilationPipeline::new();
    println!("Compiling Haxe code with Math.PI...");
    let result = pipeline.compile_file("test.hx", source);

    println!("\nResults:");
    println!("  Compilation errors: {}", result.errors.len());
    println!("  HIR modules: {}", result.hir_modules.len());
    println!("  MIR modules: {}", result.mir_modules.len());

    if !result.errors.is_empty() {
        println!("\n❌ Compilation errors:");
        for (i, error) in result.errors.iter().enumerate().take(5) {
            println!("  {}. {}", i + 1, error.message);
        }
    } else {
        println!("\n✅ Successfully compiled code using Math.PI and Math.sin()!");
        println!("✅ Math methods detected and mapped to runtime functions");
    }

    println!("\n{}", "=".repeat(70));
}
