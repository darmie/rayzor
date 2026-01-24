//! Rayzor Benchmark Suite Runner
//!
//! Compares Rayzor execution modes against each other and external targets.
//!
//! Usage:
//!   cargo run --release --package compiler --example benchmark_runner
//!   cargo run --release --package compiler --example benchmark_runner -- mandelbrot
//!   cargo run --release --package compiler --example benchmark_runner -- --json

use compiler::codegen::tiered_backend::{TieredBackend, TieredConfig, TierPreset};
use compiler::codegen::CraneliftBackend;
use compiler::codegen::InterpValue;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::{IrFunctionId, IrModule, RayzorBundle, load_bundle};
use compiler::ir::optimization::{PassManager, OptimizationLevel};

#[cfg(feature = "llvm-backend")]
use compiler::codegen::LLVMJitBackend;
#[cfg(feature = "llvm-backend")]
use compiler::codegen::init_llvm_once;
#[cfg(feature = "llvm-backend")]
use inkwell::context::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const WARMUP_RUNS: usize = 15;  // Increased to ensure LLVM promotion during warmup
const BENCH_RUNS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkResult {
    name: String,
    target: String,
    compile_time_ms: f64,
    runtime_ms: f64,
    total_time_ms: f64,
    iterations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkSuite {
    date: String,
    benchmarks: Vec<BenchmarkResults>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkResults {
    name: String,
    results: Vec<BenchmarkResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    RayzorInterpreter,
    RayzorCranelift,
    RayzorTiered,
    RayzorPrecompiled,       // .rzb pre-bundled MIR (skips parse/lower, still JITs)
    RayzorPrecompiledTiered, // .rzb pre-bundled MIR + tiered warmup + LLVM
    #[cfg(feature = "llvm-backend")]
    RayzorLLVM,
}

impl Target {
    fn name(&self) -> &'static str {
        match self {
            Target::RayzorInterpreter => "rayzor-interpreter",
            Target::RayzorCranelift => "rayzor-cranelift",
            Target::RayzorTiered => "rayzor-tiered",
            Target::RayzorPrecompiled => "rayzor-precompiled",
            Target::RayzorPrecompiledTiered => "rayzor-precompiled-tiered",
            #[cfg(feature = "llvm-backend")]
            Target::RayzorLLVM => "rayzor-llvm",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Target::RayzorInterpreter => "MIR Interpreter (instant startup)",
            Target::RayzorCranelift => "Cranelift JIT (compile from source)",
            Target::RayzorTiered => "Tiered (source -> interp -> Cranelift)",
            Target::RayzorPrecompiled => "Pre-bundled MIR + JIT (skip parsing)",
            Target::RayzorPrecompiledTiered => "Pre-bundled MIR + tiered + LLVM",
            #[cfg(feature = "llvm-backend")]
            Target::RayzorLLVM => "LLVM JIT (-O3, maximum optimization)",
        }
    }
}

struct Benchmark {
    name: String,
    source: String,
}

fn get_runtime_symbols() -> Vec<(&'static str, *const u8)> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    symbols.iter().map(|(n, p)| (*n, *p)).collect()
}

fn load_benchmark(name: &str) -> Option<Benchmark> {
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks/src");
    let file_path = base_path.join(format!("{}.hx", name));

    if file_path.exists() {
        let source = fs::read_to_string(&file_path).ok()?;
        Some(Benchmark {
            name: name.to_string(),
            source,
        })
    } else {
        None
    }
}

/// Check if a precompiled .rzb bundle exists for this benchmark
fn has_precompiled_bundle(name: &str) -> bool {
    get_precompiled_path(name).exists()
}

/// Get the path to the precompiled .rzb bundle
fn get_precompiled_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benchmarks/precompiled")
        .join(format!("{}.rzb", name))
}

