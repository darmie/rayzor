//! Rayzor Benchmark Suite Runner
//!
//! Compares Rayzor execution modes against each other and external targets.
//!
//! Usage:
//!   cargo run --release --package compiler --example benchmark_runner
//!   cargo run --release --package compiler --example benchmark_runner -- mandelbrot
//!   cargo run --release --package compiler --example benchmark_runner -- --json

use compiler::codegen::tiered_backend::{TieredBackend, TieredConfig};
use compiler::codegen::profiling::ProfileConfig;
use compiler::codegen::CraneliftBackend;
use compiler::codegen::InterpValue;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrFunctionId;

#[cfg(feature = "llvm-backend")]
use compiler::codegen::LLVMJitBackend;
#[cfg(feature = "llvm-backend")]
use inkwell::context::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

const WARMUP_RUNS: usize = 3;
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

#[derive(Debug, Clone, Copy)]
enum Target {
    RayzorInterpreter,
    RayzorCranelift,
    RayzorTiered,
    #[cfg(feature = "llvm-backend")]
    RayzorLLVM,
}

impl Target {
    fn name(&self) -> &'static str {
        match self {
            Target::RayzorInterpreter => "rayzor-interpreter",
            Target::RayzorCranelift => "rayzor-cranelift",
            Target::RayzorTiered => "rayzor-tiered",
            #[cfg(feature = "llvm-backend")]
            Target::RayzorLLVM => "rayzor-llvm",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Target::RayzorInterpreter => "MIR Interpreter (instant startup)",
            Target::RayzorCranelift => "Cranelift JIT (optimized)",
            Target::RayzorTiered => "Tiered (interpreter -> JIT)",
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

    let mir_modules = unit.get_mir_modules();
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

    let config = TieredConfig {
        profile_config: ProfileConfig {
            interpreter_threshold: u64::MAX,  // Never promote
            warm_threshold: u64::MAX,
            hot_threshold: u64::MAX,
            blazing_threshold: u64::MAX,
            sample_rate: 1,
        },
        enable_background_optimization: false,
        optimization_check_interval_ms: 1000,
        max_parallel_optimizations: 1,
        verbosity: 0,
        start_interpreted: true,
    };

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

fn setup_tiered_benchmark(bench: &Benchmark, symbols: &[(&str, *const u8)]) -> Result<TieredBenchmarkState, String> {
    let compile_start = Instant::now();

    // Use fast() for lazy stdlib - avoids trace resolution issues
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
    unit.add_file(&bench.source, &format!("{}.hx", bench.name))
        .map_err(|e| format!("parse: {}", e))?;
    unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

    let mir_modules = unit.get_mir_modules();

    let config = TieredConfig {
        profile_config: ProfileConfig {
            interpreter_threshold: 5,     // JIT after 5 calls (warmup will trigger)
            warm_threshold: 50,
            hot_threshold: 200,
            blazing_threshold: 1000,
            sample_rate: 1,
        },
        enable_background_optimization: true,
        optimization_check_interval_ms: 5,
        max_parallel_optimizations: 4,
        verbosity: 0,
        start_interpreted: true,
    };

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

    let config = TieredConfig {
        profile_config: ProfileConfig {
            interpreter_threshold: 5,
            warm_threshold: 50,
            hot_threshold: 200,
            blazing_threshold: 1000,
            sample_rate: 1,
        },
        enable_background_optimization: true,
        optimization_check_interval_ms: 5,
        max_parallel_optimizations: 4,
        verbosity: 0,
        start_interpreted: true,
    };

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

#[cfg(feature = "llvm-backend")]
fn run_benchmark_llvm(bench: &Benchmark, symbols: &[(&str, *const u8)]) -> Result<(Duration, Duration), String> {
    // Compile
    let compile_start = Instant::now();

    // Use fast() for lazy stdlib like interpreter - avoids trace resolution issues
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
    unit.add_file(&bench.source, &format!("{}.hx", bench.name))
        .map_err(|e| format!("parse: {}", e))?;
    unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

    let mir_modules = unit.get_mir_modules();

    // Create LLVM context and backend
    let context = Context::create();
    let mut backend = LLVMJitBackend::with_symbols(&context, symbols)
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

fn run_benchmark(bench: &Benchmark, target: Target) -> Result<BenchmarkResult, String> {
    let symbols = get_runtime_symbols();
    let mut compile_times = Vec::new();
    let mut exec_times = Vec::new();

    // For tiered, use stateful approach to allow JIT promotion across iterations
    if matches!(target, Target::RayzorTiered) {
        // Set up tiered backend ONCE - this is key for JIT promotion
        let mut state = setup_tiered_benchmark(bench, &symbols)?;
        let compile_time = state.compile_time;

        // Warmup - runs accumulate, triggering JIT promotion
        print!("  Warming up {} ({})... ", bench.name, target.name());
        for _ in 0..WARMUP_RUNS {
            let _ = run_tiered_iteration(&mut state);
        }
        // Give background JIT time to compile (if threshold reached)
        std::thread::sleep(std::time::Duration::from_millis(50));
        println!("done");

        // Benchmark runs - should now be running JIT-compiled code
        print!("  Running {} iterations... ", BENCH_RUNS);
        for i in 0..BENCH_RUNS {
            match run_tiered_iteration(&mut state) {
                Ok(exec) => {
                    compile_times.push(compile_time); // Same compile time for all
                    exec_times.push(exec);
                }
                Err(e) => {
                    println!("FAILED at iteration {}: {}", i + 1, e);
                    return Err(e);
                }
            }
        }
        println!("done");
    } else {
        // Non-tiered: each iteration is independent
        print!("  Warming up {} ({})... ", bench.name, target.name());
        for _ in 0..WARMUP_RUNS {
            let _ = match target {
                Target::RayzorCranelift => run_benchmark_cranelift(bench, &symbols),
                Target::RayzorInterpreter => run_benchmark_interpreter(bench, &symbols),
                Target::RayzorTiered => unreachable!(),
                #[cfg(feature = "llvm-backend")]
                Target::RayzorLLVM => run_benchmark_llvm(bench, &symbols),
            };
        }
        println!("done");

        print!("  Running {} iterations... ", BENCH_RUNS);
        for i in 0..BENCH_RUNS {
            let result = match target {
                Target::RayzorCranelift => run_benchmark_cranelift(bench, &symbols),
                Target::RayzorInterpreter => run_benchmark_interpreter(bench, &symbols),
                Target::RayzorTiered => unreachable!(),
                #[cfg(feature = "llvm-backend")]
                Target::RayzorLLVM => run_benchmark_llvm(bench, &symbols),
            };

            match result {
                Ok((compile, exec)) => {
                    compile_times.push(compile);
                    exec_times.push(exec);
                }
                Err(e) => {
                    println!("FAILED at iteration {}: {}", i + 1, e);
                    return Err(e);
                }
            }
        }
        println!("done");
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

    let mut targets = vec![
        Target::RayzorCranelift,
        Target::RayzorInterpreter,
        Target::RayzorTiered,
    ];

    #[cfg(feature = "llvm-backend")]
    targets.push(Target::RayzorLLVM);

    println!("Running {} benchmarks x {} targets", benchmarks_to_run.len(), targets.len());
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

        let mut results = Vec::new();

        for target in &targets {
            println!("\n  Target: {} ({})", target.name(), target.description());

            match run_benchmark(&bench, *target) {
                Ok(result) => {
                    println!("  -> Compile: {:.2}ms, Execute: {:.2}ms, Total: {:.2}ms",
                        result.compile_time_ms, result.runtime_ms, result.total_time_ms);
                    results.push(result);
                }
                Err(e) => {
                    eprintln!("  -> FAILED: {}", e);
                }
            }
        }

        print_results(&results);

        suite.benchmarks.push(BenchmarkResults {
            name: bench_name.clone(),
            results,
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
