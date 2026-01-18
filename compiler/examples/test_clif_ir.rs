use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn get_runtime_symbols() -> Vec<(&'static str, *const u8)> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    plugin.runtime_symbols().iter().map(|(n, p)| (*n, *p)).collect()
}

fn main() {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();
    
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
}
