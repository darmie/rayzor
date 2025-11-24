/// Array type implementation using MIR Builder
///
/// Provides array operations with actual MIR function bodies

use crate::ir::mir_builder::MirBuilder;
use crate::ir::{IrType, CallingConvention};

/// Build all array type functions
pub fn build_array_type(builder: &mut MirBuilder) {
    build_array_push(builder);
    build_array_pop(builder);
    build_array_length(builder);
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

    let _arr = builder.get_param(0);
    let _value = builder.get_param(1);

    // TODO: Implement array push
    // This would typically modify the array structure in place
    // For now, just return
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

    let _arr = builder.get_param(0);

    // TODO: Implement array pop
    // For now, return a null/undefined value
    let null_val = builder.const_value(crate::ir::IrValue::Null);
    builder.ret(Some(null_val));
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

    let _arr = builder.get_param(0);

    // TODO: Implement array length extraction
    // For now, return 0
    let zero = builder.const_i32(0);
    builder.ret(Some(zero));
}
