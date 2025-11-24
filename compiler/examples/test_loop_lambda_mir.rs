/// Minimal test for loop + lambda at MIR level
///
/// This test creates MIR directly to isolate the loop+lambda issue
/// without going through TAST/HIR lowering

use compiler::ir::*;
use compiler::ir::builder::MirBuilder;
use compiler::ir::types::IrType;
use cranelift_jit_backend::CraneliftBackend;
use std::collections::HashMap;

fn main() {
    println!("\n=== Testing Loop + Lambda at MIR Level ===\n");

    // Create a simple test: loop that creates lambdas capturing loop variable
    let mut builder = MirBuilder::new();

    // Build lambda function: fn lambda(env: *u8) -> i32
    // The env contains captured variable `i`
    let lambda_id = builder.begin_function("<lambda_0>")
        .param("env", builder.ptr_type(builder.u8_type()))
        .returns(builder.i32_type())
        .build();

    builder.set_current_function(lambda_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    // Load `i` from environment (env[0] is pointer to i)
    let env_ptr = builder.get_param(0);
    let i_ptr_ptr = builder.ptr_add(env_ptr, builder.const_i64(0), builder.ptr_type(builder.u8_type()));
    let i_ptr = builder.load(i_ptr_ptr, builder.ptr_type(builder.u8_type()));
    let i_val = builder.load(i_ptr, builder.i32_type());

    // Return i * 2
    let two = builder.const_i32(2);
    let result = builder.mul(i_val, two, builder.i32_type());
    builder.ret(Some(result));

    // Build main function
    let main_id = builder.begin_function("main")
        .returns(builder.void_type())
        .build();

    builder.set_current_function(main_id);
    let main_entry = builder.create_block("entry");
    let loop_header = builder.create_block("loop_header");
    let loop_body = builder.create_block("loop_body");
    let loop_exit = builder.create_block("loop_exit");

    builder.set_insert_point(main_entry);

    // Allocate `i` on stack
    let i_ptr = builder.alloca(builder.i32_type(), 4);
    let zero = builder.const_i32(0);
    builder.store(zero, i_ptr);

    builder.br(loop_header);

    // Loop header: check i < 3
    builder.set_insert_point(loop_header);
    let i_current = builder.load(i_ptr, builder.i32_type());
    let three = builder.const_i32(3);
    let cond = builder.icmp(i_current, three, builder.i32_type(), "slt");
    builder.br_cond(cond, loop_body, loop_exit);

    // Loop body: create lambda capturing i
    builder.set_insert_point(loop_body);

    // Allocate environment (just contains pointer to i)
    let env_size = builder.const_i64(8);
    let malloc_id = builder.get_or_create_extern_function(
        "malloc",
        vec![builder.i64_type()],
        builder.ptr_type(builder.u8_type())
    );
    let env_ptr = builder.call(malloc_id, vec![env_size]).unwrap();

    // Store pointer to i in environment
    let i_ptr_storage = builder.ptr_add(env_ptr, builder.const_i64(0), builder.ptr_type(builder.u8_type()));
    builder.store(i_ptr, i_ptr_storage);

    // Call lambda with environment
    let lambda_result = builder.call(lambda_id, vec![env_ptr]).unwrap();

    // For now, just ignore the result
    // In real code we'd store it somewhere

    // Increment i
    let one = builder.const_i32(1);
    let i_next = builder.add(i_current, one, builder.i32_type());
    builder.store(i_next, i_ptr);

    builder.br(loop_header);

    // Loop exit
    builder.set_insert_point(loop_exit);
    builder.ret(None);

    // Finalize module
    let module = builder.finalize();

    println!("MIR Module created with {} functions:", module.functions.len());
    for (func_id, func) in &module.functions {
        println!("  - {:?}: {} ({} blocks)", func_id, func.name, func.cfg.blocks.len());
    }

    // Try to compile to Cranelift
    println!("\n=== Compiling to Cranelift ===\n");

    let module_arc = std::sync::Arc::new(module);

    match CraneliftBackend::new() {
        Ok(mut backend) => {
            match backend.compile_module(&module_arc) {
                Ok(()) => {
                    println!("✅ Cranelift compilation succeeded!");

                    // Try to execute
                    println!("\n=== Executing main() ===\n");
                    match backend.call_main(&module_arc) {
                        Ok(()) => {
                            println!("✅ Execution succeeded!");
                        }
                        Err(e) => {
                            println!("❌ Execution failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Cranelift compilation failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to create Cranelift backend: {}", e);
            std::process::exit(1);
        }
    }
}
