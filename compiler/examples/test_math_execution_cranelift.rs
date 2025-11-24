/// Test Math operations with actual Cranelift JIT execution
///
/// This example:
/// 1. Creates MIR functions that call Math operations
/// 2. Compiles them with Cranelift JIT
/// 3. Executes them and shows real results

use compiler::codegen::CraneliftBackend;
use compiler::ir::*;
use compiler::tast::SymbolId;

fn main() -> Result<(), String> {
    println!("=== Math Operations with Cranelift JIT Execution ===\n");

    // Test 1: Math.abs(-5.0)
    test_math_abs()?;

    // Test 2: Math.sqrt(16.0)
    test_math_sqrt()?;

    // Test 3: Math.min(3.0, 7.0)
    test_math_min()?;

    // Test 4: Math.max(3.0, 7.0)
    test_math_max()?;

    // Test 5: Math.sin(0.0)
    test_math_sin()?;

    println!("\n🎉 All Math tests passed!");
    Ok(())
}

fn test_math_abs() -> Result<(), String> {
    println!("TEST: Math.abs(-5.0)");
    println!("{}", "─".repeat(50));

    let func_id = IrFunctionId(0);
    let symbol_id = SymbolId::from_raw(1);

    // Create function: () -> f64 that calls Math.abs(-5.0)
    let signature = IrFunctionSignature {
        parameters: vec![],
        return_type: IrType::F64,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
    };

    let mut function = IrFunction::new(
        func_id,
        symbol_id,
        "test_abs".to_string(),
        signature,
    );

    // Create basic block with Math.abs call
    let block_id = function.cfg.create_block();
    function.cfg.entry = block_id;

    // Create the constant -5.0
    let neg_five = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Constant {
        dest: neg_five,
        value: IrConstant::F64(-5.0),
    });

    // Call haxe_math_abs
    let result = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Call {
        dest: Some(result),
        function_name: "haxe_math_abs".to_string(),
        arguments: vec![neg_five],
    });

    // Return the result
    function.cfg.blocks.get_mut(&block_id).unwrap().terminator = Some(IrTerminator::Return {
        value: Some(result),
    });

    // Compile and execute
    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    module.functions.insert(func_id, function);

    let mut backend = CraneliftBackend::new()?;
    backend.compile_module(&module)?;

    let func_ptr = backend.get_function_ptr(func_id)?;
    let func: fn() -> f64 = unsafe { std::mem::transmute(func_ptr) };
    let result = func();

    println!("  Input: -5.0");
    println!("  Output: {}", result);
    println!("  Expected: 5.0");

    if (result - 5.0).abs() < 0.0001 {
        println!("  ✅ PASSED\n");
        Ok(())
    } else {
        Err(format!("Math.abs failed: expected 5.0, got {}", result))
    }
}

fn test_math_sqrt() -> Result<(), String> {
    println!("TEST: Math.sqrt(16.0)");
    println!("{}", "─".repeat(50));

    let func_id = IrFunctionId(1);
    let symbol_id = SymbolId::from_raw(2);

    let signature = IrFunctionSignature {
        parameters: vec![],
        return_type: IrType::F64,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
    };

    let mut function = IrFunction::new(
        func_id,
        symbol_id,
        "test_sqrt".to_string(),
        signature,
    );

    let block_id = function.cfg.create_block();
    function.cfg.entry = block_id;

    let sixteen = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Constant {
        dest: sixteen,
        value: IrConstant::F64(16.0),
    });

    let result = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Call {
        dest: Some(result),
        function_name: "haxe_math_sqrt".to_string(),
        arguments: vec![sixteen],
    });

    function.cfg.blocks.get_mut(&block_id).unwrap().terminator = Some(IrTerminator::Return {
        value: Some(result),
    });

    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    module.functions.insert(func_id, function);

    let mut backend = CraneliftBackend::new()?;
    backend.compile_module(&module)?;

    let func_ptr = backend.get_function_ptr(func_id)?;
    let func: fn() -> f64 = unsafe { std::mem::transmute(func_ptr) };
    let result = func();

    println!("  Input: 16.0");
    println!("  Output: {}", result);
    println!("  Expected: 4.0");

    if (result - 4.0).abs() < 0.0001 {
        println!("  ✅ PASSED\n");
        Ok(())
    } else {
        Err(format!("Math.sqrt failed: expected 4.0, got {}", result))
    }
}

