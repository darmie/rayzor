//! Direct MIR construction to isolate thread_multiple bug
//!
//! This test builds the MIR for a loop with lambda capture directly,
//! bypassing HIR/TAST to identify where the incorrect function call is generated.

use compiler::ir::mir_builder::MirBuilder;
use compiler::ir::{IrType, IrFunctionId, BinaryOp, CompareOp};
use compiler::codegen::cranelift_backend::CraneliftBackend;

fn main() {
    println!("=== Building MIR directly for loop-with-lambda scenario ===\n");

    // Build a minimal reproduction of:
    // var i = 0;
    // while (i < 5) {
    //     var handle = Thread.spawn(() -> { return i * 2; });
    //     i++;
    // }

    let mut builder = MirBuilder::new("test_loop_lambda");

    // Step 1: Create lambda function: fn lambda_0(env: i64) -> i32
    println!("Step 1: Creating lambda function...");
    let lambda_id = builder.begin_function("lambda_0")
        .param("env", IrType::I64)  // Environment pointer
        .returns(IrType::I32)
        .build();

    builder.set_current_function(lambda_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    // Lambda body: load i from env, multiply by 2, return
    let env_ptr = builder.get_param(0);
    let i_loaded = builder.load(env_ptr, IrType::I32);  // Load captured i
    let two = builder.const_i32(2);
    let result = builder.mul(i_loaded, two, IrType::I32);
    builder.ret(Some(result));

    println!("  Lambda signature: (i64) -> i32");
    println!("  Lambda body: load env[0] as i32, multiply by 2, return\n");

    // Step 2: Create Thread.spawn mock
    println!("Step 2: Creating Thread.spawn mock...");
    let thread_spawn_id = builder.begin_function("Thread_spawn")
        .param("closure_ptr", IrType::I64)  // Closure (lambda + env)
        .returns(IrType::I64)  // Thread handle
        .extern_func()
        .build();
    builder.mark_as_extern(thread_spawn_id);

    println!("  Thread.spawn signature: (i64) -> i64 [extern]\n");

    // Step 3: Create Array.push mock
    println!("Step 3: Creating Array.push mock...");
    let array_push_id = builder.begin_function("Array_push")
        .param("array_ptr", IrType::I64)
        .param("element", IrType::I64)
        .returns(IrType::Void)
        .extern_func()
        .build();
    builder.mark_as_extern(array_push_id);

    println!("  Array.push signature: (i64, i64) -> void [extern]\n");

    // Step 4: Create main function
    println!("Step 4: Creating main function with loop...");
    let main_id = builder.begin_function("main")
        .returns(IrType::I32)
        .build();

    builder.set_current_function(main_id);

    // Create blocks for loop structure
    let entry_block = builder.create_block("entry");
    let loop_header = builder.create_block("loop_header");
    let loop_body = builder.create_block("loop_body");
    let loop_exit = builder.create_block("loop_exit");

    // Entry block: initialize i = 0, create array
    builder.set_insert_point(entry_block);
    let i_init = builder.const_i32(0);
    let i_ptr = builder.alloc(IrType::I32, None);  // Stack slot for i
    builder.store(i_ptr, i_init);

    let array_ptr = builder.const_i64(0x1000);  // Mock array pointer
    builder.br(loop_header);

    // Loop header: check i < 5
    builder.set_insert_point(loop_header);
    let i_current = builder.load(i_ptr, IrType::I32);
    let five = builder.const_i32(5);
    let cond = builder.cmp(CompareOp::Lt, i_current, five);
    builder.cond_br(cond, loop_body, loop_exit);

    // Loop body: CRITICAL SECTION - where the bug likely occurs
    builder.set_insert_point(loop_body);

    println!("  Loop body construction (CRITICAL):");

    // Load i for capture
    let i_for_capture = builder.load(i_ptr, IrType::I32);
    println!("    - Load i from stack: v{}", i_for_capture.as_u32());

    // Create environment for lambda (single i32 value)
    let env_alloc = builder.alloc(IrType::I32, None);  // 4 bytes for i32
    println!("    - Allocate environment: v{}", env_alloc.as_u32());

    builder.store(env_alloc, i_for_capture);
    println!("    - Store i into environment");

    // Cast env pointer to i64 (expected by lambda)
    let env_i64 = builder.cast(env_alloc,
                                IrType::Ptr(Box::new(IrType::I32)),
                                IrType::I64);
    println!("    - Cast env pointer to i64: v{}", env_i64.as_u32());

    // THIS IS THE CRITICAL CALL - Call Thread.spawn with lambda + env
    println!("    - Calling Thread.spawn(env_i64)...");
    let handle = builder.call(thread_spawn_id, vec![env_i64]);
    println!("    - Got handle: {:?}", handle);

    // Add handle to array
    if let Some(handle_reg) = handle {
        println!("    - Calling Array.push(array, handle)...");
        builder.call(array_push_id, vec![array_ptr, handle_reg]);
    }

    // Increment i
    let i_loaded_again = builder.load(i_ptr, IrType::I32);
    let one = builder.const_i32(1);
    let i_incremented = builder.add(i_loaded_again, one, IrType::I32);
    builder.store(i_ptr, i_incremented);
    println!("    - Increment i and loop back\n");

    builder.br(loop_header);

    // Loop exit: return 0
    builder.set_insert_point(loop_exit);
    let zero = builder.const_i32(0);
    builder.ret(Some(zero));

    println!("Step 5: MIR construction complete!\n");

    // Step 6: Validate and compile to Cranelift
    println!("=== Compiling to Cranelift IR ===\n");
    let module = builder.finish();

    // Print MIR summary
    println!("MIR Module: {}", module.name);
    println!("Functions:");
    for (id, func) in &module.functions {
        println!("  {} (u0:{}): {} -> {:?}",
                 func.name,
                 id.0,
                 func.signature.parameters.iter()
                     .map(|p| format!("{:?}", p.ty))
                     .collect::<Vec<_>>()
                     .join(", "),
                 func.signature.return_type);
    }
    println!();

    // Compile with Cranelift
    let mut backend = match CraneliftBackend::new() {
        Ok(b) => b,
        Err(e) => {
            println!("❌ Failed to initialize Cranelift backend: {}", e);
            return;
        }
    };

    match backend.compile_module(&module) {
        Ok(()) => {
            println!("✅ Cranelift compilation succeeded!");
            println!("\n=== ANALYSIS ===");
            println!("If this succeeded, the bug is NOT in direct MIR → Cranelift.");
            println!("The bug must be in HIR → MIR lowering for Thread.spawn calls.");
        }
        Err(e) => {
            println!("❌ Cranelift compilation failed: {}", e);
            println!("\n=== ANALYSIS ===");
            println!("If this failed, the bug reproduces in direct MIR construction.");
            println!("Check the Cranelift IR above for function signature mismatches.");
        }
    }

    println!("\n=== EXPECTED vs ACTUAL ===");
    println!("EXPECTED in loop body:");
    println!("  v_env = alloc i32");
    println!("  store i_current, v_env");
    println!("  v_env_i64 = cast v_env to i64");
    println!("  v_handle = call Thread_spawn(v_env_i64)  ← Should call fn2 (Thread.spawn)");
    println!("  call Array_push(array, v_handle)");
    println!();
    println!("ACTUAL in test_rayzor_stdlib_e2e (BUGGY):");
    println!("  v15 = call fn1(v8)  ← Calls wrong function (fn1 instead of Thread.spawn)");
    println!("  where fn1 has signature () -> i64 (wrong!)");
}
