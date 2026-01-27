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
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    let source = r#"
package test;

class Point {
    public var x:Float;
    public var y:Float;

    public function new(x:Float, y:Float) {
        this.x = x;
        this.y = y;
    }

    public function add(p:Point):Point {
        return new Point(x + p.x, y + p.y);
    }
}

class TestPoint {
    public static function main() {
        var p1 = new Point(1.0, 2.0);
        var p2 = new Point(3.0, 4.0);
        var p3 = p1.add(p2);
        trace(p3.x);
    }
}
    "#;

    println!("Compiling...");
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(&source, "test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    // Print all function names
    println!("\n=== All functions ===");
    for module in &mir_modules {
        for (_id, func) in &module.functions {
            if func.name.contains("Point") {
                println!("Found: {}", func.name);
            }
        }
    }
}
