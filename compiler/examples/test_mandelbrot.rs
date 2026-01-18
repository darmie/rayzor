use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use std::time::Instant;

fn get_runtime_symbols() -> Vec<(&'static str, *const u8)> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    plugin.runtime_symbols().iter().map(|(n, p)| (*n, *p)).collect()
}

fn main() {
    let source = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/benchmarks/src/mandelbrot.hx")
    ).expect("read");
    let symbols = get_runtime_symbols();

    println!("Compiling mandelbrot...");
    let compile_start = Instant::now();
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(&source, "mandelbrot.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    let mut backend = CraneliftBackend::with_symbols(&symbols).expect("backend");
    for module in &mir_modules {
        backend.compile_module(module).expect("compile");
    }
    let compile_time = compile_start.elapsed();
    println!("Compiled in {:?}", compile_time);

    println!("Running mandelbrot...");
    let exec_start = Instant::now();
    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            break;
        }
    }
    let exec_time = exec_start.elapsed();
    println!("Executed in {:?}", exec_time);
}
