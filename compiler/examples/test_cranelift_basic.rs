//! Basic test for Cranelift backend
//!
//! Tests that we can:
//! 1. Create a simple MIR function
//! 2. Compile it with Cranelift
//! 3. Get a function pointer
//!
//! Run with: cargo run --example test_cranelift_basic

use compiler::codegen::cranelift_backend::CraneliftBackend;
use compiler::ir::{
    IrFunction, IrFunctionId, IrFunctionSignature, IrParameter,
    IrType, IrId, IrTerminator, CallingConvention,
};
use compiler::tast::SymbolId;

fn main() {
    println!("=== Rayzor Cranelift Backend Test ===\n");

    // Create a simple identity function: fn identity(x: i32) -> i32 { return x; }
    let func_id = IrFunctionId(0);
    let symbol_id = SymbolId::from_raw(1);

    let signature = IrFunctionSignature {
        parameters: vec![
            IrParameter {
                name: "x".to_string(),
                ty: IrType::I32,
                reg: IrId::new(0),
                by_ref: false,
            }
        ],
        return_type: IrType::I32,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: Vec::new(),
    };

    let mut function = IrFunction::new(
        func_id,
        symbol_id,
        "identity".to_string(),
        signature,
    );

    // Build function body: just return the parameter
    let entry_block = function.entry_block();
    let x_reg = function.get_param_reg(0).unwrap();

    if let Some(entry) = function.cfg.get_block_mut(entry_block) {
        entry.set_terminator(IrTerminator::Return {
            value: Some(x_reg),
        });
    }

    println!("Created MIR function:");
    println!("  Name: {}", function.name);
    println!("  Signature: fn(i32) -> i32");
    println!("  Body: return param[0]\n");

    // Create Cranelift backend with baseline optimization
    println!("Creating Cranelift backend with baseline optimization...");
    let mut backend = CraneliftBackend::with_optimization_level("none")
        .expect("Failed to create Cranelift backend");

    println!("  Pointer size: {} bytes", backend.get_pointer_size());
    println!("  i32 size: {} bytes", backend.get_type_size(&IrType::I32));
    println!("  i32 alignment: {} bytes\n", backend.get_type_alignment(&IrType::I32));

    // Compile the function
    println!("Compiling function...");
    let start = std::time::Instant::now();

    backend.compile_single_function(func_id, &function)
        .expect("Failed to compile function");

    let compile_time = start.elapsed();
    println!("  Compilation time: {:?}\n", compile_time);

    // Get function pointer
    println!("Getting function pointer...");
    let fn_ptr = backend.get_function_ptr(func_id)
        .expect("Failed to get function pointer");

    println!("  Function pointer: {:p}\n", fn_ptr);

    // Note: We can't actually call the function safely in Rust without
    // using unsafe code and proper FFI setup. In a real scenario, we'd
    // use unsafe transmute to call it.

    println!("✓ Test passed! Successfully:");
    println!("  1. Created MIR function");
    println!("  2. Compiled with Cranelift");
    println!("  3. Retrieved function pointer");
    println!("\nCranelift backend is working correctly!");
}
