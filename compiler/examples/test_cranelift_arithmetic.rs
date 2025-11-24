/// Test Cranelift backend with real MIR arithmetic operations
///
/// This example creates a MIR function that adds two i64 parameters and returns the result.
/// Function signature: fn add(a: i64, b: i64) -> i64 { return a + b; }

use compiler::codegen::CraneliftBackend;
use compiler::ir::*;
use compiler::tast::SymbolId;

fn main() -> Result<(), String> {
    println!("=== Cranelift Backend Test: Arithmetic Operations ===\n");

    // Create function: fn add(a: i64, b: i64) -> i64
    let func_id = IrFunctionId(0);
    let symbol_id = SymbolId::from_raw(1);

    // Create parameter registers
    let param_a = IrId::new(0);
    let param_b = IrId::new(1);
    let result_reg = IrId::new(2);

    // Create function signature
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
    };

    // Create the function
    let mut function = IrFunction::new(
        func_id,
        symbol_id,
        "add".to_string(),
        signature,
    );

    // Add local variable for result
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

    // Build the function body: result = a + b; return result;
    let entry_block = function.cfg.entry_block;
    let entry = function.cfg.blocks.get_mut(&entry_block).unwrap();

    // Instruction: result = a + b
    entry.instructions.push(IrInstruction::BinOp {
        dest: result_reg,
        op: BinaryOp::Add,
        left: param_a,
        right: param_b,
    });

    // Terminator: return result
    entry.terminator = IrTerminator::Return {
        value: Some(result_reg),
    };

    println!("Created MIR function:");
    println!("  Name: add");
    println!("  Signature: (a: i64, b: i64) -> i64");
    println!("  Instructions:");
    println!("    %2 = add %0, %1");
    println!("    return %2");
    println!();

    // Create MIR module
    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    module.functions.insert(func_id, function);

    // Initialize Cranelift backend
    println!("Initializing Cranelift backend...");
    let mut backend = CraneliftBackend::new()?;
    println!("✅ Backend initialized\n");

    // Compile the module
    println!("Compiling MIR → Cranelift IR...");
    backend.compile_module(&module)?;
    println!("✅ Compilation successful\n");

    // Get function pointer
    println!("Retrieving function pointer...");
    let func_ptr = backend.get_function_ptr(func_id)?;
    println!("✅ Function pointer: {:p}\n", func_ptr);

    // Cast to Rust function type and execute
    println!("Executing JIT-compiled function...");

    // Test case 1: add(10, 20) = 30
    let add_fn: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result1 = add_fn(10, 20);
    println!("  add(10, 20) = {}", result1);

    // Test case 2: add(100, 200) = 300
    let result2 = add_fn(100, 200);
    println!("  add(100, 200) = {}", result2);

    // Test case 3: add(-5, 15) = 10
    let result3 = add_fn(-5, 15);
    println!("  add(-5, 15) = {}", result3);

    // Test case 4: add(0, 0) = 0
    let result4 = add_fn(0, 0);
    println!("  add(0, 0) = {}", result4);

    println!("\n✅ Execution complete\n");

    // Verify results
    if result1 == 30 && result2 == 300 && result3 == 10 && result4 == 0 {
        println!("🎉 SUCCESS: All test cases passed!");
        Ok(())
    } else {
        Err(format!(
            "FAILED: Expected (30, 300, 10, 0), got ({}, {}, {}, {})",
            result1, result2, result3, result4
        ))
    }
}
