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
//! Test Rust stdlib functions called from Cranelift JIT-compiled code

extern crate rayzor_runtime;

use compiler::codegen::cranelift_backend::CraneliftBackend;
use compiler::ir::mir_builder::MirBuilder;
use compiler::ir::{CallingConvention, IrType, StructField};

fn main() {
    println!("🚀 Testing Rust Stdlib via Cranelift JIT");
    println!();

    // Create a simple MIR module that calls Rust stdlib
    let mut builder = MirBuilder::new("test_stdlib");

    // Define the HaxeVec type: struct { ptr: *u8, len: u64, cap: u64 }
    let u8_ty = builder.u8_type();
    let u64_ty = builder.u64_type();
    let ptr_u8_ty = builder.ptr_type(u8_ty.clone());

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

    // Declare extern function: haxe_vec_new() -> HaxeVec
    let vec_new_id = builder
        .begin_function("haxe_vec_new")
        .returns(vec_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(vec_new_id);

    // Declare extern function: haxe_vec_push(vec: *HaxeVec, value: u8)
    let void_ty = builder.void_type();
    let ptr_vec_ty = builder.ptr_type(vec_ty.clone());
    let vec_push_id = builder
        .begin_function("haxe_vec_push")
        .param("vec", ptr_vec_ty.clone())
        .param("value", u8_ty.clone())
        .returns(void_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(vec_push_id);

    // Declare extern function: haxe_vec_len(vec: *HaxeVec) -> u64
    let vec_len_id = builder
        .begin_function("haxe_vec_len")
        .param("vec", ptr_vec_ty.clone())
        .returns(u64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(vec_len_id);

    // Declare extern function: haxe_vec_get(vec: *HaxeVec, index: u64) -> u8
    let vec_get_id = builder
        .begin_function("haxe_vec_get")
        .param("vec", ptr_vec_ty.clone())
        .param("index", u64_ty.clone())
        .returns(u8_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(vec_get_id);

    // Create a test function that uses the vec
    // fn test_vec() -> u64
    let test_func_id = builder
        .begin_function("test_vec")
        .returns(u64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(test_func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    // Call haxe_vec_new() -> vec (returns struct by value with sret)
    let vec_value = builder.call(vec_new_id, vec![]).unwrap();

    // Allocate stack space for the vec struct (single instance)
    let vec_ptr = builder.alloc(vec_ty.clone(), None);
    builder.store(vec_value, vec_ptr);

    // Push some values: vec.push(42), vec.push(100), vec.push(200)
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

    println!("📋 Step 1: Created MIR module");
    println!("  - Declared 4 extern functions (vec_new, vec_push, vec_len, vec_get)");
    println!("  - Created test_vec() function that:");
    println!("    1. Creates a new Vec");
    println!("    2. Pushes 3 values (42, 100, 200)");
    println!("    3. Returns the length");
    println!();

    // Get the module
    let mir_module = builder.finish();

    println!("📋 Step 2: Compiling MIR to native code with Cranelift...");
    let mut backend = CraneliftBackend::new().unwrap();

    match backend.compile_module(&mir_module) {
        Ok(_) => println!("  ✓ Compilation successful!"),
        Err(e) => {
            eprintln!("  ✗ Compilation failed: {:?}", e);
            return;
        }
    }
    println!();

    println!("📋 Step 3: Getting function pointer...");
    let test_func_ptr = backend.get_function_ptr(test_func_id).unwrap();
    println!("  ✓ Got function pointer: {:p}", test_func_ptr);
    println!();

    println!("📋 Step 4: Calling JIT-compiled function...");

    // Cast to correct function type
    type TestFn = unsafe extern "C" fn() -> u64;
    let test_fn = unsafe { std::mem::transmute::<*const u8, TestFn>(test_func_ptr) };

    unsafe {
        let result = test_fn();
        println!("  ✓ Function returned: {}", result);
        println!();

        if result == 3 {
            println!("✅ SUCCESS! The JIT-compiled code successfully:");
            println!("   1. Called haxe_vec_new() to create a Vec");
            println!("   2. Called haxe_vec_push() three times");
            println!("   3. Called haxe_vec_len() and got the correct length (3)");
            println!();
            println!("🎉 Rust stdlib integration with Cranelift JIT WORKS!");
        } else {
            println!("❌ FAILED! Expected length 3, got {}", result);
        }
    }
}
