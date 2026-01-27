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
//! Test Sys class functions

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use std::sync::Arc;

fn main() {
    println!("=== Sys Class Functions Test ===\n");

    // Test 1: Sys.time()
    println!("Test 1: Sys.time() - Get current time");
    let source1 = r#"
class Main {
    static function main() {
        var t = Sys.time();
        trace(t);  // Should print a timestamp (large float value)
    }
}
"#;
    run_test(source1, "sys_time");

    // Test 2: Sys.cpuTime()
    println!("\nTest 2: Sys.cpuTime() - Get CPU time");
    let source2 = r#"
class Main {
    static function main() {
        var t = Sys.cpuTime();
        trace(t);  // Should print a small non-negative float
    }
}
"#;
    run_test(source2, "sys_cpu_time");

    // Test 3: Sys.systemName()
    println!("\nTest 3: Sys.systemName() - Get OS name");
    let source3 = r#"
class Main {
    static function main() {
        var name = Sys.systemName();
        trace(name);  // Should print "Mac", "Linux", "Windows", or "BSD"
    }
}
"#;
    run_test(source3, "sys_system_name");

    // Test 4: Sys.getCwd()
    println!("\nTest 4: Sys.getCwd() - Get current working directory");
    let source4 = r#"
class Main {
    static function main() {
        var cwd = Sys.getCwd();
        trace(cwd);  // Should print current directory
    }
}
"#;
    run_test(source4, "sys_get_cwd");

    // Test 5: Sys.getEnv() / Sys.putEnv()
    println!("\nTest 5: Sys.getEnv/putEnv - Environment variables");
    let source5 = r#"
class Main {
    static function main() {
        // Get PATH which should exist on all systems
        var path = Sys.getEnv("PATH");
        // PATH should not be null
        trace(path != null);  // Should print true
    }
}
"#;
    run_test(source5, "sys_env");

    // Test 6: Sys.sleep() - Quick test
    println!("\nTest 6: Sys.sleep() - Sleep for 0.01 seconds");
    let source6 = r#"
class Main {
    static function main() {
        trace("before sleep");
        Sys.sleep(0.01);
        trace("after sleep");
    }
}
"#;
    run_test(source6, "sys_sleep");
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

    let _typed_files = unit
        .lower_to_tast()
        .map_err(|errors| format!("TAST lowering failed: {:?}", errors))?;

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
