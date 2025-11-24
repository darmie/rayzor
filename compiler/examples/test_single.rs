/// Single isolated test to debug symbol resolution
use compiler::compilation::{CompilationUnit, CompilationConfig};

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
