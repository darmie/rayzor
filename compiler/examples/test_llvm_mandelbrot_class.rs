//! Test LLVM JIT with mandelbrot (class-based version)
//!
//! This tests the original mandelbrot.hx which uses Complex classes
//! and heap allocation. Without proper drop analysis, this will leak memory.

#[cfg(feature = "llvm-backend")]
use compiler::codegen::LLVMJitBackend;
#[cfg(feature = "llvm-backend")]
use inkwell::context::Context;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn get_runtime_symbols() -> Vec<(&'static str, *const u8)> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    plugin.runtime_symbols().iter().map(|(n, p)| (*n, *p)).collect()
}

fn main() {
    #[cfg(not(feature = "llvm-backend"))]
    {
        eprintln!("LLVM backend not enabled. Run with: cargo run --release --features llvm-backend --example test_llvm_mandelbrot_class");
        return;
    }

    #[cfg(feature = "llvm-backend")]
    {
        // Use original mandelbrot.hx (class-based with heap allocations)
        // This tests drop analysis - without proper Free instructions, memory will leak
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/benchmarks/src/mandelbrot.hx")
        ).expect("read");
        let symbols = get_runtime_symbols();

        println!("Compiling mandelbrot (class-based) with LLVM...");
        println!("NOTE: This version uses Complex classes and heap allocation.");
        println!("If memory usage explodes, drop analysis may be broken.");

        let mut unit = CompilationUnit::new(CompilationConfig::fast());
        unit.load_stdlib().expect("stdlib");
        unit.add_file(&source, "mandelbrot.hx").expect("parse");
        unit.lower_to_tast().expect("tast");
        let mir_modules = unit.get_mir_modules();

        println!("Got {} MIR modules", mir_modules.len());

        let context = Context::create();
        let mut backend = LLVMJitBackend::with_symbols(&context, &symbols).expect("backend");

        println!("Declaring modules...");
        for module in &mir_modules {
            backend.declare_module(module).expect("declare");
        }

        println!("Compiling bodies...");
        for module in &mir_modules {
            backend.compile_module_bodies(module).expect("compile");
        }

        println!("Finalizing (running -O3 optimization)...");
        backend.finalize().expect("finalize");

        println!("Calling main...");
        for module in mir_modules.iter().rev() {
            if backend.call_main(module).is_ok() {
                break;
            }
        }
        println!("Done!");
    }
}
