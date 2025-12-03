/// Array type implementation using MIR Builder
///
/// Provides array operations with actual MIR function bodies
///
/// Array operations that return Array instances (slice, copy) use out-param
/// convention where the runtime function writes to a provided HaxeArray struct.
/// The MIR wrappers handle allocation and forwarding.

use crate::ir::mir_builder::MirBuilder;
use crate::ir::{IrType, CallingConvention};

/// HaxeArray runtime structure size in bytes
/// struct HaxeArray { ptr: *mut u8, len: usize, cap: usize, elem_size: usize }
/// On 64-bit: 8 + 8 + 8 + 8 = 32 bytes
const HAXE_ARRAY_STRUCT_SIZE: usize = 32;

/// Build all array type functions
pub fn build_array_type(builder: &mut MirBuilder) {
    // Declare extern runtime functions
    declare_array_externs(builder);

    // Build MIR wrapper functions
    build_array_push(builder);
    build_array_pop(builder);
    build_array_length(builder);
    build_array_slice(builder);
}

/// Declare Array extern runtime functions
fn declare_array_externs(builder: &mut MirBuilder) {
    let ptr_void = IrType::Ptr(Box::new(IrType::Void));
    let i64_ty = IrType::I64;
    let i32_ty = IrType::I32;
    let void_ty = IrType::Void;

    // haxe_array_push_i64(arr: *mut HaxeArray, val: i64)
    let func_id = builder.begin_function("haxe_array_push_i64")
        .param("arr", ptr_void.clone())
        .param("val", i64_ty.clone())
        .returns(void_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // haxe_array_pop_ptr(arr: *mut HaxeArray) -> *mut u8
    let func_id = builder.begin_function("haxe_array_pop_ptr")
        .param("arr", ptr_void.clone())
        .returns(ptr_void.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // haxe_array_length(arr: *const HaxeArray) -> usize
    let func_id = builder.begin_function("haxe_array_length")
        .param("arr", ptr_void.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // haxe_array_slice(out: *mut HaxeArray, arr: *const HaxeArray, start: usize, end: usize)
    let func_id = builder.begin_function("haxe_array_slice")
        .param("out", ptr_void.clone())
        .param("arr", ptr_void.clone())
        .param("start", i64_ty.clone())
        .param("end", i64_ty.clone())
        .returns(void_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // haxe_array_copy(out: *mut HaxeArray, arr: *const HaxeArray)
    let func_id = builder.begin_function("haxe_array_copy")
        .param("out", ptr_void.clone())
        .param("arr", ptr_void.clone())
        .returns(void_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);
}

/// Build: fn array_push(arr: Any, value: Any) -> void
/// Appends an element to the array
fn build_array_push(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("array_push")
        .param("arr", IrType::Any)
        .param("value", IrType::Any)
        .returns(IrType::Void)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let arr = builder.get_param(0);
    let value = builder.get_param(1);

    // Call runtime function haxe_array_push_i64(arr: *HaxeArray, val: i64)
    let extern_func = builder.get_function_by_name("haxe_array_push_i64")
        .expect("haxe_array_push_i64 extern not found");

    builder.call(extern_func, vec![arr, value]);

    builder.ret(None);
}

/// Build: fn array_pop(arr: Any) -> Any
/// Removes and returns the last element from the array
fn build_array_pop(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("array_pop")
        .param("arr", IrType::Any)
        .returns(IrType::Any)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let arr = builder.get_param(0);

    // Call runtime function haxe_array_pop_ptr(arr: *HaxeArray) -> *mut u8
    let extern_func = builder.get_function_by_name("haxe_array_pop_ptr")
        .expect("haxe_array_pop_ptr extern not found");

    if let Some(result) = builder.call(extern_func, vec![arr]) {
        builder.ret(Some(result));
    } else {
        let null_val = builder.const_value(crate::ir::IrValue::Null);
        builder.ret(Some(null_val));
    }
}

/// Build: fn array_length(arr: Any) -> i32
/// Returns the length of the array
fn build_array_length(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("array_length")
        .param("arr", IrType::Any)
        .returns(IrType::I32)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let arr = builder.get_param(0);

    // Call runtime function haxe_array_length(arr: *HaxeArray) -> i64 (usize)
    let extern_func = builder.get_function_by_name("haxe_array_length")
        .expect("haxe_array_length extern not found");

    if let Some(len_i64) = builder.call(extern_func, vec![arr]) {
        // Cast i64 to i32 (array lengths should fit in i32)
        let len_i32 = builder.cast(len_i64, IrType::I64, IrType::I32);
        builder.ret(Some(len_i32));
    } else {
        let zero = builder.const_i32(0);
        builder.ret(Some(zero));
    }
}

/// Build: fn Array_slice(arr: Ptr(Void), start: i64, end: i64) -> Ptr(Void)
/// Wrapper for haxe_array_slice that handles out-param allocation
///
/// This wrapper:
/// 1. Allocates space for the result HaxeArray struct (32 bytes)
/// 2. Calls haxe_array_slice(out_ptr, arr, start, end)
/// 3. Returns the pointer to the allocated result
fn build_array_slice(builder: &mut MirBuilder) {
    let ptr_void = IrType::Ptr(Box::new(IrType::Void));
    let i64_ty = IrType::I64;

    // Function signature: Array_slice(arr: *Array, start: i64, end: i64) -> *Array
    let func_id = builder.begin_function("Array_slice")
        .param("arr", ptr_void.clone())
        .param("start", i64_ty.clone())
        .param("end", i64_ty.clone())
        .returns(ptr_void.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let arr = builder.get_param(0);
    let start = builder.get_param(1);
    let end = builder.get_param(2);

    // Allocate space for HaxeArray struct (32 bytes = 4 x i64)
    // HaxeArray struct: { ptr: *mut u8, len: usize, cap: usize, elem_size: usize }
    // We allocate an array of 4 i64 values on the stack
    let array_count = builder.const_i64(4);
    let out_ptr = builder.alloc(i64_ty.clone(), Some(array_count));

    // Call haxe_array_slice(out_ptr, arr, start, end)
    let extern_func = builder.get_function_by_name("haxe_array_slice")
        .expect("haxe_array_slice extern not found");

    builder.call(
        extern_func,
        vec![out_ptr, arr, start, end],
    );

    // Return the pointer to the allocated array
    builder.ret(Some(out_ptr));
}
