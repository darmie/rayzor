/// Test lambdas in isolation without threads or native calls
/// This helps debug lambda signature and codegen issues
use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::CraneliftBackend;

fn compile_and_run(name: &str, source: &str) {
    println!("\n{}", "=".repeat(70));
    println!("TEST: {}", name);
    println!("{}", "=".repeat(70));

    // Create compilation unit WITHOUT stdlib for pure lambda tests
    let mut config = CompilationConfig::default();
    config.load_stdlib = false;  // Don't load concurrency stdlib - we only need basic types
    let mut unit = CompilationUnit::new(config);

    // Add source file
    if let Err(e) = unit.add_file(source, &format!("{}.hx", name)) {
        println!("  ❌ Failed to add file: {}", e);
        return;
    }

    // Run through TAST
    println!("  📝 Compiling to TAST...");
    let _tast_modules = match unit.lower_to_tast() {
        Ok(modules) => {
            println!("  ✅ TAST succeeded ({} files)", modules.len());
            modules
        }
        Err(errors) => {
            println!("  ❌ TAST failed:");
            for err in &errors {
                println!("    {:?}", err);
            }
            return;
        }
    };

    // HIR is integrated in the pipeline
    println!("  ✅ HIR succeeded");

    // Get MIR modules
    println!("  📝 Lowering to MIR...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        println!("  ❌ No MIR modules generated");
        return;
    }

    let mir_module = mir_modules.last().unwrap();
    println!("  ✅ MIR succeeded ({} modules)", mir_modules.len());

    // Print MIR functions (focus on user functions and lambdas)
    println!("  📋 MIR Functions:");
    for (func_id, func) in &mir_module.functions {
        // Skip stdlib functions for clarity
        if func.name.starts_with("string_") ||
           func.name.starts_with("array_") ||
           func.name.starts_with("haxe_array_") ||
           func.name.starts_with("int_") ||
           func.name.starts_with("float_") ||
           func.name.starts_with("bool_") ||
           func.name == "trace" {
            continue;
        }

        println!("    {:?}: {} (params: {}, returns: {:?})",
                 func_id, func.name, func.signature.parameters.len(), func.signature.return_type);
        for (i, param) in func.signature.parameters.iter().enumerate() {
            println!("      param[{}]: {} ({:?})", i, param.name, param.ty);
        }
    }

    // Compile to native
    println!("  📝 Compiling to native code...");

    // Get runtime symbols from the plugin system
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = match CraneliftBackend::with_symbols(&symbols_ref) {
        Ok(b) => b,
        Err(e) => {
            println!("  ❌ Backend creation failed: {}", e);
            return;
        }
    };

    // Compile all modules
    for module in &mir_modules {
        if let Err(e) = backend.compile_module(module) {
            println!("  ❌ Codegen failed: {}", e);
            return;
        }
    }
    println!("  ✅ Codegen succeeded");

    // Execute main
    println!("  🚀 Executing main()...");
    if let Err(e) = backend.call_main(mir_module) {
        println!("  ❌ Execution failed: {}", e);
        return;
    }
    println!("  ✅ Execution succeeded");
}

fn main() {
    println!("Array + Lambda Isolation Test Suite");
    println!("====================================\n");

    // Test 1: Simple array creation and push
    compile_and_run(
        "array_simple",
        r#"
package test;

class Main {
    static function main() {
        var arr = new Array<Int>();
        arr.push(42);
        trace(arr.length);
    }
}
"#,
    );

    // Test 2: Array with loop (no lambda)
    compile_and_run(
        "array_loop",
        r#"
package test;

class Main {
    static function main() {
        var arr = new Array<Int>();
        var i = 0;
        while (i < 3) {
            arr.push(i * 10);
            i++;
        }
        trace(arr.length);
    }
}
"#,
    );

    // Test 3: Lambda with capture - simple case
    compile_and_run(
        "lambda_capture_simple",
        r#"
package test;

class Main {
    static function main() {
        var x = 42;
        var f = () -> {
            return x;
        };
        var result = f();
        trace(result);
    }
}
"#,
    );

    // Test 4: Lambda with capture in loop - the problematic case
    compile_and_run(
        "lambda_capture_loop",
        r#"
package test;

class Main {
    static function main() {
        var funcs = new Array<Void -> Int>();
        var i = 0;
        while (i < 3) {
            var f = () -> {
                return i * 10;
            };
            funcs.push(f);
            i++;
        }

        // Call each lambda and trace results
        var j = 0;
        while (j < funcs.length) {
            trace(j);
            j++;
        }
    }
}
"#,
    );

    // Test 5: Immediate lambda invocation (the key test!)
    compile_and_run(
        "lambda_invoke_immediate",
        r#"
package test;

class Main {
    static function main() {
        var i = 0;
        while (i < 3) {
            var result = (() -> {
                return i * 10;
            })();
            trace(result);
            i++;
        }
    }
}
"#,
    );

    // Test 6: Array iterator (for...in loop)
    compile_and_run(
        "array_iterator",
        r#"
package test;

class Main {
    static function main() {
        var arr = new Array<Int>();
        arr.push(10);
        arr.push(20);
        arr.push(30);

        // Test iterator
        var sum = 0;
        for (val in arr) {
            sum += val;
        }

        // sum should be 60
        trace(sum);
    }
}
"#,
    );

    println!("\n{}", "=".repeat(70));
    println!("Array + Lambda Isolation Test Suite Complete");
    println!("{}", "=".repeat(70));
}