fn test_math_min() -> Result<(), String> {
    println!("TEST: Math.min(3.0, 7.0)");
    println!("{}", "─".repeat(50));

    let func_id = IrFunctionId(2);
    let symbol_id = SymbolId::from_raw(3);

    let signature = IrFunctionSignature {
        parameters: vec![],
        return_type: IrType::F64,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
    };

    let mut function = IrFunction::new(
        func_id,
        symbol_id,
        "test_min".to_string(),
        signature,
    );

    let block_id = function.cfg.create_block();
    function.cfg.entry = block_id;

    let three = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Constant {
        dest: three,
        value: IrConstant::F64(3.0),
    });

    let seven = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Constant {
        dest: seven,
        value: IrConstant::F64(7.0),
    });

    let result = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Call {
        dest: Some(result),
        function_name: "haxe_math_min".to_string(),
        arguments: vec![three, seven],
    });

    function.cfg.blocks.get_mut(&block_id).unwrap().terminator = Some(IrTerminator::Return {
        value: Some(result),
    });

    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    module.functions.insert(func_id, function);

    let mut backend = CraneliftBackend::new()?;
    backend.compile_module(&module)?;

    let func_ptr = backend.get_function_ptr(func_id)?;
    let func: fn() -> f64 = unsafe { std::mem::transmute(func_ptr) };
    let result = func();

    println!("  Input: 3.0, 7.0");
    println!("  Output: {}", result);
    println!("  Expected: 3.0");

    if (result - 3.0).abs() < 0.0001 {
        println!("  ✅ PASSED\n");
        Ok(())
    } else {
        Err(format!("Math.min failed: expected 3.0, got {}", result))
    }
}

fn test_math_max() -> Result<(), String> {
    println!("TEST: Math.max(3.0, 7.0)");
    println!("{}", "─".repeat(50));

    let func_id = IrFunctionId(3);
    let symbol_id = SymbolId::from_raw(4);

    let signature = IrFunctionSignature {
        parameters: vec![],
        return_type: IrType::F64,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
    };

    let mut function = IrFunction::new(
        func_id,
        symbol_id,
        "test_max".to_string(),
        signature,
    );

    let block_id = function.cfg.create_block();
    function.cfg.entry = block_id;

    let three = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Constant {
        dest: three,
        value: IrConstant::F64(3.0),
    });

    let seven = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Constant {
        dest: seven,
        value: IrConstant::F64(7.0),
    });

    let result = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Call {
        dest: Some(result),
        function_name: "haxe_math_max".to_string(),
        arguments: vec![three, seven],
    });

    function.cfg.blocks.get_mut(&block_id).unwrap().terminator = Some(IrTerminator::Return {
        value: Some(result),
    });

    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    module.functions.insert(func_id, function);

    let mut backend = CraneliftBackend::new()?;
    backend.compile_module(&module)?;

    let func_ptr = backend.get_function_ptr(func_id)?;
    let func: fn() -> f64 = unsafe { std::mem::transmute(func_ptr) };
    let result = func();

    println!("  Input: 3.0, 7.0");
    println!("  Output: {}", result);
    println!("  Expected: 7.0");

    if (result - 7.0).abs() < 0.0001 {
        println!("  ✅ PASSED\n");
        Ok(())
    } else {
        Err(format!("Math.max failed: expected 7.0, got {}", result))
    }
}

fn test_math_sin() -> Result<(), String> {
    println!("TEST: Math.sin(0.0)");
    println!("{}", "─".repeat(50));

    let func_id = IrFunctionId(4);
    let symbol_id = SymbolId::from_raw(5);

    let signature = IrFunctionSignature {
        parameters: vec![],
        return_type: IrType::F64,
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
    };

    let mut function = IrFunction::new(
        func_id,
        symbol_id,
        "test_sin".to_string(),
        signature,
    );

    let block_id = function.cfg.create_block();
    function.cfg.entry = block_id;

    let zero = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Constant {
        dest: zero,
        value: IrConstant::F64(0.0),
    });

    let result = function.create_temp(IrType::F64);
    function.cfg.blocks.get_mut(&block_id).unwrap().instructions.push(IrInstruction::Call {
        dest: Some(result),
        function_name: "haxe_math_sin".to_string(),
        arguments: vec![zero],
    });

    function.cfg.blocks.get_mut(&block_id).unwrap().terminator = Some(IrTerminator::Return {
        value: Some(result),
    });

    let mut module = IrModule::new("test".to_string(), "test.hx".to_string());
    module.functions.insert(func_id, function);

    let mut backend = CraneliftBackend::new()?;
    backend.compile_module(&module)?;

    let func_ptr = backend.get_function_ptr(func_id)?;
    let func: fn() -> f64 = unsafe { std::mem::transmute(func_ptr) };
    let result = func();

    println!("  Input: 0.0");
    println!("  Output: {}", result);
    println!("  Expected: 0.0");

    if result.abs() < 0.0001 {
        println!("  ✅ PASSED\n");
        Ok(())
    } else {
        Err(format!("Math.sin failed: expected 0.0, got {}", result))
    }
}