/// Run benchmark using precompiled .rzb bundle
/// This measures the performance benefit of skipping source parsing/lowering
/// The .rzb bundle contains pre-compiled MIR that still needs JIT compilation
fn run_benchmark_precompiled(name: &str, symbols: &[(&str, *const u8)]) -> Result<(Duration, Duration), String> {
    let bundle_path = get_precompiled_path(name);

    // Load time = "compile time" for precompiled (should be ~500µs)
    let load_start = Instant::now();

    let bundle = load_bundle(&bundle_path)
        .map_err(|e| format!("load bundle: {:?}", e))?;

    // Get entry function ID (pre-computed in bundle for O(1) lookup)
    let entry_func_id = bundle.entry_function_id()
        .ok_or("No entry function ID in bundle")?;

    // Use Script preset for single-run execution - starts with Cranelift JIT
    // (Benchmark preset starts interpreted, which would be slower for single run)
    let mut config = TierPreset::Script.to_config();
    config.start_interpreted = false;  // Start with Cranelift (Baseline tier)
    config.verbosity = 0;

    let mut backend = TieredBackend::with_symbols(config, symbols)
        .map_err(|e| format!("backend: {}", e))?;

    // Load ALL modules from bundle (not just entry)
    for module in bundle.modules() {
        backend.compile_module(module.clone())
            .map_err(|e| format!("load module: {}", e))?;
    }

    let load_time = load_start.elapsed();

    // Execute
    let exec_start = Instant::now();
    backend.execute_function(entry_func_id, vec![])
        .map_err(|e| format!("exec: {}", e))?;
    let exec_time = exec_start.elapsed();

    Ok((load_time, exec_time))
}

/// Precompiled-tiered benchmark state - loads from .rzb then warms up through tiers
struct PrecompiledTieredState {
    backend: TieredBackend,
    main_id: IrFunctionId,
    load_time: Duration,
}

/// Setup precompiled-tiered benchmark: load .rzb bundle then warm up with tier promotion
fn setup_precompiled_tiered_benchmark(name: &str, symbols: &[(&str, *const u8)]) -> Result<PrecompiledTieredState, String> {
    let bundle_path = get_precompiled_path(name);
    let load_start = Instant::now();

    let bundle = load_bundle(&bundle_path)
        .map_err(|e| format!("load bundle: {:?}", e))?;

    // Get entry function ID
    let main_id = bundle.entry_function_id()
        .ok_or("No entry function ID in bundle")?;

    // Use Benchmark preset - fast tier promotion, immediate bailout
    let config = TierPreset::Benchmark.to_config();

    let mut backend = TieredBackend::with_symbols(config, symbols)
        .map_err(|e| format!("backend: {}", e))?;

    // Load ALL modules from bundle
    for module in bundle.modules() {
        backend.compile_module(module.clone())
            .map_err(|e| format!("load module: {}", e))?;
    }

    let load_time = load_start.elapsed();

    Ok(PrecompiledTieredState { backend, main_id, load_time })
}

fn run_precompiled_tiered_iteration(state: &mut PrecompiledTieredState) -> Result<Duration, String> {
    let exec_start = Instant::now();
    state.backend.execute_function(state.main_id, vec![])
        .map_err(|e| format!("exec: {}", e))?;
    Ok(exec_start.elapsed())
}

fn list_benchmarks() -> Vec<String> {
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks/src");
    let mut benchmarks = Vec::new();

    if let Ok(entries) = fs::read_dir(&base_path) {
        for entry in entries.flatten() {
            if let Some(name) = entry.path().file_stem() {
                if entry.path().extension().map_or(false, |e| e == "hx") {
                    benchmarks.push(name.to_string_lossy().to_string());
                }
            }
        }
    }

    benchmarks.sort();
    benchmarks
}

fn run_benchmark_cranelift(bench: &Benchmark, symbols: &[(&str, *const u8)]) -> Result<(Duration, Duration), String> {
    // Compile
    let compile_start = Instant::now();

    // Use fast() for lazy stdlib - avoids trace resolution issues
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
    unit.add_file(&bench.source, &format!("{}.hx", bench.name))
        .map_err(|e| format!("parse: {}", e))?;
    unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

    let mut mir_modules = unit.get_mir_modules();

    // Apply MIR optimizations (O2) for fair comparison with tiered backend
    // Tiered reaches "Optimized" tier which uses O2/O3 MIR opts + Cranelift "speed"
    let mut pass_manager = PassManager::for_level(OptimizationLevel::O2);
    for module in &mut mir_modules {
        // Get mutable access to the module through Arc::make_mut
        let module_mut = std::sync::Arc::make_mut(module);
        let _ = pass_manager.run(module_mut);
    }

    let mut backend = CraneliftBackend::with_symbols(symbols)
        .map_err(|e| format!("backend: {}", e))?;

    for module in &mir_modules {
        backend.compile_module(module).map_err(|e| format!("compile: {}", e))?;
    }

    let compile_time = compile_start.elapsed();

    // Execute
    let exec_start = Instant::now();
    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            break;
        }
    }
    let exec_time = exec_start.elapsed();

    Ok((compile_time, exec_time))
}

