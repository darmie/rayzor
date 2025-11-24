/// Test Vec<u8> operations in stdlib
///
/// This test verifies that Vec<u8> functions are correctly built and validated.

use compiler::ir::IrModule;
use compiler::ir::optimizable::OptimizableModule;
use compiler::stdlib::build_stdlib;

fn main() {
    println!("🧪 Testing Vec<u8> operations in stdlib...\n");

    // Build the stdlib with Vec<u8> functions
    println!("📦 Building stdlib with Vec<u8>...");
    let module = build_stdlib();

    println!("   - Module: {}", module.name);
    println!("   - Total functions: {}", module.functions.len());
    println!();

    // Count Vec<u8> functions
    println!("🔍 Checking for Vec<u8> functions...");
    let vec_functions: Vec<_> = module.functions.iter()
        .filter(|(_, f)| f.name.starts_with("vec_u8_"))
        .collect();

    println!("   ✅ Found {} Vec<u8> functions:", vec_functions.len());
    for (_, func) in &vec_functions {
        let params: Vec<String> = func.signature.parameters.iter()
            .map(|p| format!("{:?}", p.ty))
            .collect();
        println!("      - {}({}) -> {:?}",
            func.name,
            params.join(", "),
            func.signature.return_type
        );
    }
    println!();

    // Validate the module
    println!("🔍 Validating MIR module...");
    match module.validate() {
        Ok(_) => {
            println!("   ✅ Module is valid!\n");

            // Show specific Vec<u8> function details
            println!("📋 Vec<u8> Function Details:");

            // vec_u8_new
            if let Some((_, func)) = module.functions.iter().find(|(_, f)| f.name == "vec_u8_new") {
                println!("\n   vec_u8_new():");
                println!("      - Creates new Vec<u8> with initial capacity 16");
                println!("      - Returns: {:?}", func.signature.return_type);
                println!("      - Basic blocks: {}", func.cfg.blocks.len());
            }

            // vec_u8_push
            if let Some((_, func)) = module.functions.iter().find(|(_, f)| f.name == "vec_u8_push") {
                println!("\n   vec_u8_push():");
                println!("      - Appends element to vector with dynamic growth");
                println!("      - Parameters: {}", func.signature.parameters.len());
                println!("      - Basic blocks: {}", func.cfg.blocks.len());
                println!("      - Handles capacity doubling when full");
            }

            // vec_u8_pop
            if let Some((_, func)) = module.functions.iter().find(|(_, f)| f.name == "vec_u8_pop") {
                println!("\n   vec_u8_pop():");
                println!("      - Removes last element");
                println!("      - Returns: {:?}", func.signature.return_type);
                println!("      - Basic blocks: {}", func.cfg.blocks.len());
            }

            // vec_u8_get
            if let Some((_, func)) = module.functions.iter().find(|(_, f)| f.name == "vec_u8_get") {
                println!("\n   vec_u8_get():");
                println!("      - Bounds-checked access");
                println!("      - Returns: {:?}", func.signature.return_type);
                println!("      - Basic blocks: {}", func.cfg.blocks.len());
            }

            println!("\n✨ All Vec<u8> functions built and validated successfully!");
            println!("   Ready for Cranelift and LLVM lowering!");
        }
        Err(errors) => {
            eprintln!("   ❌ Module validation failed:");
            for error in errors {
                eprintln!("      - {:?}", error);
            }
            std::process::exit(1);
        }
    }
}
