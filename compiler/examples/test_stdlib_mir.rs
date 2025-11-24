//! Test MIR-based standard library
//!
//! This example demonstrates that the MIR builder successfully constructs
//! a standard library with extern functions that can be lowered to Cranelift.

use compiler::stdlib::build_stdlib;
use compiler::ir::IrModule;

fn main() {
    println!("🔧 Building MIR-based standard library...\n");

    // Build the stdlib using MIR builder
    let stdlib: IrModule = build_stdlib();

    println!("✅ Successfully built stdlib module: {}", stdlib.name);
    println!("📊 Statistics:");
    println!("   - Functions: {}", stdlib.functions.len());
    println!("   - Globals: {}", stdlib.globals.len());
    println!("   - Type definitions: {}", stdlib.types.len());

    println!("\n📋 Exported Functions:");
    for (_func_id, func) in &stdlib.functions {
        let visibility = if matches!(func.attributes.linkage, compiler::ir::Linkage::Public) {
            "public"
        } else if matches!(func.attributes.linkage, compiler::ir::Linkage::External) {
            "external"
        } else {
            "private"
        };

        let kind = if func.cfg.blocks.is_empty() {
            "extern"
        } else {
            "defined"
        };

        let params: Vec<String> = func.signature.parameters
            .iter()
            .map(|p| format!("{}: {:?}", p.name, p.ty))
            .collect();

        println!(
            "   - {} {} {}({}) -> {:?}",
            visibility,
            kind,
            func.name,
            params.join(", "),
            func.signature.return_type
        );
    }

    println!("\n🎯 Key Functions:");

    // Check for trace function
    if let Some(trace_func) = stdlib.functions.values().find(|f| f.name == "trace") {
        println!("   ✓ trace() - Haxe's standard output function");
        println!("     Calling convention: {:?}", trace_func.signature.calling_convention);
    }

    // Check for string functions
    let string_funcs: Vec<_> = stdlib.functions.values()
        .filter(|f| f.name.starts_with("string_"))
        .map(|f| &f.name)
        .collect();

    if !string_funcs.is_empty() {
        println!("   ✓ String operations ({}):", string_funcs.len());
        for name in &string_funcs {
            println!("     - {}", name);
        }
    }

    // Check for array functions
    let array_funcs: Vec<_> = stdlib.functions.values()
        .filter(|f| f.name.starts_with("array_"))
        .map(|f| &f.name)
        .collect();

    if !array_funcs.is_empty() {
        println!("   ✓ Array operations ({}):", array_funcs.len());
        for name in &array_funcs {
            println!("     - {}", name);
        }
    }

    // Verify the module is valid
    println!("\n🔍 Validating MIR module...");

    // Check each function individually to find which one has errors
    let mut has_errors = false;
    for (func_id, func) in &stdlib.functions {
        if let Err(e) = func.verify() {
            eprintln!("   ❌ Function '{}' (id={:?}): {}", func.name, func_id, e);
            has_errors = true;
        }
    }

    if has_errors {
        eprintln!("\n   ❌ Some functions failed validation");
        std::process::exit(1);
    }

    match stdlib.verify() {
        Ok(()) => println!("   ✅ Module is valid!"),
        Err(e) => {
            eprintln!("   ❌ Validation error: {}", e);
            std::process::exit(1);
        }
    }

    println!("\n✨ MIR stdlib is ready for Cranelift and LLVM lowering!");
}
