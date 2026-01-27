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
//! Test inline variable in loop bound - isolate crash

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn test_case(name: &str, source: &str, symbols: &[(&str, *const u8)]) {
    println!("\n=== Test: {} ===", name);

    let result = std::panic::catch_unwind(|| {
        let mut unit = CompilationUnit::new(CompilationConfig::fast());
        unit.load_stdlib().expect("stdlib");
        unit.add_file(source, &format!("{}.hx", name))
            .expect("parse");
        unit.lower_to_tast().expect("tast");

        let mir_modules = unit.get_mir_modules();

        let mut backend = CraneliftBackend::with_symbols(symbols).expect("backend");

        for module in &mir_modules {
            backend.compile_module(module).expect("compile");
        }

        for module in mir_modules.iter().rev() {
            if backend.call_main(module).is_ok() {
                return true;
            }
        }
        false
    });

    match result {
        Ok(true) => println!("  SUCCESS!"),
        Ok(false) => println!("  FAILED (no main executed)"),
        Err(_) => println!("  CRASHED (panic)"),
    }
}

fn main() {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    // Test A: Regular variable in loop bound (should work)
    test_case(
        "regular_var_bound",
        r#"
package test;
class Main {
    public static function main() {
        var size = 5;
        var sum = 0;
        for (i in 0...size) {
            sum = sum + i;
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    // Test B: Static inline var NOT in loop bound (should work)
    test_case(
        "inline_not_in_bound",
        r#"
package test;
class Main {
    static inline var SIZE = 5;

    public static function main() {
        var s = SIZE;
        trace(s);
    }
}
"#,
        &symbols,
    );

    // Test C: Static inline var in loop bound (may crash)
    test_case(
        "inline_in_bound",
        r#"
package test;
class Main {
    static inline var SIZE = 5;

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

    // Test D: Static inline assigned to var, then used in loop
    test_case(
        "inline_via_var",
        r#"
package test;
class Main {
    static inline var SIZE = 5;

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

    // Test E: Plain static (not inline) in loop bound
    test_case(
        "static_not_inline",
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
