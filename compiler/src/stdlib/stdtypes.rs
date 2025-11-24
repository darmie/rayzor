/// Standard Type Conversions using MIR Builder
///
/// Provides type conversion functions (int/float/bool to string)
/// These are MIR wrappers that call extern runtime functions

use crate::ir::mir_builder::MirBuilder;
use crate::ir::{IrType, CallingConvention, BinaryOp};

/// Build all standard type conversion functions
pub fn build_std_types(builder: &mut MirBuilder) {
    // Declare extern runtime functions first
    declare_string_conversion_externs(builder);

    // Build MIR wrappers
    build_int_to_string(builder);
    build_float_to_string(builder);
    build_bool_to_string(builder);
    build_string_to_string(builder);
}

/// Declare extern runtime functions for string conversions
fn declare_string_conversion_externs(builder: &mut MirBuilder) {
    // extern fn haxe_string_from_int(value: i64) -> String
    let func_id = builder.begin_function("haxe_string_from_int")
        .param("value", IrType::I64)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_string_from_float(value: f64) -> String
    let func_id = builder.begin_function("haxe_string_from_float")
        .param("value", IrType::F64)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_string_from_bool(value: bool) -> String
    let func_id = builder.begin_function("haxe_string_from_bool")
        .param("value", IrType::Bool)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_string_from_string(ptr: *u8, len: usize) -> String
    // Identity function for String type (normalizes representation)
    let ptr_u8 = builder.ptr_type(builder.u8_type());
    let func_id = builder.begin_function("haxe_string_from_string")
        .param("ptr", ptr_u8)
        .param("len", IrType::I64)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);
}

/// Build: fn int_to_string(value: i32) -> String
/// Converts an integer to its string representation by calling runtime
fn build_int_to_string(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("int_to_string")
        .param("value", IrType::I32)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);

    // Cast i32 to i64 for runtime function
    let value_i64 = builder.cast(value, IrType::I32, IrType::I64);

    // Call extern runtime function
    let extern_id = builder.get_function_by_name("haxe_string_from_int")
        .expect("haxe_string_from_int not found");
    let result = builder.call(extern_id, vec![value_i64]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn float_to_string(value: f64) -> String
/// Converts a float to its string representation by calling runtime
fn build_float_to_string(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("float_to_string")
        .param("value", IrType::F64)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);

    // Call extern runtime function
    let extern_id = builder.get_function_by_name("haxe_string_from_float")
        .expect("haxe_string_from_float not found");
    let result = builder.call(extern_id, vec![value]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn bool_to_string(value: bool) -> String
/// Converts a boolean to "true" or "false" by calling runtime
fn build_bool_to_string(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("bool_to_string")
        .param("value", IrType::Bool)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);

    // Call extern runtime function
    let extern_id = builder.get_function_by_name("haxe_string_from_bool")
        .expect("haxe_string_from_bool not found");
    let result = builder.call(extern_id, vec![value]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn string_to_string(s: String) -> String
/// Identity function for String type (normalizes representation)
fn build_string_to_string(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("string_to_string")
        .param("s", IrType::String)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let string_val = builder.get_param(0);

    // For String type, we could just return it directly, but for consistency
    // with other conversions and to ensure proper memory management, we call
    // the runtime function which normalizes the representation
    //
    // String is a struct { ptr: *u8, len: usize }, we need to extract the fields
    // TODO: Add proper struct field extraction to MirBuilder
    // For now, just return the string as-is
    builder.ret(Some(string_val));
}
