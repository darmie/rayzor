#![allow(unused_imports, unused_variables, dead_code)]
//! Debug LLVM JIT with arrays

#[cfg(feature = "llvm-backend")]
use compiler::codegen::LLVMJitBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
#[cfg(feature = "llvm-backend")]
use inkwell::context::Context;

fn get_runtime_symbols() -> Vec<(&'static str, *const u8)> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    plugin
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect()
}

fn main() {
    #[cfg(not(feature = "llvm-backend"))]
    {
        eprintln!("LLVM backend not enabled.");
        return;
    }

    #[cfg(feature = "llvm-backend")]
    {
        let source = r#"
package test;

class ArrayTest {
    public static function main() {
        trace("Before array");
        var arr = new Array<Int>();
        trace("After create");
        arr.push(1);
        trace("After push 1");
        arr.push(2);
        trace("After push 2");
        arr.push(3);
        trace("After push 3");
        trace("Done!");
    }
}
"#;
        let symbols = get_runtime_symbols();

        println!("Compiling array test...");
        let mut unit = CompilationUnit::new(CompilationConfig::fast());
        unit.load_stdlib().expect("stdlib");
        unit.add_file(&source, "array_test.hx").expect("parse");
        unit.lower_to_tast().expect("tast");
        let mir_modules = unit.get_mir_modules();

        let context = Context::create();
        let mut backend = LLVMJitBackend::with_symbols(&context, &symbols).expect("backend");

        for module in &mir_modules {
            backend.declare_module(module).expect("declare");
        }

        for module in &mir_modules {
            backend.compile_module_bodies(module).expect("compile");
        }

        backend.finalize().expect("finalize");

        println!("Calling main...");
        for module in mir_modules.iter().rev() {
            if backend.call_main(module).is_ok() {
                break;
            }
        }
        println!("Test complete!");
    }
}
