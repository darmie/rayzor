/// Memory management functions - Runtime-provided allocator
///
/// This module provides heap allocation functions that will be implemented
/// by runtime function hooks when the JIT executes the code.
///
/// ## Architecture:
/// 1. MIR functions mark malloc/realloc/free as "runtime intrinsics"
/// 2. Cranelift/LLVM backends recognize these and emit calls to runtime-provided functions
/// 3. The JIT runtime (in Rust) provides the actual allocation implementations
///
/// This keeps everything pure Rust - no C linking required!
use crate::ir::mir_builder::MirBuilder;
use crate::ir::{BinaryOp, CallingConvention, CompareOp, IrType, UnionVariant};

/// Build all memory management functions
pub fn build_memory_functions(builder: &mut MirBuilder) {
    // These will be recognized as runtime intrinsics by the backend
    build_heap_alloc(builder);
    build_heap_realloc(builder);
    build_heap_free(builder);

    // Safe wrappers with null checks
    build_safe_allocate(builder);
    build_safe_reallocate(builder);
    build_safe_deallocate(builder);
}

/// Build: fn malloc(size: u64) -> *u8
///
/// Extern declaration - will link to libc malloc
fn build_heap_alloc(builder: &mut MirBuilder) {
    let u64_ty = builder.u64_type();
    let u8_ty = builder.u8_type();
    let ptr_u8_ty = builder.ptr_type(u8_ty.clone());

    let func_id = builder
        .begin_function("malloc")
        .param("size", u64_ty.clone())
        .returns(ptr_u8_ty.clone())
        .calling_convention(CallingConvention::C) // Use C calling convention for libc
        .build();

    // Mark as extern by clearing CFG blocks
    builder.mark_as_extern(func_id);
}

/// Build: fn realloc(ptr: *u8, new_size: u64) -> *u8
///
/// Standard libc realloc - takes ptr and new size
fn build_heap_realloc(builder: &mut MirBuilder) {
    let u8_ty = builder.u8_type();
    let u64_ty = builder.u64_type();
    let ptr_u8_ty = builder.ptr_type(u8_ty.clone());

    let func_id = builder
        .begin_function("realloc")
        .param("ptr", ptr_u8_ty.clone())
        .param("new_size", u64_ty.clone())
        .returns(ptr_u8_ty.clone())
        .calling_convention(CallingConvention::C) // Use C calling convention for libc
        .build();

    // Mark as extern by clearing CFG blocks
    builder.mark_as_extern(func_id);
}

/// Build: fn free(ptr: *u8)
///
/// Standard libc free - takes only ptr
fn build_heap_free(builder: &mut MirBuilder) {
    let u8_ty = builder.u8_type();
    let ptr_u8_ty = builder.ptr_type(u8_ty);
    let void_ty = builder.void_type();

    let func_id = builder
        .begin_function("free")
        .param("ptr", ptr_u8_ty)
        .returns(void_ty.clone())
        .calling_convention(CallingConvention::C) // Use C calling convention for libc
        .build();

    // Mark as extern by clearing CFG blocks
    builder.mark_as_extern(func_id);
}

/// Build: fn allocate(size: u64) -> Option<*u8>
/// Safe wrapper around malloc that returns None on failure (null pointer)
fn build_safe_allocate(builder: &mut MirBuilder) {
    let u8_ty = builder.u8_type();
    let u64_ty = builder.u64_type();
    let ptr_u8_ty = builder.ptr_type(u8_ty.clone());
    let void_ty = builder.void_type();
    let bool_ty = builder.bool_type();

    // Build Option<*u8> type
    let option_ptr_ty = builder.union_type(
        Some("Option_ptr_u8"),
        vec![
            UnionVariant {
                name: "None".to_string(),
                tag: 0,
                fields: vec![void_ty.clone()],
            },
            UnionVariant {
                name: "Some".to_string(),
                tag: 1,
                fields: vec![ptr_u8_ty.clone()],
            },
        ],
    );

    let func_id = builder
        .begin_function("allocate")
        .param("size", u64_ty.clone())
        .returns(option_ptr_ty.clone())
        .build();

    builder.set_current_function(func_id);

    // Create blocks
    let entry = builder.create_block("entry");
    let call_malloc = builder.create_block("call_malloc");
    let check_null = builder.create_block("check_null");
    let success_block = builder.create_block("success");
    let failure_block = builder.create_block("failure");

    // Entry: get size and jump to call
    builder.set_insert_point(entry);
    let size = builder.get_param(0);
    builder.br(call_malloc);

    // Call malloc
    builder.set_insert_point(call_malloc);
    let malloc_id = builder
        .get_function_by_name("malloc")
        .expect("malloc not found");
    let ptr = builder.call(malloc_id, vec![size]).unwrap();
    builder.br(check_null);

    // Check if null (pointer value == 0)
    builder.set_insert_point(check_null);
    let null = builder.const_u64(0);
    let ptr_as_u64 = builder.cast(ptr, ptr_u8_ty.clone(), u64_ty.clone());
    let is_null = builder.icmp(CompareOp::Eq, ptr_as_u64, null, bool_ty);
    builder.cond_br(is_null, failure_block, success_block);

    // Success: return Some(ptr)
    builder.set_insert_point(success_block);
    let some_value = builder.create_union(1, ptr, option_ptr_ty.clone());
    builder.ret(Some(some_value));

    // Failure: return None
    builder.set_insert_point(failure_block);
    let unit = builder.unit_value();
    let none_value = builder.create_union(0, unit, option_ptr_ty);
    builder.ret(Some(none_value));
}

