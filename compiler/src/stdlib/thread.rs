/// Thread: Lightweight concurrent execution primitives
///
/// This module provides MIR implementations for thread operations.
/// The actual threading is delegated to extern runtime functions.
///
/// Memory layout:
/// ```
/// struct Thread<T> {
///     handle: *u8,    // Opaque OS thread handle
///     result: *T,     // Pointer to result (set when joined)
///     state: u8,      // 0=running, 1=finished, 2=joined
/// }
/// ```

use crate::ir::mir_builder::MirBuilder;
use crate::ir::{IrType, IrFunctionId, Linkage, CallingConvention};

/// Build all Thread functions
pub fn build_thread_type(builder: &mut MirBuilder) {
    // Declare extern runtime functions first
    declare_thread_externs(builder);

    // Build wrapper functions
    build_thread_spawn(builder);
    build_thread_join(builder);
    build_thread_is_finished(builder);
    build_thread_yield_now(builder);
    build_thread_sleep(builder);
    build_thread_current_id(builder);
}

/// Declare extern runtime functions
fn declare_thread_externs(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());
    let i32_ty = builder.i32_type();
    let u64_ty = builder.u64_type();
    let bool_ty = builder.bool_type();
    let void_ty = builder.void_type();

    // extern fn rayzor_thread_spawn(closure: *u8, closure_env: *u8) -> *u8
    let func_id = builder.begin_function("rayzor_thread_spawn")
        .param("closure", ptr_u8.clone())
        .param("closure_env", ptr_u8.clone())
        .returns(ptr_u8.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn rayzor_thread_join(handle: *u8) -> *u8
    let func_id = builder.begin_function("rayzor_thread_join")
        .param("handle", ptr_u8.clone())
        .returns(ptr_u8.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn rayzor_thread_is_finished(handle: *u8) -> bool
    let func_id = builder.begin_function("rayzor_thread_is_finished")
        .param("handle", ptr_u8.clone())
        .returns(bool_ty)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn rayzor_thread_yield_now()
    let func_id = builder.begin_function("rayzor_thread_yield_now")
        .returns(void_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn rayzor_thread_sleep(millis: i32)
    let func_id = builder.begin_function("rayzor_thread_sleep")
        .param("millis", i32_ty)
        .returns(void_ty)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn rayzor_thread_current_id() -> u64
    let func_id = builder.begin_function("rayzor_thread_current_id")
        .returns(u64_ty)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);
}

/// Build: fn Thread_spawn(closure_obj: *u8) -> *Thread
/// The closure_obj is a pointer to a struct { fn_ptr: *u8, env_ptr: *u8 }
fn build_thread_spawn(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());

    let func_id = builder.begin_function("Thread_spawn")
        .param("closure_obj", ptr_u8.clone())
        .returns(ptr_u8.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let closure_obj = builder.get_param(0);

    // Extract function pointer from closure object (offset 0)
    let fn_ptr = builder.load(closure_obj, ptr_u8.clone());

    // Extract environment pointer from closure object (offset 8)
    let offset_8 = builder.const_i64(8);
    let env_ptr_addr = builder.ptr_add(closure_obj, offset_8, ptr_u8.clone());
    let env_ptr = builder.load(env_ptr_addr, ptr_u8.clone());

    // Call runtime function with extracted pointers
    let spawn_id = builder.get_function_by_name("rayzor_thread_spawn")
        .expect("rayzor_thread_spawn not found");
    let handle = builder.call(spawn_id, vec![fn_ptr, env_ptr]).unwrap();

    builder.ret(Some(handle));
}

/// Build: fn Thread_join(handle: *Thread) -> *u8 (i64)
/// TODO: This should be generic Thread<T>.join() -> T
/// For now it returns i64 and relies on caller to cast to correct type
fn build_thread_join(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());

    let func_id = builder.begin_function("Thread_join")
        .param("handle", ptr_u8.clone())
        .returns(ptr_u8.clone())  // Return ptr_u8 (i64) to match rayzor_thread_join
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let handle = builder.get_param(0);

    // Call runtime function (returns *u8 which is i64)
    let join_id = builder.get_function_by_name("rayzor_thread_join")
        .expect("rayzor_thread_join not found");
    let result_ptr = builder.call(join_id, vec![handle]).unwrap();

    // TODO: The runtime returns i64, but we declared this function as returning i32.
    // Ideally we'd cast here, but the function signature already says i32, so the
    // caller will handle the truncation at the call site when the types don't match.
    // Just return the i64 value and let type checking insert the cast later.
    builder.ret(Some(result_ptr));
}

/// Build: fn Thread_isFinished(handle: *Thread) -> bool
fn build_thread_is_finished(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());
    let bool_ty = builder.bool_type();

    let func_id = builder.begin_function("Thread_isFinished")
        .param("handle", ptr_u8)
        .returns(bool_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let handle = builder.get_param(0);

    // Call runtime function
    let is_finished_id = builder.get_function_by_name("rayzor_thread_is_finished")
        .expect("rayzor_thread_is_finished not found");
    let result = builder.call(is_finished_id, vec![handle]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn Thread_yieldNow()
fn build_thread_yield_now(builder: &mut MirBuilder) {
    let void_ty = builder.void_type();

    let func_id = builder.begin_function("Thread_yieldNow")
        .returns(void_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    // Call runtime function
    let yield_id = builder.get_function_by_name("rayzor_thread_yield_now")
        .expect("rayzor_thread_yield_now not found");
    let _result = builder.call(yield_id, vec![]);

    builder.ret(None);
}

/// Build: fn Thread_sleep(millis: i32)
fn build_thread_sleep(builder: &mut MirBuilder) {
    let i32_ty = builder.i32_type();
    let void_ty = builder.void_type();

    let func_id = builder.begin_function("Thread_sleep")
        .param("millis", i32_ty)
        .returns(void_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let millis = builder.get_param(0);

    // Call runtime function
    let sleep_id = builder.get_function_by_name("rayzor_thread_sleep")
        .expect("rayzor_thread_sleep not found");
    let _result = builder.call(sleep_id, vec![millis]);

    builder.ret(None);
}

/// Build: fn Thread_currentId() -> u64
fn build_thread_current_id(builder: &mut MirBuilder) {
    let u64_ty = builder.u64_type();

    let func_id = builder.begin_function("Thread_currentId")
        .returns(u64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    // Call runtime function
    let current_id_fn = builder.get_function_by_name("rayzor_thread_current_id")
        .expect("rayzor_thread_current_id not found");
    let result = builder.call(current_id_fn, vec![]).unwrap();

    builder.ret(Some(result));
}