fn run_benchmark_interpreter(bench: &Benchmark, symbols: &[(&str, *const u8)]) -> Result<(Duration, Duration), String> {
    // Compile (to MIR only)
    let compile_start = Instant::now();

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
    unit.add_file(&bench.source, &format!("{}.hx", bench.name))
        .map_err(|e| format!("parse: {}", e))?;
    unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

    let mir_modules = unit.get_mir_modules();

    // Use Embedded preset - interpreter only, never promotes to JIT
    // This measures pure MIR interpreter performance
    let config = TierPreset::Embedded.to_config();

    let mut backend = TieredBackend::with_symbols(config, symbols)
        .map_err(|e| format!("backend: {}", e))?;

    // Find main module
    let main_module = mir_modules.iter().rev()
        .find(|m| m.functions.values().any(|f| f.name.ends_with("_main") || f.name == "main"))
        .ok_or("No main module")?;

    let main_id = main_module.functions.iter()
        .find(|(_, f)| f.name.ends_with("_main") || f.name == "main")
        .map(|(id, _)| *id)
        .ok_or("No main function")?;

    backend.compile_module((**main_module).clone())
        .map_err(|e| format!("load: {}", e))?;

    let compile_time = compile_start.elapsed();

    // Execute
    let exec_start = Instant::now();
    backend.execute_function(main_id, vec![])
        .map_err(|e| format!("exec: {}", e))?;
    let exec_time = exec_start.elapsed();

    Ok((compile_time, exec_time))
}

/// Tiered benchmark state - persisted across iterations to allow JIT promotion
struct TieredBenchmarkState {
    backend: TieredBackend,
    main_id: IrFunctionId,
    compile_time: Duration,
}

/// Heavy benchmarks that should skip interpreter and start at Baseline (Cranelift)
fn is_heavy_benchmark(name: &str) -> bool {
    matches!(name, "mandelbrot" | "mandelbrot_simple" | "nbody")
}

fn setup_tiered_benchmark(bench: &Benchmark, symbols: &[(&str, *const u8)]) -> Result<TieredBenchmarkState, String> {
    let compile_start = Instant::now();

    // Use fast() for lazy stdlib - avoids trace resolution issues
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
    unit.add_file(&bench.source, &format!("{}.hx", bench.name))
        .map_err(|e| format!("parse: {}", e))?;
    unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

    let mir_modules = unit.get_mir_modules();

    // Use Benchmark preset - optimized for performance testing
    // - Fast tier promotion (thresholds: 2, 3, 5)
    // - Immediate bailout from interpreter hot loops
    // - Synchronous optimization for deterministic results
    // - Manual LLVM upgrade after warmup (blazing_threshold = MAX)
    let config = TierPreset::Benchmark.to_config();

    let mut backend = TieredBackend::with_symbols(config, symbols)
        .map_err(|e| format!("backend: {}", e))?;

    // Compile ALL modules (like the direct LLVM benchmark does)
    for module in &mir_modules {
        backend.compile_module((**module).clone())
            .map_err(|e| format!("load: {}", e))?;
    }

    // Find the main function ID
    let main_id = mir_modules.iter().rev()
        .find_map(|m| {
            m.functions.iter()
                .find(|(_, f)| f.name.ends_with("_main") || f.name == "main")
                .map(|(id, _)| *id)
        })
        .ok_or("No main function")?;

    let compile_time = compile_start.elapsed();

    Ok(TieredBenchmarkState { backend, main_id, compile_time })
}

fn run_tiered_iteration(state: &mut TieredBenchmarkState) -> Result<Duration, String> {
    let exec_start = Instant::now();
    state.backend.execute_function(state.main_id, vec![])
        .map_err(|e| format!("exec: {}", e))?;
    Ok(exec_start.elapsed())
}

