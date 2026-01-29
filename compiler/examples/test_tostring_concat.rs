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
//! Test string concatenation with class instances that implement toString()
//! This exercises the DynamicValue runtime dispatch path for toString resolution.

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use std::sync::Arc;

fn main() {
    println!("=== toString() String Concatenation Test ===\n");

    // Test 1: String + class instance with toString()
    println!("Test 1: String concatenation with toString() class");
    let source1 = r#"
class Point {
    public var x:Int;
    public var y:Int;

    public function new(x:Int, y:Int) {
        this.x = x;
        this.y = y;
    }

    public function toString():String {
        return "Point(" + x + ", " + y + ")";
    }
}

class Main {
    static function main() {
        var p = new Point(3, 7);
        trace("The point is: " + p);
    }
}
"#;
    run_test(source1, "string_concat_tostring");

    // Test 2: Class in enum field, extracted via pattern match, concatenated
    println!("\nTest 2: Class in Result enum, extracted and concatenated");
    let source2 = r#"
import rayzor.core.Result;

class Coord {
    public var lat:Int;
    public var lon:Int;

    public function new(lat:Int, lon:Int) {
        this.lat = lat;
        this.lon = lon;
    }

    public function toString():String {
        return "Coord(" + lat + ", " + lon + ")";
    }
}

class Main {
    static function main() {
        var r = Result.Ok(new Coord(40, -74));
        switch (r) {
            case Ok(c): trace("Location: " + c);
            case Error(_): trace("error");
        }
    }
}
"#;
    run_test(source2, "enum_field_class_tostring");

    // Test 3: trace(obj) directly — compile-time toString dispatch
    println!("\nTest 3: Direct trace of class instance (compile-time dispatch)");
    let source3 = r#"
class Color {
    public var r:Int;
    public var g:Int;
    public var b:Int;

    public function new(r:Int, g:Int, b:Int) {
        this.r = r;
        this.g = g;
        this.b = b;
    }

    public function toString():String {
        return "rgb(" + r + ", " + g + ", " + b + ")";
    }
}

class Main {
    static function main() {
        var c = new Color(255, 128, 0);
        trace(c);
        trace("Color is: " + c);
    }
}
"#;
    run_test(source3, "direct_trace_and_concat");
}

fn run_test(source: &str, name: &str) {
    match compile_and_run(source, name) {
        Ok(()) => {
            println!("  {} PASSED", name);
        }
        Err(e) => {
            println!("  {} FAILED: {}", name, e);
        }
    }
}

fn compile_and_run(source: &str, name: &str) -> Result<(), String> {
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib()?;
    unit.add_file(source, &format!("{}.hx", name))?;

    let _typed_files = unit
        .lower_to_tast()
        .map_err(|errors| format!("TAST lowering failed: {:?}", errors))?;

    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    let mut backend = compile_to_native(&mir_modules)?;
    execute_main(&mut backend, &mir_modules)?;

    Ok(())
}

fn compile_to_native(modules: &[Arc<IrModule>]) -> Result<CraneliftBackend, String> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in modules {
        backend.compile_module(module)?;
    }

    Ok(backend)
}

fn execute_main(backend: &mut CraneliftBackend, modules: &[Arc<IrModule>]) -> Result<(), String> {
    backend.initialize_modules(modules)?;

    for module in modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            return Ok(());
        }
    }
    Err("Failed to execute main".to_string())
}
