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
/// Single isolated test to debug symbol resolution
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    println!("=== Single Test - Thread Spawn Basic ===\n");

    let test_code = r#"
package test;

import rayzor.concurrent.Thread;

@:derive([Send])
class Message {
    public var value: Int;
    public function new(v: Int) {
        this.value = v;
    }
}

class Main {
    static function main() {
        var msg = new Message(42);
        var handle = Thread.spawn(() -> {
            return msg.value;
        });
        var result = handle.join();
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    println!("Loading stdlib...");
    if let Err(e) = unit.load_stdlib() {
        eprintln!("❌ Failed to load stdlib: {}", e);
        return;
    }

    println!("Adding test file...");
    if let Err(e) = unit.add_file(test_code, "thread_spawn_basic.hx") {
        eprintln!("❌ Failed to add file: {}", e);
        return;
    }

    println!("Lowering to TAST...");
    match unit.lower_to_tast() {
        Ok(_) => println!("✅ TAST lowering succeeded!"),
        Err(errors) => {
            eprintln!("❌ TAST lowering failed with {} errors:", errors.len());
            for (i, error) in errors.iter().take(5).enumerate() {
                eprintln!("  {}. {}", i + 1, error.message);
            }
        }
    }
}
