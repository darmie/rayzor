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
/// Test Cranelift backend with loop constructs
///
/// **KNOWN LIMITATION - TEST CURRENTLY FAILS:**
/// This test demonstrates SSA form requirements for loops.
///
/// Issue: MIR uses Copy instructions to update loop variables (sum, i),
/// but Cranelift requires proper SSA where each value is assigned once.
///
/// Solutions:
/// 1. Use Cranelift Variable API (auto-inserts PHI nodes)
/// 2. Get proper SSA-form MIR from HIR (with explicit PHI nodes)
///
/// Current: Manually constructing MIR → Wrong SSA
/// Proper: Get MIR from HIR→MIR pipeline → Correct SSA
///
/// This test shows CFG structure works (blocks/branches/back-edges OK),
/// but execution fails due to SSA variable handling.
///
/// TODO: Re-enable after Variable API or HIR→MIR integration
///
/// Creates a function: fn sum_to_n(n: i64) -> i64 {
///     var sum = 0;
///     var i = 1;
///     while (i <= n) {
///         sum = sum + i;
///         i = i + 1;
///     }
///     return sum;
/// }
///
/// This computes the sum 1+2+3+...+n
use compiler::codegen::CraneliftBackend;
use compiler::ir::*;
use compiler::tast::SymbolId;

fn main() -> Result<(), String> {
    println!("=== Cranelift Backend Test: Loop (While) ===\n");

    // Create function: fn sum_to_n(n: i64) -> i64
    let func_id = IrFunctionId(0);
    let symbol_id = SymbolId::from_raw(1);

    // Registers
    let param_n = IrId::new(0); // Parameter: n
    let sum_reg = IrId::new(1); // Local: sum
    let i_reg = IrId::new(2); // Local: i
    let const_0 = IrId::new(3); // Constant: 0
    let const_1 = IrId::new(4); // Constant: 1
    let cond_reg = IrId::new(5); // Condition: i <= n
    let sum_new = IrId::new(6); // sum + i
    let i_new = IrId::new(7); // i + 1

    let signature = IrFunctionSignature {
        parameters: vec![IrParameter {
            name: "n".to_string(),
            ty: IrType::I64,
            reg: param_n,
            by_ref: false,
        }],
        return_type: IrType::I64,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
        uses_sret: false,
    };

    let mut function = IrFunction::new(func_id, symbol_id, "sum_to_n".to_string(), signature);

    // Add locals for all registers
    for (id, name, ty) in [
        (param_n, "n", IrType::I64),
        (sum_reg, "sum", IrType::I64),
        (i_reg, "i", IrType::I64),
        (const_0, "const_0", IrType::I64),
        (const_1, "const_1", IrType::I64),
        (cond_reg, "cond", IrType::I32),
        (sum_new, "sum_new", IrType::I64),
        (i_new, "i_new", IrType::I64),
    ] {
        function.locals.insert(
            id,
            IrLocal {
                name: name.to_string(),
                ty,
                mutable: true,
                source_location: IrSourceLocation::unknown(),
                allocation: AllocationHint::Register,
            },
        );
    }

    // Create blocks
    let entry_block = function.cfg.entry_block; // bb0: entry
    let loop_header = function.cfg.create_block(); // bb1: loop header (check condition)
    let loop_body = function.cfg.create_block(); // bb2: loop body
    let exit_block = function.cfg.create_block(); // bb3: exit (after loop)

    // Entry block: initialize sum=0, i=1, jump to loop header
    {
        let entry = function.cfg.blocks.get_mut(&entry_block).unwrap();

        // sum = 0
        entry.instructions.push(IrInstruction::Const {
            dest: const_0,
            value: IrValue::I64(0),
        });
        entry.instructions.push(IrInstruction::Copy {
            dest: sum_reg,
            src: const_0,
        });

        // i = 1
        entry.instructions.push(IrInstruction::Const {
            dest: const_1,
            value: IrValue::I64(1),
        });
        entry.instructions.push(IrInstruction::Copy {
            dest: i_reg,
            src: const_1,
        });

        // Jump to loop header
        entry.terminator = IrTerminator::Branch {
            target: loop_header,
        };
    }

    // Loop header: check condition (i <= n)
    {
        let header = function.cfg.blocks.get_mut(&loop_header).unwrap();

        // cond = i <= n
        header.instructions.push(IrInstruction::Cmp {
            dest: cond_reg,
            op: CompareOp::Le,
            left: i_reg,
            right: param_n,
        });

        // if (cond) goto loop_body else goto exit_block
        header.terminator = IrTerminator::CondBranch {
            condition: cond_reg,
            true_target: loop_body,
            false_target: exit_block,
        };
    }

    // Loop body: sum = sum + i; i = i + 1; goto loop_header
    {
        let body = function.cfg.blocks.get_mut(&loop_body).unwrap();

        // sum_new = sum + i
        body.instructions.push(IrInstruction::BinOp {
            dest: sum_new,
            op: BinaryOp::Add,
            left: sum_reg,
            right: i_reg,
        });

        // sum = sum_new
        body.instructions.push(IrInstruction::Copy {
            dest: sum_reg,
            src: sum_new,
        });

        // i_new = i + 1
        body.instructions.push(IrInstruction::BinOp {
            dest: i_new,
            op: BinaryOp::Add,
            left: i_reg,
            right: const_1,
        });

        // i = i_new
        body.instructions.push(IrInstruction::Copy {
            dest: i_reg,
            src: i_new,
        });

        // Jump back to loop header
        body.terminator = IrTerminator::Branch {
            target: loop_header,
        };
    }

    // Exit block: return sum
    {
        let exit = function.cfg.blocks.get_mut(&exit_block).unwrap();
        exit.terminator = IrTerminator::Return {
            value: Some(sum_reg),
        };
    }

    println!("Created MIR function:");
    println!("  Name: sum_to_n");
    println!("  Signature: (n: i64) -> i64");
    println!("  CFG:");
    println!("    bb0 (entry):");
    println!("      sum = 0");
    println!("      i = 1");
    println!("      goto bb1");
    println!("    bb1 (loop_header):");
    println!("      cond = (i <= n)");
    println!("      if cond goto bb2 else goto bb3");
    println!("    bb2 (loop_body):");
    println!("      sum = sum + i");
    println!("      i = i + 1");
    println!("      goto bb1");
    println!("    bb3 (exit):");
    println!("      return sum");
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

    // Test the loop function
    println!("Executing JIT-compiled function...\n");

    let sum_fn: fn(i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };

    // Test cases: sum_to_n(n) = n*(n+1)/2
    let tests = vec![
        (0, 0),      // sum_to_n(0) = 0
        (1, 1),      // sum_to_n(1) = 1
        (5, 15),     // sum_to_n(5) = 1+2+3+4+5 = 15
        (10, 55),    // sum_to_n(10) = 55
        (100, 5050), // sum_to_n(100) = 5050
    ];

    let mut all_passed = true;

    for (n, expected) in tests {
        let result = sum_fn(n);
        let passed = result == expected;
        let symbol = if passed { "✓" } else { "✗" };
        println!(
            "  {} sum_to_n({}) = {} (expected {})",
            symbol, n, result, expected
        );
        all_passed &= passed;
    }

    println!();

    if all_passed {
        println!("🎉 SUCCESS: All loop tests passed!");
        Ok(())
    } else {
        Err("FAILED: Some loop tests produced incorrect results".to_string())
    }
}
