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
/// Test Cranelift backend with all binary operations
///
/// Tests: add, sub, mul, div, rem, and, or, xor, shl, shr
use compiler::codegen::CraneliftBackend;
use compiler::ir::*;
use compiler::tast::SymbolId;

fn create_binop_function(name: &str, op: BinaryOp, func_id: u32) -> (IrFunction, IrFunctionId) {
    let fid = IrFunctionId(func_id);
    let symbol_id = SymbolId::from_raw(func_id + 1);

    let param_a = IrId::new(0);
    let param_b = IrId::new(1);
    let result_reg = IrId::new(2);

    let signature = IrFunctionSignature {
        parameters: vec![
            IrParameter {
                name: "a".to_string(),
                ty: IrType::I64,
                reg: param_a,
                by_ref: false,
            },
            IrParameter {
                name: "b".to_string(),
                ty: IrType::I64,
                reg: param_b,
                by_ref: false,
            },
        ],
        return_type: IrType::I64,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
        uses_sret: false,
    };

    let mut function = IrFunction::new(fid, symbol_id, name.to_string(), signature);

    function.locals.insert(
        result_reg,
        IrLocal {
            name: "result".to_string(),
            ty: IrType::I64,
            mutable: false,
            source_location: IrSourceLocation::unknown(),
            allocation: AllocationHint::Register,
        },
    );

    let entry_block = function.cfg.entry_block;
    let entry = function.cfg.blocks.get_mut(&entry_block).unwrap();

    entry.instructions.push(IrInstruction::BinOp {
        dest: result_reg,
        op,
        left: param_a,
        right: param_b,
    });

    entry.terminator = IrTerminator::Return {
        value: Some(result_reg),
    };

    (function, fid)
}

fn main() -> Result<(), String> {
    println!("=== Cranelift Backend Test: Binary Operations ===\n");

    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    let mut func_ids = Vec::new();

    // Create functions for each operation
    let operations = vec![
        ("add", BinaryOp::Add),
        ("sub", BinaryOp::Sub),
        ("mul", BinaryOp::Mul),
        ("div", BinaryOp::Div),
        ("rem", BinaryOp::Rem),
        ("and", BinaryOp::And),
        ("or", BinaryOp::Or),
        ("xor", BinaryOp::Xor),
        ("shl", BinaryOp::Shl),
        ("shr", BinaryOp::Shr),
    ];

    println!(
        "Creating MIR functions for {} operations...",
        operations.len()
    );
    for (idx, (name, op)) in operations.iter().enumerate() {
        let (func, fid) = create_binop_function(name, op.clone(), idx as u32);
        func_ids.push((name.to_string(), fid));
        module.functions.insert(fid, func);
        println!("  ✓ {}: (a: i64, b: i64) -> i64", name);
    }
    println!();

    // Initialize and compile
    println!("Initializing Cranelift backend...");
    let mut backend = CraneliftBackend::new()?;
    println!("✅ Backend initialized\n");

    println!("Compiling {} functions...", func_ids.len());
    backend.compile_module(&module)?;
    println!("✅ Compilation successful\n");

    // Test each operation
    println!("Testing operations:\n");

    let mut all_passed = true;

    // Test Add
    {
        let (name, fid) = &func_ids[0];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(10, 20);
        let expected = 30;
        println!("  {}(10, 20) = {} (expected {})", name, result, expected);
        all_passed &= result == expected;
    }

    // Test Sub
    {
        let (name, fid) = &func_ids[1];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(50, 20);
        let expected = 30;
        println!("  {}(50, 20) = {} (expected {})", name, result, expected);
        all_passed &= result == expected;
    }

    // Test Mul
    {
        let (name, fid) = &func_ids[2];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(6, 7);
        let expected = 42;
        println!("  {}(6, 7) = {} (expected {})", name, result, expected);
        all_passed &= result == expected;
    }

    // Test Div
    {
        let (name, fid) = &func_ids[3];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(84, 2);
        let expected = 42;
        println!("  {}(84, 2) = {} (expected {})", name, result, expected);
        all_passed &= result == expected;
    }

    // Test Rem
    {
        let (name, fid) = &func_ids[4];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(17, 5);
        let expected = 2;
        println!("  {}(17, 5) = {} (expected {})", name, result, expected);
        all_passed &= result == expected;
    }

    // Test And
    {
        let (name, fid) = &func_ids[5];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(0b1100, 0b1010);
        let expected = 0b1000;
        println!(
            "  {}(0b1100, 0b1010) = 0b{:b} (expected 0b{:b})",
            name, result, expected
        );
        all_passed &= result == expected;
    }

    // Test Or
    {
        let (name, fid) = &func_ids[6];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(0b1100, 0b1010);
        let expected = 0b1110;
        println!(
            "  {}(0b1100, 0b1010) = 0b{:b} (expected 0b{:b})",
            name, result, expected
        );
        all_passed &= result == expected;
    }

    // Test Xor
    {
        let (name, fid) = &func_ids[7];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(0b1100, 0b1010);
        let expected = 0b0110;
        println!(
            "  {}(0b1100, 0b1010) = 0b{:b} (expected 0b{:b})",
            name, result, expected
        );
        all_passed &= result == expected;
    }

    // Test Shl (left shift)
    {
        let (name, fid) = &func_ids[8];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(5, 2);
        let expected = 20; // 5 << 2 = 20
        println!("  {}(5, 2) = {} (expected {})", name, result, expected);
        all_passed &= result == expected;
    }

    // Test Shr (right shift)
    {
        let (name, fid) = &func_ids[9];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
        let result = op(20, 2);
        let expected = 5; // 20 >> 2 = 5
        println!("  {}(20, 2) = {} (expected {})", name, result, expected);
        all_passed &= result == expected;
    }

    println!();

    if all_passed {
        println!(
            "🎉 SUCCESS: All {} binary operations passed!",
            operations.len()
        );
        Ok(())
    } else {
        Err("FAILED: Some operations produced incorrect results".to_string())
    }
}
