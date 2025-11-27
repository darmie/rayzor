//! Test Sys class methods

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn main() {
    println!("=== Sys Class Test ===\n");

    // Test 1: Sys.time - get timestamp
    println!("Test 1: Sys.time");
    let source1 = r#"
class Main {
    static function main() {
        var t = Sys.time();
        trace(t > 0.0);  // true - time should be positive
    }
}
"#;
    run_test(source1, "sys_time");

    // Test 2: Sys.cpuTime - get CPU time
    println!("\nTest 2: Sys.cpuTime");
    let source2 = r#"
class Main {
    static function main() {
        var t = Sys.cpuTime();
        trace(t >= 0.0);  // true - CPU time should be non-negative
    }
}
"#;
    run_test(source2, "sys_cpu_time");

    // Test 3: Sys.systemName - get OS name
    println!("\nTest 3: Sys.systemName");
    let source3 = r#"
class Main {
    static function main() {
        var name = Sys.systemName();
        trace(name);  // Should print "Mac", "Linux", or "Windows"
    }
}
"#;
    run_test(source3, "sys_system_name");

    // Test 4: Sys.getCwd - get current working directory
    println!("\nTest 4: Sys.getCwd");
    let source4 = r#"
class Main {
    static function main() {
        var cwd = Sys.getCwd();
        trace(cwd);  // Should print current directory
    }
}
"#;
    run_test(source4, "sys_get_cwd");

    // Test 5: Sys.getEnv/putEnv - environment variables
    println!("\nTest 5: Sys.getEnv/putEnv");
    let source5 = r#"
class Main {
    static function main() {
        // Set an environment variable
        Sys.putEnv("RAYZOR_TEST", "hello123");

        // Get it back
        var value = Sys.getEnv("RAYZOR_TEST");
        trace(value);  // Should print "hello123"

        // Get existing env var
        var path = Sys.getEnv("PATH");
        trace(path);  // Should print PATH value
    }
}
"#;
    run_test(source5, "sys_env");

    // Test 6: Sys.sleep - sleep for a short duration
    println!("\nTest 6: Sys.sleep");
    let source6 = r#"
class Main {
    static function main() {
        trace("Sleeping...");
        Sys.sleep(0.05);  // Sleep 50ms
        trace("Done");
    }
}
"#;
    run_test(source6, "sys_sleep");

    // Test 7: Sys.programPath - get program path
    println!("\nTest 7: Sys.programPath");
    let source7 = r#"
class Main {
    static function main() {
        var path = Sys.programPath();
        trace(path);  // Should print program path
    }
}
"#;
    run_test(source7, "sys_program_path");

    // Test 8: Sys.command - execute shell command
    println!("\nTest 8: Sys.command");
    let source8 = r#"
class Main {
    static function main() {
        // Execute a simple command that should succeed
        var exitCode = Sys.command("echo hello");
        trace(exitCode);  // 0 for success
    }
}
"#;
    run_test(source8, "sys_command");
}

fn run_test(source: &str, name: &str) {
    match compile_and_run(source, name) {
        Ok(()) => {
            println!("✅ {} PASSED", name);
        }
        Err(e) => {
            println!("❌ {} FAILED: {}", name, e);
        }
    }
}

fn compile_and_run(source: &str, name: &str) -> Result<(), String> {
    let mut unit = CompilationUnit::new(CompilationConfig::default());
    unit.load_stdlib()?;
    unit.add_file(source, &format!("{}.hx", name))?;

    let _typed_files = unit.lower_to_tast().map_err(|errors| {
        format!("TAST lowering failed: {:?}", errors)
    })?;

    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    let mut backend = compile_to_native(&mir_modules)?;
    execute_main(&mut backend, &mir_modules)?;

    Ok(())
}

fn compile_to_native(modules: &[Arc<IrModule>]) -> Result<CraneliftBackend, String> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in modules {
        backend.compile_module(module)?;
    }

    Ok(backend)
}

fn execute_main(backend: &mut CraneliftBackend, modules: &[Arc<IrModule>]) -> Result<(), String> {
    for module in modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            return Ok(());
        }
    }
    Err("Failed to execute main".to_string())
}
