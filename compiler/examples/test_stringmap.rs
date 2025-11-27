//! Test StringMap and IntMap from haxe.ds

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn main() {
    println!("=== StringMap/IntMap Test ===\n");

    // Test 1: Minimal StringMap - just new and exists
    println!("Test 1: StringMap new + exists");
    let source1 = r#"
import haxe.ds.StringMap;

class Main {
    static function main() {
        var map = new StringMap<Int>();
        trace("map created");

        // Check exists on empty map
        var exists = map.exists("key");
        trace(exists);  // false
    }
}
"#;
    run_test(source1, "stringmap_exists");

    // Test 2: StringMap set
    println!("\nTest 2: StringMap set");
    let source2 = r#"
import haxe.ds.StringMap;

class Main {
    static function main() {
        var map = new StringMap<Int>();

        // Set a value
        map.set("one", 1);
        trace("set done");

        // Check exists
        trace(map.exists("one"));  // true
    }
}
"#;
    run_test(source2, "stringmap_set");

    // Test 3: StringMap get
    println!("\nTest 3: StringMap get");
    let source3 = r#"
import haxe.ds.StringMap;

class Main {
    static function main() {
        var map = new StringMap<Int>();

        // Set values
        map.set("one", 1);
        map.set("two", 2);
        map.set("three", 3);

        // Get values
        var v1 = map.get("one");
        var v2 = map.get("two");
        var v3 = map.get("three");

        trace(v1);  // 1
        trace(v2);  // 2
        trace(v3);  // 3
    }
}
"#;
    run_test(source3, "stringmap_get");

    // Test 4: StringMap<Float>
    println!("\nTest 4: StringMap<Float>");
    let source4 = r#"
import haxe.ds.StringMap;

class Main {
    static function main() {
        var map = new StringMap<Float>();

        map.set("pi", 3.14159);
        map.set("e", 2.71828);

        var pi = map.get("pi");
        var e = map.get("e");

        trace(pi);  // 3.14159
        trace(e);   // 2.71828
    }
}
"#;
    run_test(source4, "stringmap_float");

    // Test 5: StringMap<Bool>
    println!("\nTest 5: StringMap<Bool>");
    let source5 = r#"
import haxe.ds.StringMap;

class Main {
    static function main() {
        var map = new StringMap<Bool>();

        map.set("yes", true);
        map.set("no", false);

        trace(map.get("yes"));  // true
        trace(map.get("no"));   // false
        trace(map.exists("yes"));  // true
        trace(map.exists("maybe")); // false
    }
}
"#;
    run_test(source5, "stringmap_bool");

    // Test 6: IntMap<Int>
    println!("\nTest 6: IntMap<Int>");
    let source6 = r#"
import haxe.ds.IntMap;

class Main {
    static function main() {
        var map = new IntMap<Int>();

        map.set(1, 100);
        map.set(2, 200);
        map.set(42, 4200);

        trace(map.get(1));   // 100
        trace(map.get(2));   // 200
        trace(map.get(42));  // 4200
        trace(map.exists(42));  // true
        trace(map.exists(99));  // false
    }
}
"#;
    run_test(source6, "intmap_int");

    // Test 7: IntMap<Float>
    println!("\nTest 7: IntMap<Float>");
    let source7 = r#"
import haxe.ds.IntMap;

class Main {
    static function main() {
        var map = new IntMap<Float>();

        map.set(0, 0.0);
        map.set(1, 1.5);
        map.set(2, 2.5);

        trace(map.get(0));  // 0.0
        trace(map.get(1));  // 1.5
        trace(map.get(2));  // 2.5
    }
}
"#;
    run_test(source7, "intmap_float");

    // Test 8: StringMap remove
    println!("\nTest 8: StringMap remove");
    let source8 = r#"
import haxe.ds.StringMap;

class Main {
    static function main() {
        var map = new StringMap<Int>();

        map.set("a", 1);
        map.set("b", 2);
        trace(map.exists("a"));  // true

        var removed = map.remove("a");
        trace(removed);  // true
        trace(map.exists("a"));  // false

        var removed2 = map.remove("nonexistent");
        trace(removed2);  // false
    }
}
"#;
    run_test(source8, "stringmap_remove");

    // Test 9: StringMap<Class> - storing class object pointers
    println!("\nTest 9: StringMap<Class> - storing class objects");
    let source9 = r#"
import haxe.ds.StringMap;

class Point {
    public var x:Int;
    public var y:Int;

    public function new(x:Int, y:Int) {
        this.x = x;
        this.y = y;
    }
}

class Main {
    static function main() {
        var map = new StringMap<Point>();

        // Store class objects
        var p1 = new Point(10, 20);
        var p2 = new Point(30, 40);
        var p3 = new Point(50, 60);

        map.set("origin", new Point(0, 0));
        map.set("point1", p1);
        map.set("point2", p2);

        // Retrieve and verify
        var retrieved = map.get("point1");
        trace(retrieved.x);  // 10
        trace(retrieved.y);  // 20

        var origin = map.get("origin");
        trace(origin.x);  // 0
        trace(origin.y);  // 0

        // Verify exists
        trace(map.exists("point1"));  // true
        trace(map.exists("missing"));  // false
    }
}
"#;
    run_test(source9, "stringmap_class");

    // Test 10: IntMap<Class> - storing class objects with int keys
    println!("\nTest 10: IntMap<Class> - storing class objects");
    let source10 = r#"
import haxe.ds.IntMap;

class Entity {
    public var id:Int;
    public var name:String;

    public function new(id:Int, name:String) {
        this.id = id;
        this.name = name;
    }
}

class Main {
    static function main() {
        var entities = new IntMap<Entity>();

        // Store entities by ID
        entities.set(1, new Entity(1, "Player"));
        entities.set(2, new Entity(2, "Enemy"));
        entities.set(100, new Entity(100, "Boss"));

        // Retrieve and verify
        var player = entities.get(1);
        trace(player.id);    // 1
        trace(player.name);  // Player

        var boss = entities.get(100);
        trace(boss.id);      // 100
        trace(boss.name);    // Boss

        // Check exists
        trace(entities.exists(1));   // true
        trace(entities.exists(999)); // false
    }
}
"#;
    run_test(source10, "intmap_class");
}

fn run_test(source: &str, name: &str) {
    match compile_and_run(source, name) {
        Ok(()) => {
            println!("✅ {} PASSED", name);
        }
        Err(e) => {
            println!("❌ {} FAILED: {}", name, e);
        }
    }
}

fn compile_and_run(source: &str, name: &str) -> Result<(), String> {
    let mut unit = CompilationUnit::new(CompilationConfig::default());
    unit.load_stdlib()?;
    unit.add_file(source, &format!("{}.hx", name))?;

    let _typed_files = unit.lower_to_tast().map_err(|errors| {
        format!("TAST lowering failed: {:?}", errors)
    })?;

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
    // Note: RTTI for primitives is lazily initialized on first access
    for module in modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            return Ok(());
        }
    }
    Err("Failed to execute main".to_string())
}