fn run_benchmark_tiered(bench: &Benchmark, symbols: &[(&str, *const u8)]) -> Result<(Duration, Duration), String> {
    // For single-iteration compatibility, create fresh backend
    let compile_start = Instant::now();

    // Use fast() for lazy stdlib - avoids trace resolution issues
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
    unit.add_file(&bench.source, &format!("{}.hx", bench.name))
        .map_err(|e| format!("parse: {}", e))?;
    unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

    let mir_modules = unit.get_mir_modules();

    // Use Application preset for single-iteration tiered benchmark
    // This is a legacy function - main benchmark uses setup_tiered_benchmark() with Benchmark preset
    let mut config = TierPreset::Application.to_config();
    config.start_interpreted = false;  // Start at Baseline (Cranelift) for benchmarks
    config.verbosity = 0;

    let mut backend = TieredBackend::with_symbols(config, symbols)
        .map_err(|e| format!("backend: {}", e))?;

    let main_module = mir_modules.iter().rev()
        .find(|m| m.functions.values().any(|f| f.name.ends_with("_main") || f.name == "main"))
        .ok_or("No main module")?;

    let main_id = main_module.functions.iter()
        .find(|(_, f)| f.name.ends_with("_main") || f.name == "main")
        .map(|(id, _)| *id)
        .ok_or("No main function")?;

    backend.compile_module((**main_module).clone())
        .map_err(|e| format!("load: {}", e))?;

    let compile_time = compile_start.elapsed();

    // Execute
    let exec_start = Instant::now();
    backend.execute_function(main_id, vec![])
        .map_err(|e| format!("exec: {}", e))?;
    let exec_time = exec_start.elapsed();

    Ok((compile_time, exec_time))
}

/// LLVM benchmark state - persisted across iterations (context must outlive backend)
#[cfg(feature = "llvm-backend")]
struct LLVMBenchmarkState<'ctx> {
    backend: LLVMJitBackend<'ctx>,
    mir_modules: Vec<std::sync::Arc<compiler::ir::IrModule>>,
    compile_time: Duration,
}

#[cfg(feature = "llvm-backend")]
fn setup_llvm_benchmark<'ctx>(
    bench: &Benchmark,
    symbols: &[(&str, *const u8)],
    context: &'ctx Context,
) -> Result<LLVMBenchmarkState<'ctx>, String> {
    let compile_start = Instant::now();

    // Use fast() for lazy stdlib like interpreter - avoids trace resolution issues
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
    unit.add_file(&bench.source, &format!("{}.hx", bench.name))
        .map_err(|e| format!("parse: {}", e))?;
    unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

    let mir_modules = unit.get_mir_modules();

    // NOTE: We don't apply MIR optimization here because:
    // 1. Some MIR optimization passes have bugs that break IR validity
    // 2. LLVM applies its own aggressive O3 optimizations during finalize()
    // 3. The MIR is already correct from the frontend; LLVM will optimize it further

    // Acquire LLVM lock for thread safety during compilation
    let _llvm_guard = compiler::codegen::llvm_lock();

    // Create LLVM backend (context is passed in and must outlive this)
    let mut backend = LLVMJitBackend::with_symbols(context, symbols)
        .map_err(|e| format!("backend: {}", e))?;

    // Two-pass compilation for cross-module function references:
    // 1. First declare ALL functions from ALL modules
    for module in &mir_modules {
        backend.declare_module(module).map_err(|e| format!("declare: {}", e))?;
    }
    // 2. Then compile all function bodies
    for module in &mir_modules {
        backend.compile_module_bodies(module).map_err(|e| format!("compile: {}", e))?;
    }

    // IMPORTANT: Call finalize() ONCE to run LLVM optimization passes and create execution engine.
    // finalize() runs the LLVM optimizer (default<O3>) and creates the JIT execution engine.
    // This is the expensive part that should be counted as compile time, not execution time.
    backend.finalize().map_err(|e| format!("finalize: {}", e))?;

    let compile_time = compile_start.elapsed();

    Ok(LLVMBenchmarkState { backend, mir_modules, compile_time })
}

#[cfg(feature = "llvm-backend")]
fn run_llvm_iteration(state: &mut LLVMBenchmarkState) -> Result<Duration, String> {
    let exec_start = Instant::now();
    for module in state.mir_modules.iter().rev() {
        if state.backend.call_main(module).is_ok() {
            break;
        }
    }
    Ok(exec_start.elapsed())
}

// Legacy function kept for compatibility - redirects to stateful approach
#[cfg(feature = "llvm-backend")]
fn run_benchmark_llvm(bench: &Benchmark, symbols: &[(&str, *const u8)]) -> Result<(Duration, Duration), String> {
    let context = Context::create();
    let mut state = setup_llvm_benchmark(bench, symbols, &context)?;
    let exec_time = run_llvm_iteration(&mut state)?;
    Ok((state.compile_time, exec_time))
}

