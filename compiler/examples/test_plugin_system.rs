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
//! Test the plugin-based runtime system with pointer-based Vec API

use compiler::codegen::cranelift_backend::CraneliftBackend;
use compiler::ir::mir_builder::MirBuilder;
use compiler::ir::{CallingConvention, IrType, StructField};
use compiler::plugin::PluginRegistry;

fn main() {
    println!("🚀 Testing Plugin-Based Runtime System\n");

    // Step 1: Set up plugin registry and collect symbols
    println!("📋 Step 1: Registering runtime plugins...");
    let mut registry = PluginRegistry::new();

    // Register the Rayzor runtime plugin
    registry
        .register(rayzor_runtime::get_plugin())
        .expect("Failed to register rayzor_runtime plugin");

    println!("  ✓ Registered plugins: {:?}\n", registry.list_plugins());

    // Collect all runtime symbols
    let symbols = registry.collect_symbols();
    println!("  ✓ Collected {} runtime symbols\n", symbols.len());

    // Step 2: Build MIR module
    println!("📋 Step 2: Building MIR module...");
    let mut builder = MirBuilder::new("test_plugin");

    // Define types
    let u8_ty = builder.u8_type();
    let u64_ty = builder.u64_type();
    let ptr_u8_ty = builder.ptr_type(u8_ty.clone());
    let ptr_vec_ty = builder.ptr_type(IrType::Struct {
        name: "HaxeVec".to_string(),
        fields: vec![
            StructField {
                name: "ptr".to_string(),
                ty: ptr_u8_ty.clone(),
                offset: 0,
            },
            StructField {
                name: "len".to_string(),
                ty: u64_ty.clone(),
                offset: 8,
            },
            StructField {
                name: "cap".to_string(),
                ty: u64_ty.clone(),
                offset: 16,
            },
        ],
    });

    // Declare extern functions (pointer-based API - no struct returns!)
    let vec_new_id = builder
        .begin_function("haxe_vec_new_ptr")
        .param("out", ptr_vec_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(vec_new_id);

    let vec_push_id = builder
        .begin_function("haxe_vec_push_ptr")
        .param("vec", ptr_vec_ty.clone())
        .param("value", u8_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(vec_push_id);

    let vec_len_id = builder
        .begin_function("haxe_vec_len_ptr")
        .param("vec", ptr_vec_ty.clone())
        .returns(u64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(vec_len_id);

    // Create test function: fn test_vec() -> u64
    let test_func_id = builder
        .begin_function("test_vec")
        .returns(u64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(test_func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    // Allocate stack space for the vec struct
    let vec_ty = IrType::Struct {
        name: "HaxeVec".to_string(),
        fields: vec![
            StructField {
                name: "ptr".to_string(),
                ty: ptr_u8_ty.clone(),
                offset: 0,
            },
            StructField {
                name: "len".to_string(),
                ty: u64_ty.clone(),
                offset: 8,
            },
            StructField {
                name: "cap".to_string(),
                ty: u64_ty.clone(),
                offset: 16,
            },
        ],
    };
    let vec_ptr = builder.alloc(vec_ty, None);

    // Call haxe_vec_new_ptr(vec_ptr) - initializes the struct
    builder.call(vec_new_id, vec![vec_ptr]);

    // Push some values
    let val_42 = builder.const_u8(42);
    let val_100 = builder.const_u8(100);
    let val_200 = builder.const_u8(200);

    builder.call(vec_push_id, vec![vec_ptr, val_42]);
    builder.call(vec_push_id, vec![vec_ptr, val_100]);
    builder.call(vec_push_id, vec![vec_ptr, val_200]);

    // Get the length
    let length = builder.call(vec_len_id, vec![vec_ptr]).unwrap();

    // Return the length (should be 3)
    builder.ret(Some(length));

    println!("  ✓ Created test function with pointer-based Vec API\n");

    // Get the module
    let mir_module = builder.finish();

    // Step 3: Compile with Cranelift
    println!("📋 Step 3: Compiling with Cranelift JIT...");
    let mut backend = CraneliftBackend::with_symbols(&symbols).unwrap();

    match backend.compile_module(&mir_module) {
        Ok(_) => println!("  ✓ Compilation successful!\n"),
        Err(e) => {
            eprintln!("  ✗ Compilation failed: {:?}\n", e);
            return;
        }
    }

    // Step 4: Execute
    println!("📋 Step 4: Getting function pointer...");
    let func_ptr = backend.get_function_ptr(test_func_id).unwrap();
    println!("  ✓ Got function pointer: {:p}\n", func_ptr);

    println!("📋 Step 5: Executing JIT-compiled function...");
    let test_fn: extern "C" fn() -> u64 = unsafe { std::mem::transmute(func_ptr) };
    let result = test_fn();

    println!("\n🎉 Test completed!");
    println!("   Result: {}", result);
    println!("   Expected: 3");

    if result == 3 {
        println!("\n✅ SUCCESS! Plugin system working correctly!");
    } else {
        println!("\n❌ FAILED! Expected 3, got {}", result);
        std::process::exit(1);
    }
}
