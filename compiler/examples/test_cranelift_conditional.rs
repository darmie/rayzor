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
/// Test Cranelift backend with conditional branching (if/else)
///
/// Creates a function: fn max(a: i64, b: i64) -> i64 {
///     if (a > b) { return a; } else { return b; }
/// }
use compiler::codegen::CraneliftBackend;
use compiler::ir::*;
use compiler::tast::SymbolId;

fn main() -> Result<(), String> {
    println!("=== Cranelift Backend Test: Conditional Branching ===\n");

    // Create function: fn max(a: i64, b: i64) -> i64
    let func_id = IrFunctionId(0);
    let symbol_id = SymbolId::from_raw(1);

    let param_a = IrId::new(0);
    let param_b = IrId::new(1);
    let cond_reg = IrId::new(2);

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

    let mut function = IrFunction::new(func_id, symbol_id, "max".to_string(), signature);

    // Add locals
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
        cond_reg,
        IrLocal {
            name: "cond".to_string(),
            ty: IrType::I32,
            mutable: false,
            source_location: IrSourceLocation::unknown(),
            allocation: AllocationHint::Register,
        },
    );

    // Create blocks
    let entry_block = function.cfg.entry_block; // bb0
    let then_block = function.cfg.create_block(); // bb1
    let else_block = function.cfg.create_block(); // bb2

    // Entry block: compare a > b, then branch
    {
        let entry = function.cfg.blocks.get_mut(&entry_block).unwrap();

        // cond = a > b
        entry.instructions.push(IrInstruction::Cmp {
            dest: cond_reg,
            op: CompareOp::Gt,
            left: param_a,
            right: param_b,
        });

        // if (cond) goto then_block else goto else_block
        entry.terminator = IrTerminator::CondBranch {
            condition: cond_reg,
            true_target: then_block,
            false_target: else_block,
        };
    }

    // Then block: return a
    {
        let then = function.cfg.blocks.get_mut(&then_block).unwrap();
        then.terminator = IrTerminator::Return {
            value: Some(param_a),
        };
    }

    // Else block: return b
    {
        let else_blk = function.cfg.blocks.get_mut(&else_block).unwrap();
        else_blk.terminator = IrTerminator::Return {
            value: Some(param_b),
        };
    }

    println!("Created MIR function:");
    println!("  Name: max");
    println!("  Signature: (a: i64, b: i64) -> i64");
    println!("  CFG:");
    println!("    bb0 (entry):");
    println!("      %2 = cmp.gt %0, %1");
    println!("      brif %2, bb1, bb2");
    println!("    bb1 (then):");
    println!("      return %0");
    println!("    bb2 (else):");
    println!("      return %1");
    println!();

    // Create MIR module
    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    module.functions.insert(func_id, function);

    // Initialize and compile
    println!("Initializing Cranelift backend...");
    let mut backend = CraneliftBackend::new()?;
    println!("✅ Backend initialized\n");

    println!("Compiling MIR → Cranelift IR...");
    backend.compile_module(&module)?;
    println!("✅ Compilation successful\n");

    // Get function pointer
    println!("Retrieving function pointer...");
    let func_ptr = backend.get_function_ptr(func_id)?;
    println!("✅ Function pointer: {:p}\n", func_ptr);

    // Test the conditional function
    println!("Executing JIT-compiled function...\n");

    let max_fn: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };

    // Test cases
    let tests = vec![
        (10, 20, 20),    // b > a, should return b
        (30, 15, 30),    // a > b, should return a
        (42, 42, 42),    // a == b, should return b (else branch)
        (-10, 5, 5),     // b > a (negative), should return b
        (100, -50, 100), // a > b (negative), should return a
    ];

    let mut all_passed = true;

    for (a, b, expected) in tests {
        let result = max_fn(a, b);
        let passed = result == expected;
        let symbol = if passed { "✓" } else { "✗" };
        println!(
            "  {} max({}, {}) = {} (expected {})",
            symbol, a, b, result, expected
        );
        all_passed &= passed;
    }

    println!();

    if all_passed {
        println!("🎉 SUCCESS: All conditional branch tests passed!");
        Ok(())
    } else {
        Err("FAILED: Some conditional branches produced incorrect results".to_string())
    }
}
