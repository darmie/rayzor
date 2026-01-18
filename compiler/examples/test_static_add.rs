use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn get_runtime_symbols() -> Vec<(&'static str, *const u8)> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    plugin.runtime_symbols().iter().map(|(n, p)| (*n, *p)).collect()
}

fn main() {
    let source = std::fs::read_to_string("/tmp/test_static_add.hx").expect("read");
    let symbols = get_runtime_symbols();

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(&source, "test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    let mut backend = CraneliftBackend::with_symbols(&symbols).expect("backend");
    for module in &mir_modules {
        backend.compile_module(module).expect("compile");
    }

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            break;
        }
    }
}