/// Build: fn reallocate(ptr: *u8, old_size: u64, new_size: u64) -> Option<*u8>
/// Safe wrapper around realloc that returns None on failure
fn build_safe_reallocate(builder: &mut MirBuilder) {
    let u8_ty = builder.u8_type();
    let u64_ty = builder.u64_type();
    let ptr_u8_ty = builder.ptr_type(u8_ty.clone());
    let void_ty = builder.void_type();
    let bool_ty = builder.bool_type();

    // Build Option<*u8> type
    let option_ptr_ty = builder.union_type(
        Some("Option_ptr_u8"),
        vec![
            UnionVariant {
                name: "None".to_string(),
                tag: 0,
                fields: vec![void_ty.clone()],
            },
            UnionVariant {
                name: "Some".to_string(),
                tag: 1,
                fields: vec![ptr_u8_ty.clone()],
            },
        ],
    );

    let func_id = builder
        .begin_function("reallocate")
        .param("ptr", ptr_u8_ty.clone())
        .param("old_size", u64_ty.clone())
        .param("new_size", u64_ty.clone())
        .returns(option_ptr_ty.clone())
        .build();

    builder.set_current_function(func_id);

    // Create blocks
    let entry = builder.create_block("entry");
    let call_realloc = builder.create_block("call_realloc");
    let check_null = builder.create_block("check_null");
    let success_block = builder.create_block("success");
    let failure_block = builder.create_block("failure");

    // Entry: get parameters and jump to call
    builder.set_insert_point(entry);
    let ptr = builder.get_param(0);
    let old_size = builder.get_param(1);
    let new_size = builder.get_param(2);
    builder.br(call_realloc);

    // Call realloc with old_size and new_size
    builder.set_insert_point(call_realloc);
    let realloc_id = builder
        .get_function_by_name("realloc")
        .expect("realloc not found");
    // libc realloc takes only 2 params: ptr and new_size
    let new_ptr = builder.call(realloc_id, vec![ptr, new_size]).unwrap();
    builder.br(check_null);

    // Check if null
    builder.set_insert_point(check_null);
    let null = builder.const_u64(0);
    let ptr_as_u64 = builder.cast(new_ptr, ptr_u8_ty.clone(), u64_ty.clone());
    let is_null = builder.icmp(CompareOp::Eq, ptr_as_u64, null, bool_ty);
    builder.cond_br(is_null, failure_block, success_block);

    // Success: return Some(new_ptr)
    builder.set_insert_point(success_block);
    let some_value = builder.create_union(1, new_ptr, option_ptr_ty.clone());
    builder.ret(Some(some_value));

    // Failure: return None
    builder.set_insert_point(failure_block);
    let unit = builder.unit_value();
    let none_value = builder.create_union(0, unit, option_ptr_ty);
    builder.ret(Some(none_value));
}

/// Build: fn deallocate(ptr: *u8) -> void
/// Safe wrapper around free
fn build_safe_deallocate(builder: &mut MirBuilder) {
    let u8_ty = builder.u8_type();
    let ptr_u8_ty = builder.ptr_type(u8_ty);
    let void_ty = builder.void_type();

    let func_id = builder
        .begin_function("deallocate")
        .param("ptr", ptr_u8_ty)
        .returns(void_ty.clone())
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    // Get ptr parameter
    let ptr = builder.get_param(0);

    // Call free (libc signature: ptr only)
    let free_id = builder
        .get_function_by_name("free")
        .expect("free not found");
    builder.call(free_id, vec![ptr]);

    // Void function - return nothing
    builder.ret(None);
}
