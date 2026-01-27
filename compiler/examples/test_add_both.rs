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
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn get_runtime_symbols() -> Vec<(&'static str, *const u8)> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    plugin
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect()
}

fn main() {
    let source = std::fs::read_to_string("/tmp/test_add_both.hx").expect("read");
    let symbols = get_runtime_symbols();

    println!("Compiling...");
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(&source, "test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    let mut backend = CraneliftBackend::with_symbols(&symbols).expect("backend");
    for module in &mir_modules {
        backend.compile_module(module).expect("compile");
    }
    println!("Compiled!");

    println!("Running...");
    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            break;
        }
    }
    println!("Done!");
}