fn run_benchmark(bench: &Benchmark, target: Target) -> Result<BenchmarkResult, String> {
    let symbols = get_runtime_symbols();
    let mut compile_times = Vec::new();
    let mut exec_times = Vec::new();

    match target {
        // Tiered: stateful approach for JIT promotion across iterations
        Target::RayzorTiered => {
            let mut state = setup_tiered_benchmark(bench, &symbols)?;
            let compile_time = state.compile_time;

            // Warmup - runs accumulate, triggering tier promotion
            for _ in 0..WARMUP_RUNS {
                let _ = run_tiered_iteration(&mut state);
            }

            // Process optimization queue synchronously
            let optimized = state.backend.process_queue_sync();
            if optimized > 0 {
                for _ in 0..3 {
                    let _ = run_tiered_iteration(&mut state);
                }
                let _ = state.backend.process_queue_sync();
            }

            // Upgrade to LLVM tier for maximum performance
            #[cfg(feature = "llvm-backend")]
            {
                let _ = state.backend.upgrade_to_llvm();
            }

            // Benchmark runs
            for _ in 0..BENCH_RUNS {
                match run_tiered_iteration(&mut state) {
                    Ok(exec) => {
                        compile_times.push(compile_time);
                        exec_times.push(exec);
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        // LLVM: stateful approach - finalize() should only be called once
        #[cfg(feature = "llvm-backend")]
        Target::RayzorLLVM => {
            let context = Context::create();
            let mut state = setup_llvm_benchmark(bench, &symbols, &context)?;
            let compile_time = state.compile_time;

            // Warmup runs
            for _ in 0..WARMUP_RUNS {
                let _ = run_llvm_iteration(&mut state);
            }

            // Benchmark runs
            for _ in 0..BENCH_RUNS {
                match run_llvm_iteration(&mut state) {
                    Ok(exec) => {
                        compile_times.push(compile_time);
                        exec_times.push(exec);
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        // Precompiled .rzb bundles: each iteration loads fresh (measures AOT benefits)
        Target::RayzorPrecompiled => {
            for _ in 0..WARMUP_RUNS {
                let _ = run_benchmark_precompiled(&bench.name, &symbols);
            }

            for _ in 0..BENCH_RUNS {
                match run_benchmark_precompiled(&bench.name, &symbols) {
                    Ok((load, exec)) => {
                        compile_times.push(load);  // Load time = "compile time"
                        exec_times.push(exec);
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        // Precompiled + Tiered warmup: load .rzb, warm up, promote to LLVM
        Target::RayzorPrecompiledTiered => {
            let mut state = setup_precompiled_tiered_benchmark(&bench.name, &symbols)?;
            let load_time = state.load_time;

            // Warmup - runs accumulate, triggering tier promotion
            for _ in 0..WARMUP_RUNS {
                let _ = run_precompiled_tiered_iteration(&mut state);
            }

            // Process optimization queue synchronously
            let optimized = state.backend.process_queue_sync();
            if optimized > 0 {
                for _ in 0..3 {
                    let _ = run_precompiled_tiered_iteration(&mut state);
                }
                let _ = state.backend.process_queue_sync();
            }

            // Upgrade to LLVM tier for maximum performance
            #[cfg(feature = "llvm-backend")]
            {
                // "already done" is expected when multiple backends exist - silently ignore
                let _ = state.backend.upgrade_to_llvm();
            }

            // Benchmark runs at highest tier
            for _ in 0..BENCH_RUNS {
                match run_precompiled_tiered_iteration(&mut state) {
                    Ok(exec) => {
                        compile_times.push(load_time);  // Load time = "compile time"
                        exec_times.push(exec);
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        // Cranelift and Interpreter: each iteration is independent
        Target::RayzorCranelift | Target::RayzorInterpreter => {
            for _ in 0..WARMUP_RUNS {
                let _ = match target {
                    Target::RayzorCranelift => run_benchmark_cranelift(bench, &symbols),
                    Target::RayzorInterpreter => run_benchmark_interpreter(bench, &symbols),
                    _ => unreachable!(),
                };
            }

            for _ in 0..BENCH_RUNS {
                let result = match target {
                    Target::RayzorCranelift => run_benchmark_cranelift(bench, &symbols),
                    Target::RayzorInterpreter => run_benchmark_interpreter(bench, &symbols),
                    _ => unreachable!(),
                };

                match result {
                    Ok((compile, exec)) => {
                        compile_times.push(compile);
                        exec_times.push(exec);
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    // Calculate medians
    compile_times.sort();
    exec_times.sort();

    let median_compile = compile_times[BENCH_RUNS / 2];
    let median_exec = exec_times[BENCH_RUNS / 2];
    let total = median_compile + median_exec;

    Ok(BenchmarkResult {
        name: bench.name.clone(),
        target: target.name().to_string(),
        compile_time_ms: median_compile.as_secs_f64() * 1000.0,
        runtime_ms: median_exec.as_secs_f64() * 1000.0,
        total_time_ms: total.as_secs_f64() * 1000.0,
        iterations: BENCH_RUNS as u32,
    })
}

fn print_results(results: &[BenchmarkResult]) {
    if results.is_empty() {
        return;
    }

    let bench_name = &results[0].name;
    let max_width = 50;

    println!("\n{}", "=".repeat(70));
    println!("  {} - Results", bench_name);
    println!("{}", "=".repeat(70));

    // Find baseline (cranelift) for speedup calculation
    let baseline = results.iter()
        .find(|r| r.target == "rayzor-cranelift")
        .map(|r| r.total_time_ms)
        .unwrap_or(results[0].total_time_ms);

    println!("\n{:24} {:>12} {:>12} {:>12} {:>8}", "Target", "Compile", "Execute", "Total", "vs JIT");
    println!("{}", "-".repeat(70));

    for result in results {
        let speedup = baseline / result.total_time_ms;
        let bar_len = ((result.total_time_ms / baseline) * 20.0).min(max_width as f64) as usize;
        let bar = "#".repeat(bar_len.max(1));

        println!(
            "{:24} {:>10.2}ms {:>10.2}ms {:>10.2}ms {:>7.2}x",
            result.target,
            result.compile_time_ms,
            result.runtime_ms,
            result.total_time_ms,
            speedup
        );
        println!("                         {}", bar);
    }

    println!();
}

fn save_results(suite: &BenchmarkSuite) -> Result<(), String> {
    let results_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks/results");
    fs::create_dir_all(&results_dir).map_err(|e| format!("mkdir: {}", e))?;

    let filename = format!("results_{}.json", suite.date);
    let path = results_dir.join(&filename);

    let json = serde_json::to_string_pretty(suite)
        .map_err(|e| format!("serialize: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("write: {}", e))?;
    println!("Results saved to: {}", path.display());

    Ok(())
}

fn generate_chart_html(suite: &BenchmarkSuite) -> Result<(), String> {
    let charts_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks/charts");
    fs::create_dir_all(&charts_dir).map_err(|e| format!("mkdir: {}", e))?;

    let mut html = String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Rayzor Benchmark Results</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 20px; }
        .chart-container { width: 800px; height: 400px; margin: 20px auto; }
        h1 { text-align: center; }
        h2 { margin-top: 40px; }
        .summary { background: #f5f5f5; padding: 20px; border-radius: 8px; margin: 20px 0; }
    </style>
</head>
<body>
    <h1>Rayzor Benchmark Results</h1>
    <p style="text-align: center">Generated: "#);

    html.push_str(&suite.date);
    html.push_str(r#"</p>
    <div class="summary">
        <h3>Summary</h3>
        <ul>
"#);

    for bench in &suite.benchmarks {
        html.push_str(&format!("            <li><strong>{}</strong>: {} targets measured</li>\n",
            bench.name, bench.results.len()));
    }

    html.push_str(r#"        </ul>
    </div>
"#);

    for (i, bench) in suite.benchmarks.iter().enumerate() {
        let canvas_id = format!("chart_{}", i);

        html.push_str(&format!(r#"
    <h2>{}</h2>
    <div class="chart-container">
        <canvas id="{}"></canvas>
    </div>
    <script>
        new Chart(document.getElementById('{}'), {{
            type: 'bar',
            data: {{
                labels: [{}],
                datasets: [
                    {{
                        label: 'Compile (ms)',
                        data: [{}],
                        backgroundColor: 'rgba(54, 162, 235, 0.8)'
                    }},
                    {{
                        label: 'Execute (ms)',
                        data: [{}],
                        backgroundColor: 'rgba(255, 99, 132, 0.8)'
                    }}
                ]
            }},
            options: {{
                responsive: true,
                scales: {{
                    x: {{ stacked: true }},
                    y: {{ stacked: true, title: {{ display: true, text: 'Time (ms)' }} }}
                }},
                plugins: {{
                    title: {{ display: true, text: '{}' }}
                }}
            }}
        }});
    </script>
"#,
            bench.name,
            canvas_id,
            canvas_id,
            bench.results.iter().map(|r| format!("'{}'", r.target)).collect::<Vec<_>>().join(", "),
            bench.results.iter().map(|r| format!("{:.2}", r.compile_time_ms)).collect::<Vec<_>>().join(", "),
            bench.results.iter().map(|r| format!("{:.2}", r.runtime_ms)).collect::<Vec<_>>().join(", "),
            bench.name
        ));
    }

    html.push_str(r#"
</body>
</html>
"#);

    let path = charts_dir.join("index.html");
    fs::write(&path, html).map_err(|e| format!("write: {}", e))?;
    println!("Charts saved to: {}", path.display());

    Ok(())
}

fn main() {
    // IMPORTANT: Initialize LLVM on main thread BEFORE spawning any background threads
    // This prevents crashes due to LLVM's thread-unsafe global initialization
    #[cfg(feature = "llvm-backend")]
    init_llvm_once();

    let args: Vec<String> = std::env::args().collect();

    println!("{}", "=".repeat(70));
    println!("           Rayzor Benchmark Suite");
    println!("{}", "=".repeat(70));
    println!();

    // Parse arguments
    let json_output = args.iter().any(|a| a == "--json");
    let specific_bench = args.iter()
        .find(|a| !a.starts_with("-") && *a != &args[0])
        .cloned();

    // Get available benchmarks
    let available = list_benchmarks();
    println!("Available benchmarks: {}", available.join(", "));
    println!();

    // Select benchmarks to run
    let benchmarks_to_run: Vec<String> = if let Some(name) = specific_bench {
        if available.contains(&name) {
            vec![name]
        } else {
            eprintln!("Unknown benchmark: {}. Available: {}", name, available.join(", "));
            return;
        }
    } else {
        // Run all by default
        available
    };

    let all_targets = vec![
        Target::RayzorCranelift,
        Target::RayzorInterpreter,
        Target::RayzorTiered,
        // Note: LLVM is now a standalone backend, not part of default benchmarks
        // Use --llvm flag or test_llvm_* examples for LLVM benchmarks
    ];

    println!("Running {} benchmarks x up to {} targets", benchmarks_to_run.len(), all_targets.len());
    println!("Warmup: {} runs, Benchmark: {} runs", WARMUP_RUNS, BENCH_RUNS);
    println!();

    let mut suite = BenchmarkSuite {
        date: chrono::Local::now().format("%Y-%m-%d").to_string(),
        benchmarks: Vec::new(),
    };

    for bench_name in &benchmarks_to_run {
        println!("{}", "-".repeat(70));
        println!("Benchmark: {}", bench_name);
        println!("{}", "-".repeat(70));

        let bench = match load_benchmark(bench_name) {
            Some(b) => b,
            None => {
                eprintln!("  Failed to load benchmark: {}", bench_name);
                continue;
            }
        };

        // Filter targets for this benchmark
        // Skip standalone interpreter for heavy benchmarks (millions of iterations too slow)
        // Tiered mode handles interpreter → Cranelift handoff automatically
        let is_heavy = is_heavy_benchmark(bench_name);
        let has_precompiled = has_precompiled_bundle(bench_name);

        let mut targets: Vec<Target> = all_targets.iter()
            .filter(|t| !is_heavy || !matches!(t, Target::RayzorInterpreter))
            .copied()
            .collect();

        // Add precompiled targets if .rzb bundle exists
        if has_precompiled {
            targets.push(Target::RayzorPrecompiled);
            targets.push(Target::RayzorPrecompiledTiered);
            println!("  (Precompiled .rzb bundle found - testing AOT and AOT+tiered)\n");
        }

        if is_heavy {
            println!("  (Standalone interpreter skipped - tiered mode shows full progression)\n");
        }

        // IMPORTANT: Run LLVM FIRST before spawning background threads!
        // LLVM's optimization passes hang when JIT code (Cranelift) executes concurrently.
        // This is a known limitation - LLVM finalize() must complete before other JIT runs.
        let mut results = Vec::new();

        #[cfg(feature = "llvm-backend")]
        let llvm_target = targets.iter().find(|t| matches!(t, Target::RayzorLLVM)).cloned();
        #[cfg(not(feature = "llvm-backend"))]
        let llvm_target: Option<Target> = None;

        // Run LLVM on main thread FIRST (before spawning other threads)
        #[cfg(feature = "llvm-backend")]
        if let Some(llvm) = llvm_target {
            println!("\n  Running LLVM first (must complete before other JIT)...\n");
            let result = run_benchmark(&bench, llvm);
            match result {
                Ok(bench_result) => {
                    println!("  [DONE] {} ({})", llvm.name(), llvm.description());
                    println!("         Compile: {:.2}ms, Execute: {:.2}ms, Total: {:.2}ms\n",
                        bench_result.compile_time_ms, bench_result.runtime_ms, bench_result.total_time_ms);
                    results.push(bench_result);
                }
                Err(e) => {
                    eprintln!("  [FAIL] {}: {}\n", llvm.name(), e);
                }
            }
        }

        // Run tiered NEXT (before other parallel targets) to ensure consistent timing
        // Tiered mode uses interpreter for startup, then promotes to Cranelift JIT
        let tiered_target = targets.iter().find(|t| matches!(t, Target::RayzorTiered)).cloned();
        if let Some(tiered) = tiered_target {
            println!("  Running tiered (interpreter -> Cranelift)...\n");
            let result = run_benchmark(&bench, tiered);
            match result {
                Ok(bench_result) => {
                    println!("  [DONE] {} ({})", tiered.name(), tiered.description());
                    println!("         Compile: {:.2}ms, Execute: {:.2}ms, Total: {:.2}ms\n",
                        bench_result.compile_time_ms, bench_result.runtime_ms, bench_result.total_time_ms);
                    results.push(bench_result);
                }
                Err(e) => {
                    eprintln!("  [FAIL] {}: {}\n", tiered.name(), e);
                }
            }
        }

        // NOW run remaining targets in parallel (Cranelift, Interpreter)
        let (tx, rx) = mpsc::channel::<(Target, Result<BenchmarkResult, String>)>();
        let mut handles = Vec::new();

        let parallel_targets: Vec<Target> = targets.iter()
            .filter(|t| {
                #[cfg(feature = "llvm-backend")]
                if matches!(t, Target::RayzorLLVM) { return false; }
                !matches!(t, Target::RayzorTiered)
            })
            .copied()
            .collect();

        if !parallel_targets.is_empty() {
            println!("  Running {} other targets in parallel...\n", parallel_targets.len());
        }

        for target in parallel_targets {
            let tx = tx.clone();
            let bench_source = bench.source.clone();
            let bench_name_clone = bench.name.clone();

            let handle = thread::spawn(move || {
                let bench = Benchmark {
                    name: bench_name_clone,
                    source: bench_source,
                };
                let result = run_benchmark(&bench, target);
                let _ = tx.send((target, result));
            });
            handles.push(handle);
        }
        drop(tx); // Close sender so receiver knows when all threads are done

        while let Ok((target, result)) = rx.recv() {
            match result {
                Ok(bench_result) => {
                    println!("  [DONE] {} ({})", target.name(), target.description());
                    println!("         Compile: {:.2}ms, Execute: {:.2}ms, Total: {:.2}ms\n",
                        bench_result.compile_time_ms, bench_result.runtime_ms, bench_result.total_time_ms);
                    results.push(bench_result);
                }
                Err(e) => {
                    eprintln!("  [FAIL] {}: {}\n", target.name(), e);
                }
            }
        }

        // Wait for all threads to complete
        for handle in handles {
            let _ = handle.join();
        }

        // Sort results by target name for consistent ordering
        results.sort_by(|a, b| a.target.cmp(&b.target));

        print_results(&results);

        suite.benchmarks.push(BenchmarkResults {
            name: bench_name.clone(),
            results: results.clone(),
        });
    }

    // Save results
    println!("\n{}", "=".repeat(70));
    println!("Saving results...");

    if let Err(e) = save_results(&suite) {
        eprintln!("Failed to save results: {}", e);
    }

    if let Err(e) = generate_chart_html(&suite) {
        eprintln!("Failed to generate charts: {}", e);
    }

    if json_output {
        println!("\nJSON Output:");
        println!("{}", serde_json::to_string_pretty(&suite).unwrap_or_default());
    }
}
