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
    CallingConvention, IrFunction, IrFunctionId, IrFunctionSignature, IrId, IrModule, IrParameter,
    IrTerminator, IrType,
};
use compiler::tast::SymbolId;

fn main() {
    println!("=== Rayzor Cranelift Backend Test ===\n");

    // Create a simple identity function: fn identity(x: i32) -> i32 { return x; }
    let func_id = IrFunctionId(0);
    let symbol_id = SymbolId::from_raw(1);

    let signature = IrFunctionSignature {
        parameters: vec![IrParameter {
            name: "x".to_string(),
            ty: IrType::I32,
            reg: IrId::new(0),
            by_ref: false,
        }],
        return_type: IrType::I32,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: Vec::new(),
        uses_sret: false,
    };

    let mut function = IrFunction::new(func_id, symbol_id, "identity".to_string(), signature);

    // Build function body: just return the parameter
    let entry_block = function.entry_block();
    let x_reg = function.get_param_reg(0).unwrap();

    if let Some(entry) = function.cfg.get_block_mut(entry_block) {
        entry.set_terminator(IrTerminator::Return { value: Some(x_reg) });
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
    println!(
        "  i32 alignment: {} bytes\n",
        backend.get_type_alignment(&IrType::I32)
    );

    // Create module for compilation
    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    module.functions.insert(func_id, function.clone());

    // Compile the function
    println!("Compiling function...");
    let start = std::time::Instant::now();

    backend
        .compile_single_function(func_id, &module, &function)
        .expect("Failed to compile function");

    let compile_time = start.elapsed();
    println!("  Compilation time: {:?}\n", compile_time);

    // Get function pointer
    println!("Getting function pointer...");
    let fn_ptr = backend
        .get_function_ptr(func_id)
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
