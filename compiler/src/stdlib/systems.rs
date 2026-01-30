/// Systems-level type MIR wrappers (Box, Ptr, Ref, Usize)
///
/// These are zero-cost abstracts over Int (i64) at MIR level.
/// Box operations delegate to runtime functions (alloc/free).
/// Ptr/Ref operations are direct load/store/arithmetic MIR instructions.
/// Usize operations are native i64 arithmetic.
use crate::ir::mir_builder::MirBuilder;
use crate::ir::{BinaryOp, CallingConvention, CompareOp, IrType};

/// Build all systems-level type functions
pub fn build_systems_types(builder: &mut MirBuilder) {
    // Declare extern runtime functions for Box
    declare_box_externs(builder);

    // Build Box MIR wrappers
    build_box_init(builder);
    build_box_unbox(builder);
    build_box_raw(builder);
    build_box_free(builder);

    // Build Ptr MIR wrappers (no externs needed — direct MIR ops)
    build_ptr_from_raw(builder);
    build_ptr_raw(builder);
    build_ptr_deref(builder);
    build_ptr_write(builder);
    build_ptr_offset(builder);
    build_ptr_is_null(builder);

    // Build Ref MIR wrappers (no externs needed — direct MIR ops)
    build_ref_from_raw(builder);
    build_ref_raw(builder);
    build_ref_deref(builder);

    // Build Usize MIR wrappers (no externs needed — native i64 ops)
    build_usize_from_int(builder);
    build_usize_to_int(builder);
    build_usize_add(builder);
    build_usize_sub(builder);
    build_usize_band(builder);
    build_usize_bor(builder);
    build_usize_shl(builder);
    build_usize_shr(builder);
    build_usize_align_up(builder);
    build_usize_is_zero(builder);
}

// ============================================================================
// Box<T> — extern declarations
// ============================================================================

