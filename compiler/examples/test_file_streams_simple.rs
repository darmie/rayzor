//! Simple test for File.read/write

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn main() {
    println!("=== Simple File Stream Test ===\n");

    // Baseline: simple program without sys.io
    println!("Test 0: Baseline (no sys.io imports)");
    let baseline = r#"
class Main {
    static function main() {
        trace("Hello");
        var x = 1 + 2;
        trace(x);
    }
}
"#;
    let _ = compile_and_run(baseline, "baseline");

    // Test File.write and writeByte, File.read and readByte
    println!("\nTest 1: FileOutput and FileInput stream operations");
    let source = r#"
import sys.io.File;
import sys.io.FileOutput;
import sys.io.FileInput;
import sys.FileSystem;

class Main {
    static function main() {
        trace("=== Test FileOutput ===");
        var output:FileOutput = File.write("/tmp/rayzor_simple_test.txt", true);

        // Write some bytes
        output.writeByte(72);  // 'H'
        output.writeByte(105); // 'i'
        output.writeByte(33);  // '!'
        output.close();
        trace("Wrote Hi!");

        trace("=== Test FileInput ===");
        var input:FileInput = File.read("/tmp/rayzor_simple_test.txt", true);

        // Read bytes
        var b1 = input.readByte();
        var b2 = input.readByte();
        var b3 = input.readByte();
        input.close();

        trace(b1);  // 72
        trace(b2);  // 105
        trace(b3);  // 33

        FileSystem.deleteFile("/tmp/rayzor_simple_test.txt");
        trace("Done!");
    }
}
"#;

    match compile_and_run(source, "simple_test") {
        Ok(()) => {
            println!("✅ Test completed");
        }
        Err(e) => {
            println!("❌ FAILED: {}", e);
        }
    }

    // Compare speed vs fast compilation modes
    println!("\n=== Cranelift Mode Comparison ===");

    let comparison_source = r#"
class Main {
    static function main() {
        var sum = 0;
        for (i in 0...100) {
            sum += i;
        }
        trace(sum);
    }
}
"#;

    println!("\nTest 2a: Speed mode (optimized)");
    let _ = compile_and_run_with_mode(comparison_source, "speed_test", false);

    println!("\nTest 2b: Fast mode (no optimization)");
    let _ = compile_and_run_with_mode(comparison_source, "fast_test", true);
}

fn compile_and_run(source: &str, name: &str) -> Result<(), String> {
    compile_and_run_with_mode(source, name, false)
}

fn compile_and_run_with_mode(source: &str, name: &str, fast_mode: bool) -> Result<(), String> {
    use std::time::Instant;

    let t0 = Instant::now();
    let mut unit = CompilationUnit::new(CompilationConfig::default());
    unit.load_stdlib()?;
    unit.add_file(source, &format!("{}.hx", name))?;
    eprintln!("[PROFILE] Load stdlib + add file: {:?}", t0.elapsed());

    let t1 = Instant::now();
    let _typed_files = unit.lower_to_tast().map_err(|errors| {
        format!("TAST lowering failed: {:?}", errors)
    })?;
    eprintln!("[PROFILE] TAST lowering: {:?}", t1.elapsed());

    let t2 = Instant::now();
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }
    eprintln!("[PROFILE] Get MIR modules: {:?}", t2.elapsed());

    let t3 = Instant::now();
    let mut backend = compile_to_native_with_mode(&mir_modules, fast_mode)?;
    eprintln!("[PROFILE] Compile to native: {:?}", t3.elapsed());

    let t4 = Instant::now();
    execute_main(&mut backend, &mir_modules)?;
    eprintln!("[PROFILE] Execute: {:?}", t4.elapsed());

    Ok(())
}

fn compile_to_native(modules: &[Arc<IrModule>]) -> Result<CraneliftBackend, String> {
    compile_to_native_with_mode(modules, false)
}

fn compile_to_native_with_mode(modules: &[Arc<IrModule>], fast_mode: bool) -> Result<CraneliftBackend, String> {
    use std::time::Instant;

    let t0 = Instant::now();
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = if fast_mode {
        CraneliftBackend::with_fast_compilation(&symbols_ref)?
    } else {
        CraneliftBackend::with_symbols(&symbols_ref)?
    };
    let mode_str = if fast_mode { "fast" } else { "speed" };
    eprintln!("  [Cranelift-{}] Backend creation: {:?}", mode_str, t0.elapsed());

    let mut total_funcs = 0;
    let mut total_impl_funcs = 0;
    for (i, module) in modules.iter().enumerate() {
        let t1 = Instant::now();
        let func_count = module.functions.len();
        let impl_count = module.functions.values().filter(|f| !f.cfg.blocks.is_empty()).count();
        total_funcs += func_count;
        total_impl_funcs += impl_count;
        backend.compile_module(module)?;
        eprintln!("  [Cranelift-{}] Module {} '{}': {} funcs ({} impl) in {:?}",
                  mode_str, i, module.name, func_count, impl_count, t1.elapsed());
    }
    eprintln!("  [Cranelift-{}] Total: {} modules, {} funcs ({} impl)",
              mode_str, modules.len(), total_funcs, total_impl_funcs);

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
