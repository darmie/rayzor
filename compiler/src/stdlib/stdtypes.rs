/// Standard Type Conversions using MIR Builder
///
/// Provides type conversion functions (int/float/bool to string)
/// These are MIR wrappers that call extern runtime functions
use crate::ir::mir_builder::MirBuilder;
use crate::ir::{BinaryOp, CallingConvention, IrType};

/// Build all standard type conversion functions
pub fn build_std_types(builder: &mut MirBuilder) {
    // Declare extern runtime functions first
    declare_string_conversion_externs(builder);
    declare_boxing_externs(builder);

    // Build MIR wrappers for string conversions
    build_int_to_string(builder);
    build_float_to_string(builder);
    build_bool_to_string(builder);
    build_string_to_string(builder);

    // Build MIR wrappers for Dynamic boxing/unboxing
    build_box_int(builder);
    build_box_float(builder);
    build_box_bool(builder);
    build_unbox_int(builder);
    build_unbox_float(builder);
    build_unbox_bool(builder);
}

/// Declare extern runtime functions for string conversions
fn declare_string_conversion_externs(builder: &mut MirBuilder) {
    // extern fn haxe_string_from_int(value: i64) -> String
    let func_id = builder
        .begin_function("haxe_string_from_int")
        .param("value", IrType::I64)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_string_from_float(value: f64) -> String
    let func_id = builder
        .begin_function("haxe_string_from_float")
        .param("value", IrType::F64)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_string_from_bool(value: bool) -> String
    let func_id = builder
        .begin_function("haxe_string_from_bool")
        .param("value", IrType::Bool)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_string_from_string(ptr: *u8, len: usize) -> String
    // Identity function for String type (normalizes representation)
    let ptr_u8 = builder.ptr_type(builder.u8_type());
    let func_id = builder
        .begin_function("haxe_string_from_string")
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
    let func_id = builder
        .begin_function("int_to_string")
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
    let extern_id = builder
        .get_function_by_name("haxe_string_from_int")
        .expect("haxe_string_from_int not found");
    let result = builder.call(extern_id, vec![value_i64]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn float_to_string(value: f64) -> String
/// Converts a float to its string representation by calling runtime
fn build_float_to_string(builder: &mut MirBuilder) {
    let func_id = builder
        .begin_function("float_to_string")
        .param("value", IrType::F64)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);

    // Call extern runtime function
    let extern_id = builder
        .get_function_by_name("haxe_string_from_float")
        .expect("haxe_string_from_float not found");
    let result = builder.call(extern_id, vec![value]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn bool_to_string(value: bool) -> String
/// Converts a boolean to "true" or "false" by calling runtime
fn build_bool_to_string(builder: &mut MirBuilder) {
    let func_id = builder
        .begin_function("bool_to_string")
        .param("value", IrType::Bool)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);

    // Call extern runtime function
    let extern_id = builder
        .get_function_by_name("haxe_string_from_bool")
        .expect("haxe_string_from_bool not found");
    let result = builder.call(extern_id, vec![value]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn string_to_string(s: String) -> String
/// Identity function for String type (normalizes representation)
fn build_string_to_string(builder: &mut MirBuilder) {
    let func_id = builder
        .begin_function("string_to_string")
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

// ============================================================================
// Dynamic Boxing/Unboxing
// ============================================================================

/// Declare extern runtime functions for boxing/unboxing Dynamic values
fn declare_boxing_externs(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());

    // Boxing functions: concrete type -> Dynamic (*u8 pointer to DynamicValue)
    // extern fn haxe_box_int_ptr(value: i64) -> *u8
    let func_id = builder
        .begin_function("haxe_box_int_ptr")
        .param("value", IrType::I64)
        .returns(ptr_u8.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_box_float_ptr(value: f64) -> *u8
    let func_id = builder
        .begin_function("haxe_box_float_ptr")
        .param("value", IrType::F64)
        .returns(ptr_u8.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_box_bool_ptr(value: bool) -> *u8
    let func_id = builder
        .begin_function("haxe_box_bool_ptr")
        .param("value", IrType::Bool)
        .returns(ptr_u8.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // Unboxing functions: Dynamic -> concrete type
    // extern fn haxe_unbox_int_ptr(dynamic: *u8) -> i64
    let func_id = builder
        .begin_function("haxe_unbox_int_ptr")
        .param("dynamic", ptr_u8.clone())
        .returns(IrType::I64)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_unbox_float_ptr(dynamic: *u8) -> f64
    let func_id = builder
        .begin_function("haxe_unbox_float_ptr")
        .param("dynamic", ptr_u8.clone())
        .returns(IrType::F64)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_unbox_bool_ptr(dynamic: *u8) -> bool
    let func_id = builder
        .begin_function("haxe_unbox_bool_ptr")
        .param("dynamic", ptr_u8.clone())
        .returns(IrType::Bool)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);
}

/// Build: fn box_int(value: i32) -> Dynamic
/// Boxes an integer into a Dynamic value
fn build_box_int(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());

    let func_id = builder
        .begin_function("box_int")
        .param("value", IrType::I32)
        .returns(ptr_u8.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);

    // Cast i32 to i64 for runtime function
    let value_i64 = builder.cast(value, IrType::I32, IrType::I64);

    // Call extern runtime function
    let extern_id = builder
        .get_function_by_name("haxe_box_int_ptr")
        .expect("haxe_box_int_ptr not found");
    let result = builder.call(extern_id, vec![value_i64]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn box_float(value: f64) -> Dynamic
/// Boxes a float into a Dynamic value
fn build_box_float(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());

    let func_id = builder
        .begin_function("box_float")
        .param("value", IrType::F64)
        .returns(ptr_u8.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);

    // Call extern runtime function
    let extern_id = builder
        .get_function_by_name("haxe_box_float_ptr")
        .expect("haxe_box_float_ptr not found");
    let result = builder.call(extern_id, vec![value]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn box_bool(value: bool) -> Dynamic
/// Boxes a boolean into a Dynamic value
fn build_box_bool(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());

    let func_id = builder
        .begin_function("box_bool")
        .param("value", IrType::Bool)
        .returns(ptr_u8.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);

    // Call extern runtime function
    let extern_id = builder
        .get_function_by_name("haxe_box_bool_ptr")
        .expect("haxe_box_bool_ptr not found");
    let result = builder.call(extern_id, vec![value]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn unbox_int(dynamic: Dynamic) -> i32
/// Unboxes an integer from a Dynamic value
fn build_unbox_int(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());

    let func_id = builder
        .begin_function("unbox_int")
        .param("dynamic", ptr_u8.clone())
        .returns(IrType::I32)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let dynamic = builder.get_param(0);

    // Call extern runtime function
    let extern_id = builder
        .get_function_by_name("haxe_unbox_int_ptr")
        .expect("haxe_unbox_int_ptr not found");
    let result_i64 = builder.call(extern_id, vec![dynamic]).unwrap();

    // Cast i64 to i32
    let result = builder.cast(result_i64, IrType::I64, IrType::I32);

    builder.ret(Some(result));
}

/// Build: fn unbox_float(dynamic: Dynamic) -> f64
/// Unboxes a float from a Dynamic value
fn build_unbox_float(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());

    let func_id = builder
        .begin_function("unbox_float")
        .param("dynamic", ptr_u8.clone())
        .returns(IrType::F64)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let dynamic = builder.get_param(0);

    // Call extern runtime function
    let extern_id = builder
        .get_function_by_name("haxe_unbox_float_ptr")
        .expect("haxe_unbox_float_ptr not found");
    let result = builder.call(extern_id, vec![dynamic]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn unbox_bool(dynamic: Dynamic) -> bool
/// Unboxes a boolean from a Dynamic value
fn build_unbox_bool(builder: &mut MirBuilder) {
    let ptr_u8 = builder.ptr_type(builder.u8_type());

    let func_id = builder
        .begin_function("unbox_bool")
        .param("dynamic", ptr_u8.clone())
        .returns(IrType::Bool)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let dynamic = builder.get_param(0);

    // Call extern runtime function
    let extern_id = builder
        .get_function_by_name("haxe_unbox_bool_ptr")
        .expect("haxe_unbox_bool_ptr not found");
    let result = builder.call(extern_id, vec![dynamic]).unwrap();

    builder.ret(Some(result));
}