fn declare_box_externs(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;
    let void_ty = IrType::Void;

    // Box is represented as i64 (opaque pointer) throughout the type system.
    // Use i64 for all params/returns to match the MIR wrappers and avoid
    // LLVM type mismatches (ptr vs i64) during module verification.

    // extern fn rayzor_box_init(value: i64) -> i64
    let func_id = builder
        .begin_function("rayzor_box_init")
        .param("value", i64_ty.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn rayzor_box_unbox(box_ptr: i64) -> i64
    let func_id = builder
        .begin_function("rayzor_box_unbox")
        .param("box_ptr", i64_ty.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn rayzor_box_raw(box_ptr: i64) -> i64
    let func_id = builder
        .begin_function("rayzor_box_raw")
        .param("box_ptr", i64_ty.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);

    // extern fn rayzor_box_free(box_ptr: i64) -> void
    let func_id = builder
        .begin_function("rayzor_box_free")
        .param("box_ptr", i64_ty)
        .returns(void_ty)
        .calling_convention(CallingConvention::C)
        .build();
    builder.mark_as_extern(func_id);
}

// ============================================================================
// Box<T> — MIR wrappers
// ============================================================================

/// Box_init(value: i64) -> i64
/// Allocates on heap, stores value, returns heap pointer as i64
fn build_box_init(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Box_init")
        .param("value", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);
    let extern_id = builder
        .get_function_by_name("rayzor_box_init")
        .expect("rayzor_box_init not found");
    let result = builder.call(extern_id, vec![value]).unwrap();
    builder.ret(Some(result));
}

/// Box_unbox(box: i64) -> i64
/// Reads the value from the heap pointer
fn build_box_unbox(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Box_unbox")
        .param("box_ptr", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let box_ptr = builder.get_param(0);
    let extern_id = builder
        .get_function_by_name("rayzor_box_unbox")
        .expect("rayzor_box_unbox not found");
    let result = builder.call(extern_id, vec![box_ptr]).unwrap();
    builder.ret(Some(result));
}

/// Box_raw(box: i64) -> i64
/// Identity — returns the heap address (also used for asPtr/asRef)
fn build_box_raw(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Box_raw")
        .param("box_ptr", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    // Identity: the box pointer IS the raw address
    let box_ptr = builder.get_param(0);
    builder.ret(Some(box_ptr));
}

/// Box_free(box: i64) -> void
/// Deallocates the heap memory
fn build_box_free(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Box_free")
        .param("box_ptr", i64_ty)
        .returns(IrType::Void)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let box_ptr = builder.get_param(0);
    let extern_id = builder
        .get_function_by_name("rayzor_box_free")
        .expect("rayzor_box_free not found");
    builder.call(extern_id, vec![box_ptr]);
    builder.ret(None);
}

// ============================================================================
// Ptr<T> — MIR wrappers (direct MIR instructions, no runtime calls)
// ============================================================================

/// Ptr_fromRaw(address: i64) -> i64  — identity
fn build_ptr_from_raw(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Ptr_fromRaw")
        .param("address", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let address = builder.get_param(0);
    builder.ret(Some(address));
}

/// Ptr_raw(ptr: i64) -> i64  — identity
fn build_ptr_raw(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Ptr_raw")
        .param("ptr", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let ptr = builder.get_param(0);
    builder.ret(Some(ptr));
}

/// Ptr_deref(ptr: i64) -> i64  — load i64 from address
fn build_ptr_deref(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Ptr_deref")
        .param("ptr", i64_ty.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let ptr = builder.get_param(0);
    let value = builder.load(ptr, i64_ty);
    builder.ret(Some(value));
}

/// Ptr_write(ptr: i64, value: i64) -> void  — store i64 to address
fn build_ptr_write(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Ptr_write")
        .param("ptr", i64_ty.clone())
        .param("value", i64_ty)
        .returns(IrType::Void)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let ptr = builder.get_param(0);
    let value = builder.get_param(1);
    builder.store(ptr, value);
    builder.ret(None);
}

/// Ptr_offset(ptr: i64, n: i64) -> i64  — ptr + n * 8 (element size is i64 = 8 bytes)
fn build_ptr_offset(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Ptr_offset")
        .param("ptr", i64_ty.clone())
        .param("n", i64_ty.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let ptr = builder.get_param(0);
    let n = builder.get_param(1);
    // offset = n * 8 (all values are i64 = 8 bytes)
    let eight = builder.const_i64(8);
    let byte_offset = builder.mul(n, eight, i64_ty.clone());
    let result = builder.add(ptr, byte_offset, i64_ty);
    builder.ret(Some(result));
}

/// Ptr_isNull(ptr: i64) -> bool  — ptr == 0
fn build_ptr_is_null(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Ptr_isNull")
        .param("ptr", i64_ty)
        .returns(IrType::Bool)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let ptr = builder.get_param(0);
    let zero = builder.const_i64(0);
    let is_null = builder.icmp(CompareOp::Eq, ptr, zero, IrType::Bool);
    builder.ret(Some(is_null));
}

// ============================================================================
// Ref<T> — MIR wrappers (same as Ptr but read-only, no write)
// ============================================================================

/// Ref_fromRaw(address: i64) -> i64  — identity
fn build_ref_from_raw(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Ref_fromRaw")
        .param("address", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let address = builder.get_param(0);
    builder.ret(Some(address));
}

/// Ref_raw(ref: i64) -> i64  — identity
fn build_ref_raw(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Ref_raw")
        .param("ref_ptr", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let ref_ptr = builder.get_param(0);
    builder.ret(Some(ref_ptr));
}

/// Ref_deref(ref: i64) -> i64  — load i64 from address
fn build_ref_deref(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Ref_deref")
        .param("ref_ptr", i64_ty.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let ref_ptr = builder.get_param(0);
    let value = builder.load(ref_ptr, i64_ty);
    builder.ret(Some(value));
}

// ============================================================================
// Usize — MIR wrappers (native i64 arithmetic, all identity/inline)
// ============================================================================

/// Usize_fromInt(value: i64) -> i64  — identity
fn build_usize_from_int(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_fromInt")
        .param("value", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let value = builder.get_param(0);
    builder.ret(Some(value));
}

/// Usize_toInt(self: i64) -> i64  — identity
fn build_usize_to_int(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_toInt")
        .param("self_val", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let self_val = builder.get_param(0);
    builder.ret(Some(self_val));
}

/// Usize_add(self: i64, other: i64) -> i64
fn build_usize_add(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_add")
        .param("self_val", i64_ty.clone())
        .param("other", i64_ty.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let self_val = builder.get_param(0);
    let other = builder.get_param(1);
    let result = builder.add(self_val, other, i64_ty);
    builder.ret(Some(result));
}

/// Usize_sub(self: i64, other: i64) -> i64
fn build_usize_sub(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_sub")
        .param("self_val", i64_ty.clone())
        .param("other", i64_ty.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let self_val = builder.get_param(0);
    let other = builder.get_param(1);
    let result = builder.sub(self_val, other, i64_ty);
    builder.ret(Some(result));
}

/// Usize_band(self: i64, other: i64) -> i64
fn build_usize_band(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_band")
        .param("self_val", i64_ty.clone())
        .param("other", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let self_val = builder.get_param(0);
    let other = builder.get_param(1);
    let result = builder.bin_op(BinaryOp::And, self_val, other);
    builder.ret(Some(result));
}

/// Usize_bor(self: i64, other: i64) -> i64
fn build_usize_bor(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_bor")
        .param("self_val", i64_ty.clone())
        .param("other", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let self_val = builder.get_param(0);
    let other = builder.get_param(1);
    let result = builder.bin_op(BinaryOp::Or, self_val, other);
    builder.ret(Some(result));
}

/// Usize_shl(self: i64, bits: i64) -> i64
fn build_usize_shl(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_shl")
        .param("self_val", i64_ty.clone())
        .param("bits", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let self_val = builder.get_param(0);
    let bits = builder.get_param(1);
    let result = builder.bin_op(BinaryOp::Shl, self_val, bits);
    builder.ret(Some(result));
}

/// Usize_shr(self: i64, bits: i64) -> i64  (unsigned/logical shift right)
fn build_usize_shr(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_shr")
        .param("self_val", i64_ty.clone())
        .param("bits", i64_ty.clone())
        .returns(i64_ty)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let self_val = builder.get_param(0);
    let bits = builder.get_param(1);
    let result = builder.bin_op(BinaryOp::Shr, self_val, bits);
    builder.ret(Some(result));
}

/// Usize_alignUp(self: i64, alignment: i64) -> i64
/// Computes: (self + alignment - 1) & ~(alignment - 1)
fn build_usize_align_up(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_alignUp")
        .param("self_val", i64_ty.clone())
        .param("alignment", i64_ty.clone())
        .returns(i64_ty.clone())
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let self_val = builder.get_param(0);
    let alignment = builder.get_param(1);

    // align_mask = alignment - 1
    let one = builder.const_i64(1);
    let align_mask = builder.sub(alignment, one, i64_ty.clone());

    // sum = self + align_mask
    let sum = builder.add(self_val, align_mask, i64_ty.clone());

    // neg_mask = ~align_mask  (XOR with -1)
    let neg_one = builder.const_i64(-1);
    let neg_mask = builder.bin_op(BinaryOp::Xor, align_mask, neg_one);

    // result = sum & neg_mask
    let result = builder.bin_op(BinaryOp::And, sum, neg_mask);
    builder.ret(Some(result));
}

/// Usize_isZero(self: i64) -> bool  — self == 0
fn build_usize_is_zero(builder: &mut MirBuilder) {
    let i64_ty = IrType::I64;

    let func_id = builder
        .begin_function("Usize_isZero")
        .param("self_val", i64_ty)
        .returns(IrType::Bool)
        .calling_convention(CallingConvention::C)
        .build();

    builder.set_current_function(func_id);
    let entry = builder.create_block("entry");
    builder.set_insert_point(entry);

    let self_val = builder.get_param(0);
    let zero = builder.const_i64(0);
    let is_zero = builder.icmp(CompareOp::Eq, self_val, zero, IrType::Bool);
    builder.ret(Some(is_zero));
}
