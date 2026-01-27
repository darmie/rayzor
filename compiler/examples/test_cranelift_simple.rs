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
/// Test Cranelift backend with a simple function that returns 42
///
/// This example creates a minimal MIR function and compiles it with Cranelift.
use compiler::codegen::CraneliftBackend;
use compiler::ir::*;
use compiler::tast::SymbolId;

fn main() -> Result<(), String> {
    println!("=== Cranelift Backend Test: Return 42 ===\n");

    // Create a simple MIR function that returns 42
    let func_id = IrFunctionId(0);
    let symbol_id = SymbolId::from_raw(1);

    // Create function signature: () -> i64
    let signature = IrFunctionSignature {
        parameters: vec![],
        return_type: IrType::I64,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
        uses_sret: false,
    };

    // Create the function using constructor
    let function = IrFunction::new(func_id, symbol_id, "return_42".to_string(), signature);

    // Create MIR module
    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    module.functions.insert(func_id, function);

    println!("Created MIR function:");
    println!("  Name: return_42");
    println!("  Signature: () -> i64");
    println!("  Body: Currently placeholder (returns 42)\n");

    // Initialize Cranelift backend
    println!("Initializing Cranelift backend...");
    let mut backend = CraneliftBackend::new()?;
    println!("✅ Backend initialized\n");

    // Compile the module
    println!("Compiling MIR → Cranelift IR...");
    backend.compile_module(&module)?;
    println!("✅ Compilation successful\n");

    // Get the function pointer
    println!("Retrieving function pointer...");
    let func_ptr = backend.get_function_ptr(func_id)?;
    println!("✅ Function pointer: {:p}\n", func_ptr);

    // Cast and execute the function
    println!("Executing JIT-compiled function...");
    let func: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result = func();

    println!("✅ Execution complete");
    println!("\nResult: {}", result);

    // Verify result
    if result == 42 {
        println!("\n🎉 SUCCESS: Function returned expected value (42)!");
        Ok(())
    } else {
        Err(format!("FAILED: Expected 42, got {}", result))
    }
}
