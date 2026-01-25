//! Minimal test for nbody benchmark crash

use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::cranelift_backend::CraneliftBackend;
use compiler::ir::optimization::{PassManager, OptimizationLevel};
use std::sync::Arc;
use std::fs;

fn get_runtime_symbols() -> Vec<(&'static str, *const u8)> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    symbols.iter().map(|(n, p)| (*n, *p)).collect()
}

fn main() {
    // Enable RUST_LOG=debug to see field access debug messages
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let symbols = get_runtime_symbols();

    // Load nbody source
    let source = fs::read_to_string("benchmarks/src/nbody.hx").expect("read nbody.hx");

    eprintln!("[TEST] Compiling nbody...");

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(&source, "nbody.hx").expect("parse");
    unit.lower_to_tast().expect("tast");

    let mut mir_modules = unit.get_mir_modules();
    eprintln!("[TEST] Got {} MIR modules", mir_modules.len());

    // Skip MIR optimizations to see if they're causing the issue
    // let mut pass_manager = PassManager::for_level(OptimizationLevel::O2);
    // for module in &mut mir_modules {
    //     let module_mut = Arc::make_mut(module);
    //     let _ = pass_manager.run(module_mut);
    // }

    // Dump the initBodies and energy function MIR with instructions
    eprintln!("[TEST] Looking at initBodies and energy functions:");
    for module in &mir_modules {
        for (id, func) in &module.functions {
            if func.name == "energy" || func.name == "initBodies" {
                eprintln!("[TEST]   {:?}: {} (params={}) ret={:?}", id, func.name, func.signature.parameters.len(), func.signature.return_type);
                eprintln!("[TEST]   {} blocks", func.cfg.blocks.len());
                for (block_id, block) in &func.cfg.blocks {
                    eprintln!("[TEST]   Block {:?} ({} instrs):", block_id, block.instructions.len());
                    // Dump first 20 instructions to see field access patterns
                    for (i, inst) in block.instructions.iter().take(20).enumerate() {
                        eprintln!("[TEST]     [{}] {:?}", i, inst);
                    }
                    if block.instructions.len() > 20 {
                        eprintln!("[TEST]     ... ({} more)", block.instructions.len() - 20);
                    }
                    eprintln!("[TEST]     terminator: {:?}", block.terminator);
                }
            }
        }
    }

    eprintln!("[TEST] Creating Cranelift backend...");
    let mut backend = CraneliftBackend::with_symbols(&symbols).expect("backend");

    eprintln!("[TEST] Compiling modules...");
    for module in &mir_modules {
        backend.compile_module(module).expect("compile");
    }

    eprintln!("[TEST] Calling main...");
    for module in mir_modules.iter().rev() {
        if let Ok(()) = backend.call_main(module) {
            eprintln!("[TEST] Success!");
            break;
        }
    }
}
