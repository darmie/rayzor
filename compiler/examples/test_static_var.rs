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
//! Test static var access

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn test_case(name: &str, source: &str, symbols: &[(&str, *const u8)]) {
    println!("\n=== Test: {} ===", name);

    let result: Result<(), String> = (|| {
        let mut unit = CompilationUnit::new(CompilationConfig::fast());
        unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
        unit.add_file(source, &format!("{}.hx", name))
            .map_err(|e| format!("parse: {}", e))?;
        unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

        let mir_modules = unit.get_mir_modules();

        let mut backend =
            CraneliftBackend::with_symbols(symbols).map_err(|e| format!("backend: {}", e))?;

        for module in &mir_modules {
            backend
                .compile_module(module)
                .map_err(|e| format!("compile: {}", e))?;
        }

        for module in mir_modules.iter().rev() {
            if backend.call_main(module).is_ok() {
                println!("  SUCCESS!");
                return Ok(());
            }
        }
        Err("No main executed".to_string())
    })();

    if let Err(e) = result {
        println!("  FAILED: {:?}", e);
    }
}

fn main() {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    // Test 1: Literal numbers (baseline)
    test_case(
        "literal_numbers",
        r#"
package test;
class Main {
    public static function main() {
        var a = 5;
        var b = (a - 2) * 4 / 5;
        trace(b);
    }
}
"#,
        &symbols,
    );

    // Test 2: Static var read
    test_case(
        "static_var_read",
        r#"
package test;
class Main {
    static var SIZE = 5;

    public static function main() {
        var s = SIZE;
        trace(s);
    }
}
"#,
        &symbols,
    );

    // Test 3: Static var in calculation
    test_case(
        "static_var_calc",
        r#"
package test;
class Main {
    static var SIZE = 5;

    public static function main() {
        var x = 3;
        var r = (x - SIZE / 2) * 4.0 / SIZE;
        trace(r);
    }
}
"#,
        &symbols,
    );

    // Test 4: Multiple static vars
    test_case(
        "multi_static_var",
        r#"
package test;
class Main {
    static var WIDTH = 5;
    static var HEIGHT = 5;

    public static function main() {
        var w = WIDTH;
        var h = HEIGHT;
        trace(w + h);
    }
}
"#,
        &symbols,
    );

    // Test 5: Static var in another class
    test_case(
        "static_var_other_class",
        r#"
package test;
class Config {
    public static var SIZE = 5;
}
class Main {
    public static function main() {
        var s = Config.SIZE;
        trace(s);
    }
}
"#,
        &symbols,
    );

    // Test 6: For loop with literal bound (baseline)
    test_case(
        "loop_literal_bound",
        r#"
package test;
class Main {
    public static function main() {
        var sum = 0;
        for (i in 0...5) {
            sum = sum + i;
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    // Test 7: For loop with local var bound
    test_case(
        "loop_local_var_bound",
        r#"
package test;
class Main {
    public static function main() {
        var n = 5;
        var sum = 0;
        for (i in 0...n) {
            sum = sum + i;
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    // Test 8: Static var assigned to local, used in loop
    test_case(
        "static_to_local_loop",
        r#"
package test;
class Main {
    static var SIZE = 5;

    public static function main() {
        var n = SIZE;
        var sum = 0;
        for (i in 0...n) {
            sum = sum + i;
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    // Test 9: Static var directly in loop bound
    test_case(
        "static_direct_loop",
        r#"
package test;
class Main {
    static var SIZE = 5;

    public static function main() {
        var sum = 0;
        for (i in 0...SIZE) {
            sum = sum + i;
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    println!("\n=== All tests completed ===");
}
