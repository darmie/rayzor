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
/// Test Cranelift backend with comparison operations
///
/// Tests: eq, ne, lt, le, gt, ge (signed comparisons)
use compiler::codegen::CraneliftBackend;
use compiler::ir::*;
use compiler::tast::SymbolId;

fn create_cmp_function(name: &str, op: CompareOp, func_id: u32) -> (IrFunction, IrFunctionId) {
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
        return_type: IrType::I32, // Comparisons return i32 (0 or 1)
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
        uses_sret: false,
    };

    let mut function = IrFunction::new(fid, symbol_id, name.to_string(), signature);

    function.locals.insert(
        param_a,
        IrLocal {
            name: "a".to_string(),
            ty: IrType::I64,
            mutable: false,
            source_location: IrSourceLocation::unknown(),
            allocation: AllocationHint::Register,
        },
    );

    function.locals.insert(
        param_b,
        IrLocal {
            name: "b".to_string(),
            ty: IrType::I64,
            mutable: false,
            source_location: IrSourceLocation::unknown(),
            allocation: AllocationHint::Register,
        },
    );

    function.locals.insert(
        result_reg,
        IrLocal {
            name: "result".to_string(),
            ty: IrType::I32,
            mutable: false,
            source_location: IrSourceLocation::unknown(),
            allocation: AllocationHint::Register,
        },
    );

    let entry_block = function.cfg.entry_block;
    let entry = function.cfg.blocks.get_mut(&entry_block).unwrap();

    entry.instructions.push(IrInstruction::Cmp {
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
    println!("=== Cranelift Backend Test: Comparison Operations ===\n");

    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    let mut func_ids = Vec::new();

    // Create functions for each comparison
    let operations = vec![
        ("eq", CompareOp::Eq),
        ("ne", CompareOp::Ne),
        ("lt", CompareOp::Lt),
        ("le", CompareOp::Le),
        ("gt", CompareOp::Gt),
        ("ge", CompareOp::Ge),
    ];

    println!(
        "Creating MIR functions for {} comparisons...",
        operations.len()
    );
    for (idx, (name, op)) in operations.iter().enumerate() {
        let (func, fid) = create_cmp_function(name, *op, idx as u32);
        func_ids.push((name.to_string(), fid));
        module.functions.insert(fid, func);
        println!("  ✓ {}: (a: i64, b: i64) -> i32", name);
    }
    println!();

    // Initialize and compile
    println!("Initializing Cranelift backend...");
    let mut backend = CraneliftBackend::new()?;
    println!("✅ Backend initialized\n");

    println!("Compiling {} functions...", func_ids.len());
    backend.compile_module(&module)?;
    println!("✅ Compilation successful\n");

    // Test each comparison
    println!("Testing comparisons:\n");

    let mut all_passed = true;

    // Test Eq (equal)
    {
        let (name, fid) = &func_ids[0];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i32 = unsafe { std::mem::transmute(func_ptr) };

        let r1 = op(42, 42);
        let r2 = op(10, 20);
        println!("  {}(42, 42) = {} (expected 1)", name, r1);
        println!("  {}(10, 20) = {} (expected 0)", name, r2);
        all_passed &= r1 != 0 && r2 == 0;
    }

    // Test Ne (not equal)
    {
        let (name, fid) = &func_ids[1];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i32 = unsafe { std::mem::transmute(func_ptr) };

        let r1 = op(10, 20);
        let r2 = op(42, 42);
        println!("  {}(10, 20) = {} (expected 1)", name, r1);
        println!("  {}(42, 42) = {} (expected 0)", name, r2);
        all_passed &= r1 != 0 && r2 == 0;
    }

    // Test Lt (less than)
    {
        let (name, fid) = &func_ids[2];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i32 = unsafe { std::mem::transmute(func_ptr) };

        let r1 = op(10, 20);
        let r2 = op(20, 10);
        let r3 = op(10, 10);
        println!("  {}(10, 20) = {} (expected 1)", name, r1);
        println!("  {}(20, 10) = {} (expected 0)", name, r2);
        println!("  {}(10, 10) = {} (expected 0)", name, r3);
        all_passed &= r1 != 0 && r2 == 0 && r3 == 0;
    }

    // Test Le (less than or equal)
    {
        let (name, fid) = &func_ids[3];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i32 = unsafe { std::mem::transmute(func_ptr) };

        let r1 = op(10, 20);
        let r2 = op(10, 10);
        let r3 = op(20, 10);
        println!("  {}(10, 20) = {} (expected 1)", name, r1);
        println!("  {}(10, 10) = {} (expected 1)", name, r2);
        println!("  {}(20, 10) = {} (expected 0)", name, r3);
        all_passed &= r1 != 0 && r2 != 0 && r3 == 0;
    }

    // Test Gt (greater than)
    {
        let (name, fid) = &func_ids[4];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i32 = unsafe { std::mem::transmute(func_ptr) };

        let r1 = op(20, 10);
        let r2 = op(10, 20);
        let r3 = op(10, 10);
        println!("  {}(20, 10) = {} (expected 1)", name, r1);
        println!("  {}(10, 20) = {} (expected 0)", name, r2);
        println!("  {}(10, 10) = {} (expected 0)", name, r3);
        all_passed &= r1 != 0 && r2 == 0 && r3 == 0;
    }

    // Test Ge (greater than or equal)
    {
        let (name, fid) = &func_ids[5];
        let func_ptr = backend.get_function_ptr(*fid)?;
        let op: fn(i64, i64) -> i32 = unsafe { std::mem::transmute(func_ptr) };

        let r1 = op(20, 10);
        let r2 = op(10, 10);
        let r3 = op(10, 20);
        println!("  {}(20, 10) = {} (expected 1)", name, r1);
        println!("  {}(10, 10) = {} (expected 1)", name, r2);
        println!("  {}(10, 20) = {} (expected 0)", name, r3);
        all_passed &= r1 != 0 && r2 != 0 && r3 == 0;
    }

    println!();

    if all_passed {
        println!(
            "🎉 SUCCESS: All {} comparison operations passed!",
            operations.len()
        );
        Ok(())
    } else {
        Err("FAILED: Some comparisons produced incorrect results".to_string())
    }
}
