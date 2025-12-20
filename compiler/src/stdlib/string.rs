/// String type implementation using MIR Builder
///
/// Provides string operations with actual MIR function bodies

use crate::ir::mir_builder::MirBuilder;
use crate::ir::{IrType, CallingConvention, CompareOp};

/// Build all string type functions
pub fn build_string_type(builder: &mut MirBuilder) {
    // Declare extern functions first
    declare_string_externs(builder);

    build_string_new(builder);
    build_string_concat(builder);
    build_string_length(builder);
    build_string_char_at(builder);
    build_string_char_code_at(builder);
    build_string_substring(builder);
    build_string_index_of(builder);
    build_string_to_upper(builder);
    build_string_to_lower(builder);
    build_string_to_int(builder);
    build_string_to_float(builder);
    build_string_from_chars(builder);
    build_trace(builder);

    // MIR wrapper functions for String methods with optional parameters
    // 1-arg versions provide default startIndex
    build_string_indexof_wrapper(builder);
    build_string_lastindexof_wrapper(builder);
    // 2-arg versions pass through the explicit startIndex
    build_string_indexof_2_wrapper(builder);
    build_string_lastindexof_2_wrapper(builder);
}

/// Declare extern runtime functions for string operations
fn declare_string_externs(builder: &mut MirBuilder) {
    let ptr_void = IrType::Ptr(Box::new(IrType::Void));
    let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
    let i32_ty = IrType::I32;

    // extern fn haxe_string_concat(a: *String, b: *String) -> *String
    // Returns a pointer to avoid struct return ABI issues
    let func_id = builder.begin_function("haxe_string_concat")
        .param("a", string_ptr_ty.clone())
        .param("b", string_ptr_ty.clone())
        .returns(string_ptr_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_string_split_array(s: *String, delim: *String) -> *HaxeArray
    // Returns a proper HaxeArray structure containing string pointers
    let func_id = builder.begin_function("haxe_string_split_array")
        .param("s", string_ptr_ty.clone())
        .param("delimiter", string_ptr_ty.clone())
        .returns(ptr_void.clone()) // Returns *HaxeArray
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_string_index_of_ptr(s: *String, needle: *String, startIndex: i32) -> i32
    let func_id = builder.begin_function("haxe_string_index_of_ptr")
        .param("s", string_ptr_ty.clone())
        .param("needle", string_ptr_ty.clone())
        .param("start_index", i32_ty.clone())
        .returns(i32_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn haxe_string_last_index_of_ptr(s: *String, needle: *String, startIndex: i32) -> i32
    let func_id = builder.begin_function("haxe_string_last_index_of_ptr")
        .param("s", string_ptr_ty.clone())
        .param("needle", string_ptr_ty.clone())
        .param("start_index", i32_ty.clone())
        .returns(i32_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);
}

/// Build: fn string_new() -> String
/// Creates an empty string
fn build_string_new(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("string_new")
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    // Return an empty string constant
    let empty_str = builder.const_string("");
    builder.ret(Some(empty_str));
}

/// Build: fn string_concat(s1: *String, s2: *String) -> *String
/// Concatenates two strings by calling the runtime function
/// Returns a pointer to avoid struct return ABI issues
fn build_string_concat(builder: &mut MirBuilder) {
    let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));

    let func_id = builder.begin_function("string_concat")
        .param("s1", string_ptr_ty.clone())
        .param("s2", string_ptr_ty.clone())
        .returns(string_ptr_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let s1_ptr = builder.get_param(0);
    let s2_ptr = builder.get_param(1);

    // Call extern runtime function directly
    let extern_id = builder.get_function_by_name("haxe_string_concat")
        .expect("haxe_string_concat not found");
    let result = builder.call(extern_id, vec![s1_ptr, s2_ptr]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn string_length(s: &String) -> i32
/// Returns the length of a string
fn build_string_length(builder: &mut MirBuilder) {
    let string_ref_ty = IrType::Ref(Box::new(IrType::String));

    let func_id = builder.begin_function("string_length")
        .param("s", string_ref_ty.clone())
        .returns(IrType::I32)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let s_ptr = builder.get_param(0);
    let s = builder.load(s_ptr, IrType::String);

    // String length is a builtin property that will be lowered
    // to the appropriate runtime call or intrinsic
    // For now, we'll use a placeholder - return 0
    let zero = builder.const_i32(0);
    builder.ret(Some(zero));
}

/// Build: fn string_char_at(s: &String, index: i32) -> String
/// Returns a single character string at the given index
fn build_string_char_at(builder: &mut MirBuilder) {
    let string_ref_ty = IrType::Ref(Box::new(IrType::String));

    let func_id = builder.begin_function("string_char_at")
        .param("s", string_ref_ty.clone())
        .param("index", IrType::I32)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let _s_ptr = builder.get_param(0);
    let _index = builder.get_param(1);

    // TODO: Implement substring extraction
    // For now, return empty string
    let empty = builder.const_string("");
    builder.ret(Some(empty));
}

/// Build: fn string_char_code_at(s: &String, index: i32) -> i32
/// Returns the character code at the given index
fn build_string_char_code_at(builder: &mut MirBuilder) {
    let string_ref_ty = IrType::Ref(Box::new(IrType::String));

    let func_id = builder.begin_function("string_char_code_at")
        .param("s", string_ref_ty.clone())
        .param("index", IrType::I32)
        .returns(IrType::I32)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let _s_ptr = builder.get_param(0);
    let _index = builder.get_param(1);

    // TODO: Implement char code extraction
    // For now, return 0
    let zero = builder.const_i32(0);
    builder.ret(Some(zero));
}

/// Build: fn string_substring(s: &String, start: i32, end: i32) -> String
/// Returns a substring from start to end (exclusive)
fn build_string_substring(builder: &mut MirBuilder) {
    let string_ref_ty = IrType::Ref(Box::new(IrType::String));

    let func_id = builder.begin_function("string_substring")
        .param("s", string_ref_ty.clone())
        .param("start", IrType::I32)
        .param("end", IrType::I32)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let _s_ptr = builder.get_param(0);
    let _start = builder.get_param(1);
    let _end = builder.get_param(2);

    // TODO: Implement substring extraction
    // For now, return empty string
    let empty = builder.const_string("");
    builder.ret(Some(empty));
}

/// Build: fn string_index_of(s: &String, substr: &String, start: i32) -> i32
/// Returns the index of substr in s, starting from start position, or -1 if not found
fn build_string_index_of(builder: &mut MirBuilder) {
    let string_ref_ty = IrType::Ref(Box::new(IrType::String));

    let func_id = builder.begin_function("string_index_of")
        .param("s", string_ref_ty.clone())
        .param("substr", string_ref_ty.clone())
        .param("start", IrType::I32)
        .returns(IrType::I32)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let _s_ptr = builder.get_param(0);
    let _substr_ptr = builder.get_param(1);
    let _start = builder.get_param(2);

    // TODO: Implement string search
    // For now, return -1 (not found)
    let not_found = builder.const_i32(-1);
    builder.ret(Some(not_found));
}

/// Build: fn string_to_upper(s: &String) -> String
/// Converts string to uppercase
fn build_string_to_upper(builder: &mut MirBuilder) {
    let string_ref_ty = IrType::Ref(Box::new(IrType::String));

    let func_id = builder.begin_function("string_to_upper")
        .param("s", string_ref_ty.clone())
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let s_ptr = builder.get_param(0);
    let s = builder.load(s_ptr, IrType::String);

    // TODO: Implement case conversion
    // For now, return the original string
    builder.ret(Some(s));
}

/// Build: fn string_to_lower(s: &String) -> String
/// Converts string to lowercase
fn build_string_to_lower(builder: &mut MirBuilder) {
    let string_ref_ty = IrType::Ref(Box::new(IrType::String));

    let func_id = builder.begin_function("string_to_lower")
        .param("s", string_ref_ty.clone())
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let s_ptr = builder.get_param(0);
    let s = builder.load(s_ptr, IrType::String);

    // TODO: Implement case conversion
    // For now, return the original string
    builder.ret(Some(s));
}

/// Build: fn string_to_int(s: String) -> i32
/// Parses an integer from a string
fn build_string_to_int(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("string_to_int")
        .param("s", IrType::String)
        .returns(IrType::I32)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let _s = builder.get_param(0);

    // TODO: Implement string to int parsing
    // For now, return 0
    let zero = builder.const_i32(0);
    builder.ret(Some(zero));
}

/// Build: fn string_to_float(s: String) -> f64
/// Parses a float from a string
fn build_string_to_float(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("string_to_float")
        .param("s", IrType::String)
        .returns(IrType::F64)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let _s = builder.get_param(0);

    // TODO: Implement string to float parsing
    // For now, return 0.0
    let zero = builder.const_value(crate::ir::IrValue::F64(0.0));
    builder.ret(Some(zero));
}

/// Build: fn string_from_chars(chars: *u8, len: i32) -> String
/// Creates a string from a character array
fn build_string_from_chars(builder: &mut MirBuilder) {
    let ptr_u8_ty = IrType::Ptr(Box::new(IrType::U8));

    let func_id = builder.begin_function("string_from_chars")
        .param("chars", ptr_u8_ty.clone())
        .param("len", IrType::I32)
        .returns(IrType::String)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let _chars_ptr = builder.get_param(0);
    let _len = builder.get_param(1);

    // TODO: Implement string creation from char array
    // For now, return empty string
    let empty = builder.const_string("");
    builder.ret(Some(empty));
}

/// Build: fn trace(value: &String) -> void
/// Prints a value to the console (Haxe's trace function)
fn build_trace(builder: &mut MirBuilder) {
    let string_ref_ty = IrType::Ref(Box::new(IrType::String));

    let func_id = builder.begin_function("trace")
        .param("value", string_ref_ty.clone())
        .returns(IrType::Void)
        .calling_convention(CallingConvention::C)
        .public()
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let _value_ptr = builder.get_param(0);

    // TODO: Implement actual printing to console
    // This would typically call a runtime function or intrinsic
    // For now, just return (no-op)
    builder.ret(None);
}

/// Build: fn String_indexOf(s: *String, needle: *String) -> i32
/// MIR wrapper for String.indexOf that provides default startIndex=0
/// This is the method called when user writes: s.indexOf(needle)
fn build_string_indexof_wrapper(builder: &mut MirBuilder) {
    let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
    let i32_ty = IrType::I32;

    let func_id = builder.begin_function("String_indexOf")
        .param("s", string_ptr_ty.clone())
        .param("needle", string_ptr_ty.clone())
        .returns(i32_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let s = builder.get_param(0);
    let needle = builder.get_param(1);

    // Default startIndex = 0
    let start_index = builder.const_i32(0);

    // Call the runtime function with default startIndex
    let extern_id = builder.get_function_by_name("haxe_string_index_of_ptr")
        .expect("haxe_string_index_of_ptr not found");
    let result = builder.call(extern_id, vec![s, needle, start_index]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn String_lastIndexOf(s: *String, needle: *String) -> i32
/// MIR wrapper for String.lastIndexOf that provides default startIndex=-1 (search from end)
/// This is the method called when user writes: s.lastIndexOf(needle)
fn build_string_lastindexof_wrapper(builder: &mut MirBuilder) {
    let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
    let i32_ty = IrType::I32;

    let func_id = builder.begin_function("String_lastIndexOf")
        .param("s", string_ptr_ty.clone())
        .param("needle", string_ptr_ty.clone())
        .returns(i32_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let s = builder.get_param(0);
    let needle = builder.get_param(1);

    // Default startIndex = -1 (means search from end in runtime)
    // Actually for lastIndexOf, the default should be null which means string.length
    // In Haxe, passing -1 or a large number would search from end
    let start_index = builder.const_i32(-1);

    // Call the runtime function with default startIndex
    let extern_id = builder.get_function_by_name("haxe_string_last_index_of_ptr")
        .expect("haxe_string_last_index_of_ptr not found");
    let result = builder.call(extern_id, vec![s, needle, start_index]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn String_indexOf_2(s: *String, needle: *String, startIndex: i32) -> i32
/// MIR wrapper for String.indexOf with explicit startIndex
/// This is used when user writes: s.indexOf(needle, startIndex)
fn build_string_indexof_2_wrapper(builder: &mut MirBuilder) {
    let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
    let i32_ty = IrType::I32;

    let func_id = builder.begin_function("String_indexOf_2")
        .param("s", string_ptr_ty.clone())
        .param("needle", string_ptr_ty.clone())
        .param("start_index", i32_ty.clone())
        .returns(i32_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let s = builder.get_param(0);
    let needle = builder.get_param(1);
    let start_index = builder.get_param(2);

    // Forward to the runtime function
    let extern_id = builder.get_function_by_name("haxe_string_index_of_ptr")
        .expect("haxe_string_index_of_ptr not found");
    let result = builder.call(extern_id, vec![s, needle, start_index]).unwrap();

    builder.ret(Some(result));
}

/// Build: fn String_lastIndexOf_2(s: *String, needle: *String, startIndex: i32) -> i32
/// MIR wrapper for String.lastIndexOf with explicit startIndex
/// This is used when user writes: s.lastIndexOf(needle, startIndex)
fn build_string_lastindexof_2_wrapper(builder: &mut MirBuilder) {
    let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
    let i32_ty = IrType::I32;

    let func_id = builder.begin_function("String_lastIndexOf_2")
        .param("s", string_ptr_ty.clone())
        .param("needle", string_ptr_ty.clone())
        .param("start_index", i32_ty.clone())
        .returns(i32_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);

    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let s = builder.get_param(0);
    let needle = builder.get_param(1);
    let start_index = builder.get_param(2);

    // Forward to the runtime function
    let extern_id = builder.get_function_by_name("haxe_string_last_index_of_ptr")
        .expect("haxe_string_last_index_of_ptr not found");
    let result = builder.call(extern_id, vec![s, needle, start_index]).unwrap();

    builder.ret(Some(result));
}
