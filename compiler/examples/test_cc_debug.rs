#![allow(
    unused_imports,
    unused_variables,
    dead_code,
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

fn main() {
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");

    let source = r#"
package test;

import rayzor.runtime.CC;

class Main {
    static function main() {
        var cc = CC.create();
        cc.compile("long answer(void) { return 42; }");
        cc.relocate();
        var sym = cc.getSymbol("answer");
        trace(sym != 0);
        var result = CC.call0(sym);
        trace(result);
        cc.delete();
    }
}
"#;

    unit.add_file(source, "test_cc.hx").expect("add file");

    match unit.lower_to_tast() {
        Ok(files) => println!("TAST: {} files", files.len()),
        Err(e) => {
            println!("TAST failed: {:?}", e);
            return;
        }
    }

    let mir_modules = unit.get_mir_modules();
    println!("MIR: {} modules", mir_modules.len());

    for (i, m) in mir_modules.iter().enumerate() {
        println!("\nModule {}:", i);

        // Print ALL extern + regular functions
        for (fid, ef) in &m.extern_functions {
            println!(
                "  EXTERN {:?}: {} params=[{}] ret={:?}",
                fid,
                ef.name,
                ef.signature
                    .parameters
                    .iter()
                    .map(|p| format!("{}: {:?}", p.name, p.ty))
                    .collect::<Vec<_>>()
                    .join(", "),
                ef.signature.return_type
            );
        }
        for (fid, f) in &m.functions {
            if !f.cfg.blocks.is_empty() {
                println!(
                    "  FUNC {:?}: {} ({} blocks)",
                    fid,
                    f.name,
                    f.cfg.blocks.len()
                );
            }
        }

        // Print main and TCC-related function MIR
        for func in &m.functions {
            let name = &func.1.name;
            if name.contains("main") || name.contains("Main") || name.contains("tcc") {
                println!("  === {} ===", name);
                for (bid, block) in &func.1.cfg.blocks {
                    println!("    block {:?}:", bid);
                    for instr in &block.instructions {
                        println!("      {:?}", instr);
                    }
                }
            }
        }
    }

    // Now compile and execute
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    // Debug: print TCC-related symbols
    for (name, ptr) in &symbols_ref {
        if name.contains("tcc") {
            println!("  SYM: {} @ {:p}", name, *ptr);
        }
    }
    let mut backend = CraneliftBackend::with_symbols(&symbols_ref).expect("backend");

    for module in &mir_modules {
        backend.compile_module(module).expect("compile");
    }
    println!("\nExecuting...");
    for module in mir_modules.iter().rev() {
        if let Ok(()) = backend.call_main(module) {
            println!("  Execution OK");
            return;
        }
    }
    println!("  Failed to execute main");
}
